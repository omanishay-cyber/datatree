/**
 * Hook: Stop (between turns) — design §6.5.
 *
 * Triggers the summarizer and updates the Step Ledger drift score for the
 * active step. Also fires the drift-hunter subagent for incremental scans.
 */

import { query as dbQuery, livebus } from "../db.ts";
import type { HookOutput } from "../types.ts";

export interface TurnEndArgs {
  sessionId: string;
}

export async function runTurnEnd(args: TurnEndArgs): Promise<HookOutput> {
  const t0 = Date.now();
  try {
    const [summarizerResult, driftResult] = await Promise.all([
      dbQuery
        .raw<{ summary_id: string; tokens: number }>("summarizer.run", {
          session_id: args.sessionId,
        })
        .catch(() => null),
      dbQuery
        .raw<{ drift_score_delta: number; current_step_id: string | null }>(
          "drift.update_step_score",
          { session_id: args.sessionId },
        )
        .catch(() => null),
    ]);

    void livebus.emit("turn.ended", {
      session_id: args.sessionId,
      summary_id: summarizerResult?.summary_id ?? null,
      drift_delta: driftResult?.drift_score_delta ?? 0,
      duration_ms: Date.now() - t0,
    });

    return {
      metadata: {
        hook: "Stop",
        duration_ms: Date.now() - t0,
        drift_delta: driftResult?.drift_score_delta ?? 0,
        current_step_id: driftResult?.current_step_id ?? null,
      },
    };
  } catch (err) {
    console.error("[datatree-mcp] turn_end failed:", err);
    return { metadata: { hook: "Stop", error: (err as Error).message } };
  }
}
