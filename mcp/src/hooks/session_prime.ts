/**
 * Hook: SessionStart — Mode A primer (design §4.1).
 *
 * Composes a 1.5K-token "project primer" injected as a system reminder
 * to seed Claude Code with the active goal, top constraints, open TODOs,
 * recent decisions, dirty files, and red drift findings.
 *
 * Output JSON shape: { additional_context: string }
 */

import { buildPrimer } from "../composer.ts";
import { livebus } from "../db.ts";
import type { HookOutput } from "../types.ts";

export interface SessionPrimeArgs {
  project: string;
  sessionId: string;
}

export async function runSessionPrime(args: SessionPrimeArgs): Promise<HookOutput> {
  const t0 = Date.now();
  try {
    const primer = await buildPrimer({
      cwd: args.project,
      sessionId: args.sessionId,
    });

    void livebus.emit("session.primed", {
      session_id: args.sessionId,
      project: args.project,
      bytes: primer.length,
      duration_ms: Date.now() - t0,
    });

    return {
      additional_context: primer,
      metadata: {
        hook: "SessionStart",
        duration_ms: Date.now() - t0,
        session_id: args.sessionId,
      },
    };
  } catch (err) {
    // Hooks must NEVER crash the harness — return an empty-context result
    // and log to stderr so the supervisor sees it.
    console.error("[mneme-mcp] session_prime failed:", err);
    return {
      additional_context: "",
      metadata: { hook: "SessionStart", error: errMsg(err) },
    };
  }
}
