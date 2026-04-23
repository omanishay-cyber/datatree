/**
 * Hook: PreToolUse — Mode C enrichment (design §4.3).
 *
 * For Read: if the file is unchanged since the last read this session,
 * short-circuit with the cached summary instead of re-reading.
 *
 * For Edit/Write: pre-inject relevant constraints (e.g. "no hardcoded
 * colors", "no any types", "named exports only").
 *
 * For Bash: short-circuit if a recent identical run is in tool_cache.db.
 *
 * For Grep/Glob: short-circuit if an equivalent query is cached.
 *
 * Output JSON shape: HookOutput — supports skip+result OR additional_context.
 */

import { query as dbQuery, livebus } from "../db.ts";
import type { Constraint, HookOutput } from "../types.ts";

export interface PreToolArgs {
  tool: string;
  params: Record<string, unknown>;
  sessionId: string;
}

export async function runPreTool(args: PreToolArgs): Promise<HookOutput> {
  const t0 = Date.now();
  try {
    switch (args.tool) {
      case "Read":
        return await handleRead(args);
      case "Edit":
      case "Write":
      case "MultiEdit":
        return await handleEditOrWrite(args);
      case "Bash":
        return await handleBash(args);
      case "Grep":
      case "Glob":
        return await handleSearch(args);
      default:
        return { metadata: { hook: "PreToolUse", duration_ms: Date.now() - t0 } };
    }
  } catch (err) {
    console.error("[mneme-mcp] pre_tool failed:", err);
    return { metadata: { hook: "PreToolUse", error: (err as Error).message } };
  }
}

// ---------------------------------------------------------------------------
// Per-tool handlers
// ---------------------------------------------------------------------------

async function handleRead(args: PreToolArgs): Promise<HookOutput> {
  const filePath = String(args.params.file_path ?? "");
  if (!filePath) return {};

  type ReadCache = {
    hit: boolean;
    content?: string;
    summary?: string;
    hash?: string;
  };
  const cached = await dbQuery
    .raw<ReadCache>("tool_cache.read_lookup", {
      file_path: filePath,
      session_id: args.sessionId,
    })
    .catch((): ReadCache => ({ hit: false }));

  if (cached.hit && cached.content) {
    void livebus.emit("pre_tool.cache_hit", {
      tool: "Read",
      file_path: filePath,
    });
    return {
      skip: true,
      result: cached.content,
      metadata: { source: "tool_cache", hash: cached.hash },
    };
  }
  return {};
}

async function handleEditOrWrite(args: PreToolArgs): Promise<HookOutput> {
  const filePath = String(args.params.file_path ?? "");
  if (!filePath) return {};

  const constraints = await dbQuery
    .raw<Constraint[]>("query.constraints_for_file", { file_path: filePath })
    .catch(() => [] as Constraint[]);

  if (constraints.length === 0) return {};

  // BLOCK on critical-severity constraints (e.g. force-push to main).
  const blockers = constraints.filter(
    (c) => c.enforcement === "block" && c.severity === "critical",
  );
  if (blockers.length > 0) {
    return {
      skip: true,
      result:
        "BLOCKED by mneme constraints:\n" +
        blockers.map((c) => `  - [${c.severity}] ${c.rule}`).join("\n"),
      metadata: { blocked: true, count: blockers.length },
    };
  }

  const lines = constraints
    .slice(0, 8)
    .map((c) => `  - [${c.severity}] ${c.rule}`);
  return {
    additional_context:
      `<mneme-constraints file="${filePath}">\n${lines.join("\n")}\n</mneme-constraints>`,
  };
}

async function handleBash(args: PreToolArgs): Promise<HookOutput> {
  const command = String(args.params.command ?? "");
  if (!command) return {};

  type BashCache = { hit: boolean; output?: string };
  const cached = await dbQuery
    .raw<BashCache>("tool_cache.bash_lookup", {
      command,
      session_id: args.sessionId,
    })
    .catch((): BashCache => ({ hit: false }));

  if (cached.hit && cached.output) {
    return {
      skip: true,
      result: cached.output,
      metadata: { source: "tool_cache" },
    };
  }
  return {};
}

async function handleSearch(args: PreToolArgs): Promise<HookOutput> {
  type SearchCache = { hit: boolean; output?: string };
  const cached = await dbQuery
    .raw<SearchCache>("tool_cache.search_lookup", {
      tool: args.tool,
      params: args.params,
      session_id: args.sessionId,
    })
    .catch((): SearchCache => ({ hit: false }));

  if (cached.hit && cached.output) {
    return {
      skip: true,
      result: cached.output,
      metadata: { source: "tool_cache" },
    };
  }
  return {};
}
