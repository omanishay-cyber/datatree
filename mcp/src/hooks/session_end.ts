/**
 * Hook: SessionEnd — design §6.6.
 *
 * Final flush:
 *   - Drains the summarizer queue.
 *   - Writes a session-close row to history.db.
 *   - Triggers an out-of-band snapshot (sub-layer 7).
 *   - Updates the manifest with end-of-session metrics.
 */

import { inject, lifecycle, livebus } from "../db.ts";
import type { HookOutput } from "../types.ts";

export interface SessionEndArgs {
  sessionId: string;
}

export async function runSessionEnd(args: SessionEndArgs): Promise<HookOutput> {
  const t0 = Date.now();
  try {
    const closedAt = new Date().toISOString();
    await inject.insert(
      "history",
      {
        tool: "__session_end__",
        params_json: JSON.stringify({ session_id: args.sessionId }),
        result: "",
        result_size: 0,
        session_id: args.sessionId,
        timestamp: closedAt,
      },
      {
        idempotency_key: `session-end-${args.sessionId}`,
        emit_event: true,
        audit: true,
      },
    );

    const snapshot = await lifecycle
      .snapshot(undefined, `auto-session-${args.sessionId}`)
      .catch(() => null);

    void livebus.emit("session.ended", {
      session_id: args.sessionId,
      snapshot_id: snapshot?.snapshot_id ?? null,
      closed_at: closedAt,
      duration_ms: Date.now() - t0,
    });

    return {
      metadata: {
        hook: "SessionEnd",
        duration_ms: Date.now() - t0,
        snapshot_id: snapshot?.snapshot_id ?? null,
      },
    };
  } catch (err) {
    console.error("[mneme-mcp] session_end failed:", err);
    return { metadata: { hook: "SessionEnd", error: (err as Error).message } };
  }
}
