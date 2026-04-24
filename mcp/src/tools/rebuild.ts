/**
 * MCP tool: rebuild (phase-c9 wired)
 *
 * Re-parse the requested scope from scratch.
 *
 * Write path (preferred): supervisor IPC verb `lifecycle.rebuild`. Runs
 * `mneme rebuild <scope>` under the supervisor's single-writer lock.
 *
 * Graceful degrade: when IPC is unavailable we fall back to spawning
 * `mneme build .` as a detached child process via `spawnRebuildChild`
 * (store.ts). We return immediately with the pid because builds are
 * long-running — callers should not block.
 *
 * Safety rail: we still refuse without `confirm=true` regardless of path.
 *
 * NOTE: as of phase-c9 the supervisor in supervisor/src/ipc.rs does NOT
 * yet route `lifecycle.rebuild` — every call currently takes the
 * child-process fallback.
 */

import { z } from "zod";
import {
  RebuildInput,
  RebuildOutput,
  type ToolDescriptor,
} from "../types.ts";
import { lifecycle } from "../db.ts";
import { spawnRebuildChild } from "../store.ts";

// Additive extension — keep original shape intact.
const RebuildOutputExtended = RebuildOutput.extend({
  note: z.string().optional(),
  pid: z.number().int().nullable().optional(),
});

type Input = ReturnType<typeof RebuildInput.parse>;
type Output = z.infer<typeof RebuildOutputExtended>;

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

    // ---- Supervisor path --------------------------------------------------
    try {
      const r = await lifecycle.rebuild(input.scope);
      return { ...r, note: "supervisor" };
    } catch {
      // Fall through to child-process spawn.
    }

    // ---- Fallback: spawn `mneme build .` ----------------------------------
    try {
      const spawned = spawnRebuildChild(input.scope);
      if (spawned.spawned) {
        return {
          rebuilt: [input.scope],
          duration_ms: Date.now() - t0,
          note: `fallback:spawned ${spawned.command}`,
          pid: spawned.pid,
        };
      }
      return {
        rebuilt: [],
        duration_ms: Date.now() - t0,
        note: "fallback:spawn-failed (mneme CLI missing?)",
        pid: null,
      };
    } catch {
      return {
        rebuilt: [],
        duration_ms: Date.now() - t0,
        note: "fallback:error",
        pid: null,
      };
    }
  },
};
