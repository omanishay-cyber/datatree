/**
 * MCP tool: snapshot (phase-c9 wired)
 *
 * Manual point-in-time snapshot of the current shard set.
 *
 * Write path (preferred): supervisor IPC verb `lifecycle.snapshot`. The
 * supervisor pauses writes briefly and copies every `<layer>.db` into
 * `~/.mneme/projects/<id>/snapshots/<timestamp>/`.
 *
 * Graceful degrade: when IPC is unavailable we fall back to
 * `snapshotFsFallback` in store.ts which performs the copy locally
 * using SQLite's online-backup API (`VACUUM INTO`). Read-only WAL
 * concurrency means this is safe for readers; a concurrent writer
 * could produce a slightly-inconsistent snapshot across shards, which
 * is acceptable for the dev-tool fallback.
 *
 * NOTE: as of phase-c9 the supervisor in supervisor/src/ipc.rs does NOT
 * yet route `lifecycle.snapshot` — every call currently takes the
 * filesystem-copy fallback.
 */

import { z } from "zod";
import {
  SnapshotInput,
  SnapshotOutput,
  type ToolDescriptor,
} from "../types.ts";
import { lifecycle } from "../db.ts";
import { listSnapshotsFs, snapshotFsFallback } from "../store.ts";

// Additive extension — keep original shape intact.
const SnapshotOutputExtended = SnapshotOutput.extend({
  note: z.string().optional(),
});

type Input = ReturnType<typeof SnapshotInput.parse>;
type Output = z.infer<typeof SnapshotOutputExtended>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "snapshot",
  description:
    "Take a manual snapshot of the current project shards. Returns snapshot_id, created_at, size_bytes. Snapshots are also taken hourly automatically.",
  inputSchema: SnapshotInput,
  outputSchema: SnapshotOutputExtended,
  category: "time",
  async handler(input) {
    // ---- Supervisor path --------------------------------------------------
    try {
      const r = await lifecycle.snapshot(undefined, input.label);
      return { ...r, note: "supervisor" };
    } catch {
      // Fall through to filesystem copy.
    }

    // ---- Fallback: local VACUUM INTO per shard ----------------------------
    try {
      const fs = snapshotFsFallback(input.label);
      if (fs) {
        return { ...fs, note: "fallback:vacuum-into" };
      }
    } catch {
      // Fall through to listing existing snapshots.
    }

    // ---- Last resort: surface most recent existing snapshot ---------------
    const existing = listSnapshotsFs();
    if (existing.length > 0) {
      const first = existing[0];
      if (first) {
        return {
          snapshot_id: first.id,
          created_at: first.captured_at,
          size_bytes: first.bytes,
          note: "fallback:existing",
        };
      }
    }
    return {
      snapshot_id: "",
      created_at: new Date().toISOString(),
      size_bytes: 0,
      note: "fallback:empty",
    };
  },
};
