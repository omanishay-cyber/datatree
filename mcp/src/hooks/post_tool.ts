/**
 * Hook: PostToolUse — Mode D capture (design §4.4).
 *
 * Records every tool call to history.db and tool_cache.db:
 *   - tool name + parameters (verbatim)
 *   - full result (read from $TOOL_RESULT_PATH file)
 *   - file diffs for Edit/Write
 *   - timestamp + session_id
 *
 * Fire-and-forget: returns immediately; capture happens asynchronously.
 */

import { readFile } from "node:fs/promises";
import { inject, livebus } from "../db.ts";
import type { HookOutput } from "../types.ts";

export interface PostToolArgs {
  tool: string;
  resultPath: string;
  sessionId: string;
  /** Optional verbatim params; falls back to undefined if not provided. */
  params?: unknown;
}

export async function runPostTool(args: PostToolArgs): Promise<HookOutput> {
  // Don't await — capture is best-effort and must never delay the next turn.
  void capture(args);
  return { metadata: { hook: "PostToolUse" } };
}

async function capture(args: PostToolArgs): Promise<void> {
  try {
    let result = "";
    if (args.resultPath) {
      try {
        result = await readFile(args.resultPath, "utf8");
      } catch {
        result = "";
      }
    }

    const timestamp = new Date().toISOString();
    const row = {
      tool: args.tool,
      params_json: JSON.stringify(args.params ?? null),
      result,
      result_size: result.length,
      session_id: args.sessionId,
      timestamp,
    };

    await Promise.all([
      inject.insert("history", row, {
        idempotency_key: `${args.sessionId}-${args.tool}-${timestamp}`,
        emit_event: false,
        audit: false,
      }),
      inject.upsert(
        "tool_cache",
        {
          tool: args.tool,
          params_json: row.params_json,
          result,
          session_id: args.sessionId,
          cached_at: timestamp,
        },
        { idempotency_key: `cache-${args.tool}-${row.params_json}` },
      ),
    ]);

    void livebus.emit("tool.captured", {
      tool: args.tool,
      session_id: args.sessionId,
      result_size: result.length,
    });
  } catch (err) {
    console.error("[mneme-mcp] post_tool capture failed:", err);
  }
}
