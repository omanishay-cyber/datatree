/**
 * MCP tool: rebuild
 *
 * Re-parse the requested scope from scratch.
 *
 * NEW-019 fix:
 *   1. First try the supervisor's `rebuild` IPC verb (Bucket B wires
 *      this in supervisor/src/ipc.rs). When live, the supervisor kills
 *      every supervised worker for the project and the watchdog respawns
 *      them — `force=true` kills immediately, `force=false` waits for
 *      the current job to drain.
 *   2. On `UnknownVerbError` (verb not routed in this build) we fall
 *      back to spawning `mneme build .` directly.
 *   3. On any other failure (timeout, unreachable) we likewise fall back
 *      to spawning the CLI but record the diagnostic so the caller knows
 *      whether the supervisor was reachable.
 *
 * Safety rail: refuses without `confirm=true`.
 */

import { z } from "zod";
import {
  RebuildInput,
  RebuildOutput,
  type ToolDescriptor,
} from "../types.ts";
import { findProjectRoot, spawnRebuildChild } from "../store.ts";
import { supervisorCommand, UnknownVerbError } from "../db.ts";

// Additive extension — keep original shape intact.
const RebuildOutputExtended = RebuildOutput.extend({
  note: z.string().optional(),
  pid: z.number().int().nullable().optional(),
});

type Input = ReturnType<typeof RebuildInput.parse>;
type Output = z.infer<typeof RebuildOutputExtended>;

/** Wire shape returned by Bucket B's `Rebuild` verb. */
interface RebuildReply {
  response: "rebuild_acked";
  workers: string[];
  force: boolean;
}

export const tool: ToolDescriptor<Input, Output> = {
  name: "rebuild",
  description:
    "Re-parse the requested scope from scratch (graph | semantic | all). Last resort — clears existing data and rebuilds. Requires confirm=true.",
  inputSchema: RebuildInput,
  outputSchema: RebuildOutputExtended,
  category: "health",
  async handler(input) {
    if (!input.confirm) {
      throw new Error(
        "rebuild: refused without confirm=true (this clears existing data)",
      );
    }
    const t0 = Date.now();

    // ---- 1) Supervisor IPC path (NEW-019) -------------------------------
    const projectRoot = findProjectRoot(process.cwd());
    if (projectRoot) {
      try {
        const reply = await supervisorCommand<RebuildReply>("rebuild", {
          project_id: projectRoot,
          force: input.scope === "all",
        });
        return {
          rebuilt: reply.workers,
          duration_ms: Date.now() - t0,
          note: `supervisor: killed ${reply.workers.length} worker(s) (force=${reply.force})`,
          pid: null,
        };
      } catch (err) {
        if (err instanceof UnknownVerbError) {
          // Verb not yet routed in this build — fall through to local.
        } else {
          const msg = err instanceof Error ? err.message : String(err);
          // Supervisor unreachable / timed out — record + fall through to
          // local spawn so the user still gets the rebuild done.
          try {
            const spawned = spawnRebuildChild(input.scope);
            if (spawned.spawned) {
              return {
                rebuilt: [input.scope],
                duration_ms: Date.now() - t0,
                note: `supervisor unreachable (${msg}); local:spawned ${spawned.command}`,
                pid: spawned.pid,
              };
            }
          } catch {
            // Fall through to error return below.
          }
          return {
            rebuilt: [],
            duration_ms: Date.now() - t0,
            note: `supervisor unreachable (${msg}); local-spawn failed`,
            pid: null,
          };
        }
      }
    }

    // ---- 2) Local CLI spawn (graceful degrade) --------------------------
    try {
      const spawned = spawnRebuildChild(input.scope);
      if (spawned.spawned) {
        return {
          rebuilt: [input.scope],
          duration_ms: Date.now() - t0,
          note: `local:spawned ${spawned.command} (verb not yet routed in this build)`,
          pid: spawned.pid,
        };
      }
      return {
        rebuilt: [],
        duration_ms: Date.now() - t0,
        note: "local:spawn-failed (mneme CLI missing?)",
        pid: null,
      };
    } catch {
      return {
        rebuilt: [],
        duration_ms: Date.now() - t0,
        note: "local:error",
        pid: null,
      };
    }
  },
};
