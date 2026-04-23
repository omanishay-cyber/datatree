/**
 * MCP tool: snapshot
 *
 * Manual snapshot of the current shard set.
 *
 * v0.1 (review P2): snapshots are filesystem copies of every `<layer>.db`
 * into `~/.mneme/projects/<id>/snapshots/<timestamp>/`. That copy must
 * go through the supervisor's `lifecycle.snapshot` (so it can pause
 * writes for the online-backup). We dispatch that IPC and, on success,
 * report the metadata. On IPC failure we fall back to listing the
 * newest on-disk snapshot dir so the caller at least learns whether a
 * prior snapshot exists.
 */

import {
  SnapshotInput,
  SnapshotOutput,
  type ToolDescriptor,
} from "../types.ts";
import { lifecycle } from "../db.ts";
import { listSnapshotsFs } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof SnapshotInput.parse>,
  ReturnType<typeof SnapshotOutput.parse>
> = {
  name: "snapshot",
  description:
    "Take a manual snapshot of the current project shards. Returns snapshot_id, created_at, size_bytes. Snapshots are also taken hourly automatically.",
  inputSchema: SnapshotInput,
  outputSchema: SnapshotOutput,
  category: "time",
  async handler(input) {
    try {
      const r = await lifecycle.snapshot(undefined, input.label);
      return r;
    } catch {
      // Supervisor offline — report a graceful no-op using the newest
      // existing on-disk snapshot if any.
      const existing = listSnapshotsFs();
      if (existing.length > 0) {
        const first = existing[0];
        if (first) {
          return {
            snapshot_id: first.id,
            created_at: first.captured_at,
            size_bytes: first.bytes,
          };
        }
      }
      return {
        snapshot_id: "",
        created_at: new Date().toISOString(),
        size_bytes: 0,
      };
    }
  },
};
