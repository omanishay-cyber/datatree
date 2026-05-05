/**
 * Hook: PreToolUse (Edit / Write / MultiEdit) — Layer 2 of the v0.4.0
 * self-ping enforcement layer.
 *
 * When the AI calls Edit or Write on file X:
 *   - Checks tool_cache.db for a blast_radius / file_intent call for X
 *     within the last `blast_radius_freshness_seconds` (default 600 s).
 *   - If FOUND  → approve the edit (pass through).
 *   - If NOT FOUND → BLOCK the edit with a structured message:
 *       "Run blast_radius X first. Auto-running it now: <result>. Try Edit again."
 *     Then immediately queries blast_radius via the MCP query layer and
 *     injects the result into the block message so the AI has it in context
 *     and can re-attempt the edit in the same turn.
 *
 * Hook output protocol (Claude Code spec for PreToolUse):
 *   Approve:  { "hook_specific": { "decision": "approve" } }  — exit 0
 *   Block:    { "hook_specific": { "decision": "block", "reason": "..." } }  — exit 2
 *
 * CRITICAL — Fail-open guarantee:
 *   Any error in this hook (IPC down, DB missing, timeout, bug) MUST let
 *   the edit through. We achieve this via a top-level try/catch that returns
 *   approve on any exception. A broken mneme daemon MUST NEVER block the
 *   user's editing workflow.
 *
 * Configuration (from ~/.mneme/config.toml [hooks]):
 *   enforce_blast_radius_before_edit = true   # default ON
 *   blast_radius_freshness_seconds   = 600    # 10 min
 */

import { wasBlastRadiusRunFor, getHooksConfig } from "./lib/recent-tool-calls.ts";
import { query } from "../db.ts";
import { errMsg } from "../errors.ts";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PreToolEditWriteArgs {
  tool: string;
  params: Record<string, unknown>;
  sessionId: string;
}

export interface PreToolEditWriteOutput {
  hook_specific: {
    decision: "approve" | "block";
    reason?: string;
  };
}

// ---------------------------------------------------------------------------
// blast_radius auto-run (inline, on block)
// ---------------------------------------------------------------------------

/**
 * Best-effort blast_radius query. Returns a formatted summary string for
 * inclusion in the block message, or a brief error notice.
 *
 * We query via the supervisor IPC layer (db.ts query.raw) which routes to
 * the blast_radius tool handler. This is intentionally a lighter-weight call
 * than the full MCP tool — we only need enough info to give the AI
 * immediate context so it can re-attempt the edit informed.
 */
async function autoRunBlastRadius(filePath: string): Promise<string> {
  try {
    // Timeout: 5 s. We'd rather show a fallback message than stall the hook.
    const TIMEOUT_MS = 5_000;
    const raceTimeout = new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error("blast_radius auto-run timed out")), TIMEOUT_MS),
    );

    type BlastResult = {
      target: string;
      affected_files: string[];
      affected_symbols: string[];
      test_files: string[];
      total_count: number;
      critical_paths: string[];
    };

    const result = await Promise.race([
      query.raw<BlastResult>("tool.blast_radius", {
        target: filePath,
        depth: 1,
        deep: false,
        include_tests: true,
      }),
      raceTimeout,
    ]);

    const lines: string[] = [];
    lines.push(`blast_radius("${result.target}"):`);
    lines.push(`  total affected: ${result.total_count}`);

    if (result.affected_files.length > 0) {
      const shown = result.affected_files.slice(0, 5);
      lines.push(`  affected files (${result.affected_files.length}):`);
      for (const f of shown) lines.push(`    - ${f}`);
      if (result.affected_files.length > 5) {
        lines.push(`    ... and ${result.affected_files.length - 5} more`);
      }
    }

    if (result.critical_paths.length > 0) {
      lines.push(`  critical paths: ${result.critical_paths.slice(0, 3).join(", ")}`);
    }

    if (result.test_files.length > 0) {
      lines.push(`  test files: ${result.test_files.slice(0, 3).join(", ")}`);
    }

    return lines.join("\n");
  } catch (err) {
    // Not a hard failure — blast_radius auto-run is best-effort.
    return `(blast_radius auto-run failed: ${errMsg(err)} — run mcp__mneme__blast_radius manually)`;
  }
}

// ---------------------------------------------------------------------------
// Main hook handler
// ---------------------------------------------------------------------------

/**
 * Determine whether the AI is allowed to proceed with an Edit/Write.
 *
 * Returns approve on any internal error (fail-open).
 */
export async function runPreToolEditWrite(
  args: PreToolEditWriteArgs,
): Promise<PreToolEditWriteOutput> {
  // Fail-open: outer catch handles any unexpected exception.
  try {
    // Only intercept Edit, Write, MultiEdit.
    if (!["Edit", "Write", "MultiEdit"].includes(args.tool)) {
      return approve();
    }

    const cfg = getHooksConfig();
    if (!cfg.enforce_blast_radius_before_edit) {
      // Feature disabled — pass through.
      return approve();
    }

    const filePath = extractFilePath(args.params);
    if (!filePath) {
      // No file path in params — can't enforce, pass through.
      return approve();
    }

    // Check recency.
    const recency = await wasBlastRadiusRunFor(
      filePath,
      args.sessionId,
      cfg.blast_radius_freshness_seconds,
    );

    if (recency.found === true) {
      // blast_radius was already run within the window — allow.
      return approve();
    }

    if (recency.found === "error") {
      // Could not reach mneme (IPC down, daemon absent, etc.). Fail open:
      // never block the user's edit because of a mneme infrastructure failure.
      console.error("[mneme-mcp] pretool-edit-write: recency check failed, failing open");
      return approve();
    }

    // found === false — blast_radius NOT run — block and auto-run it now.
    const blastResult = await autoRunBlastRadius(filePath);

    const reason = buildBlockReason(filePath, blastResult);

    return {
      hook_specific: {
        decision: "block",
        reason,
      },
    };
  } catch (err) {
    // Any exception → fail open. Never block editing because of a mneme bug.
    console.error("[mneme-mcp] pretool-edit-write hook failed (fail-open):", errMsg(err));
    return approve();
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function approve(): PreToolEditWriteOutput {
  return { hook_specific: { decision: "approve" } };
}

/**
 * Extract the target file path from Edit/Write/MultiEdit params.
 * Edit and Write both use `file_path`. MultiEdit uses `file_path` at the
 * top level too (the edits array is nested).
 */
function extractFilePath(params: Record<string, unknown>): string | null {
  const raw = params["file_path"];
  if (typeof raw === "string" && raw.length > 0) return raw;
  return null;
}

function buildBlockReason(filePath: string, blastResult: string): string {
  const lines: string[] = [];
  lines.push(
    `mneme blocked this edit: blast_radius was not run for "${filePath}" in the last 10 minutes.`,
  );
  lines.push("");
  lines.push("Auto-running blast_radius now to give you the context you need:");
  lines.push("");
  lines.push(blastResult);
  lines.push("");
  lines.push(
    "Read the blast_radius output above, then re-attempt your Edit or Write call.",
  );
  lines.push(
    "To disable this check: set enforce_blast_radius_before_edit = false in ~/.mneme/config.toml",
  );
  return lines.join("\n");
}
