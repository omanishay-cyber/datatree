/**
 * Hook: PreToolUse (Grep / Read / Glob) — Layer 3 of the v0.4.0 self-ping
 * enforcement layer.
 *
 * STATUS: SKELETON — deferred to v0.4.1.
 *
 * Design intent (v0.4.1):
 *   When the AI calls Grep or Read on a file/pattern X, check whether a
 *   mneme_recall / blast_radius query was already run for the same term in
 *   this session. If NOT, redirect: call mcp__mneme__mneme_recall first and
 *   inject its result as additionalContext. Fall back to native grep/read if
 *   mneme returns empty or times out.
 *
 * Why deferred:
 *   - Layer 1 + 2 cover the highest-impact surfaces (prompt nudge + edit gate).
 *   - Layer 3 needs careful UX work: a hard block on every Read is too
 *     aggressive (e.g. reading a config file, reading a test fixture). The
 *     v0.4.1 implementation will use a soft-redirect pattern (additionalContext
 *     injection, not a block) so the AI sees mneme results first but can still
 *     proceed with the native Read.
 *   - Deferred keeps the v0.4.0 scope tight and pre-push gates green.
 *
 * Hook output protocol:
 *   Always returns approve (exit 0). No blocking in this skeleton.
 *
 * Configuration (from ~/.mneme/config.toml [hooks]):
 *   enforce_recall_before_grep = false   # default OFF in v0.4.0
 */

import { wasMnemeRecallRunFor, getHooksConfig } from "./lib/recent-tool-calls.ts";
import { errMsg } from "../errors.ts";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PreToolGrepReadArgs {
  tool: string;
  params: Record<string, unknown>;
  sessionId: string;
}

export interface PreToolGrepReadOutput {
  hook_specific: {
    decision: "approve" | "block";
    /** Optional soft-redirect hint placed into additionalContext. */
    additionalContext?: string;
  };
}

// ---------------------------------------------------------------------------
// Main hook handler (skeleton — deferred to v0.4.1)
// ---------------------------------------------------------------------------

/**
 * v0.4.0 skeleton: always approves.
 *
 * v0.4.1 will add the soft-redirect logic:
 *   1. Extract the search term / file path from params.
 *   2. Call wasMnemeRecallRunFor to check recency.
 *   3. If NOT found: query mneme_recall inline, inject result as
 *      additionalContext, then approve (never block Read/Grep).
 *   4. If found: approve with no modification.
 */
export async function runPreToolGrepRead(
  args: PreToolGrepReadArgs,
): Promise<PreToolGrepReadOutput> {
  // Fail-open: outer catch returns approve on any exception.
  try {
    const cfg = getHooksConfig();

    // Feature disabled by default in v0.4.0.
    if (!cfg.enforce_recall_before_grep) {
      return approve();
    }

    // v0.4.1 TODOs:
    //   - Extract search term / path from args.params.
    //   - Call wasMnemeRecallRunFor(searchTerm, args.sessionId).
    //   - If not found: inline mneme_recall call, additionalContext injection.
    //   - Return approve with optional additionalContext.

    // For now: pass through.
    void wasMnemeRecallRunFor; // referenced so import is not dead code flagged
    void args; // args used in v0.4.1
    return approve();
  } catch (err) {
    console.error("[mneme-mcp] pretool-grep-read hook failed (fail-open):", errMsg(err));
    return approve();
  }
}

function approve(): PreToolGrepReadOutput {
  return { hook_specific: { decision: "approve" } };
}
