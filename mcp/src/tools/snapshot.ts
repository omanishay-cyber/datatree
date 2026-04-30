/**
 * MCP tool: snapshot
 *
 * Manual point-in-time snapshot of the current shard set.
 *
 * NEW-019 fix:
 *   1. First try the supervisor's `snapshot` IPC verb (Bucket B wires
 *      this in supervisor/src/ipc.rs). When live, the supervisor pauses
 *      writes briefly and copies every `<layer>.db` into
 *      `~/.mneme/projects/<id>/snapshots/<timestamp>/`.
 *   2. On `UnknownVerbError` (verb not routed in this build) we fall
 *      back to a local SQLite `VACUUM INTO` per shard via
 *      `snapshotFsFallback`. WAL concurrency makes that safe for readers;
 *      a concurrent writer could produce a slightly-inconsistent snapshot
 *      across shards, which is acceptable for the dev-tool path.
 *   3. On any other failure (timeout, unreachable) we likewise fall back
 *      to local but record the diagnostic. As a last resort we surface
 *      the most recent existing snapshot so callers always get *some*
 *      stable id back rather than an empty result.
 */

import { z } from "zod";
import {
  SnapshotInput,
  SnapshotOutput,
  type ToolDescriptor,
} from "../types.ts";
import {
  findProjectRoot,
  listSnapshotsFs,
  snapshotFsFallback,
} from "../store.ts";
import { supervisorCommand, UnknownVerbError } from "../db.ts";

// Additive extension — keep original shape intact.
const SnapshotOutputExtended = SnapshotOutput.extend({
  note: z.string().optional(),
});

type Input = ReturnType<typeof SnapshotInput.parse>;
type Output = z.infer<typeof SnapshotOutputExtended>;

/**
 * Wire shape returned by Bucket B's `Snapshot` verb. The supervisor
 * doesn't yet expose the snapshot id / size directly — for now it returns
 * worker + queue snapshots — so when the supervisor path succeeds we
 * still take the local snapshot to capture the actual disk state and
 * tag the note as `supervisor+local` so the caller knows both ran.
 */
interface SnapshotReply {
  response: "snapshot_combined";
  scope: string;
}

export const tool: ToolDescriptor<Input, Output> = {
  name: "snapshot",
  description:
    "Take a manual snapshot of the current project shards. Returns snapshot_id, created_at, size_bytes. Snapshots are also taken hourly automatically.",
  inputSchema: SnapshotInput,
  outputSchema: SnapshotOutputExtended,
  category: "time",
  async handler(input) {
    let supervisorNote: string | null = null;

    // ---- 1) Supervisor IPC path (NEW-019) -------------------------------
    const projectRoot = findProjectRoot(process.cwd());
    if (projectRoot) {
      try {
        const reply = await supervisorCommand<SnapshotReply>("snapshot", {
          project_id: projectRoot,
          scope: "all",
        });
        supervisorNote = `supervisor: scope=${reply.scope}`;
      } catch (err) {
        if (err instanceof UnknownVerbError) {
          // Verb not yet routed in this build — fall through to local.
        } else {
          const msg = err instanceof Error ? err.message : String(err);
          supervisorNote = `supervisor unreachable (${msg})`;
        }
      }
    }

    // ---- 2) Local VACUUM INTO per shard ---------------------------------
    try {
      const fs = snapshotFsFallback(input.label);
      if (fs) {
        const note = supervisorNote
          ? `${supervisorNote}; local:vacuum-into`
          : "local:vacuum-into (verb not yet routed in this build)";
        return { ...fs, note };
      }
    } catch {
      // Fall through to listing existing snapshots.
    }

    // ---- 3) Last resort: surface most recent existing snapshot ----------
    const existing = listSnapshotsFs();
    if (existing.length > 0) {
      const first = existing[0];
      if (first) {
        return {
          snapshot_id: first.id,
          created_at: first.captured_at,
          size_bytes: first.bytes,
          note: supervisorNote
            ? `${supervisorNote}; local:existing`
            : "local:existing",
        };
      }
    }
    return {
      snapshot_id: "",
      created_at: new Date().toISOString(),
      size_bytes: 0,
      note: supervisorNote
        ? `${supervisorNote}; local:empty`
        : "local:empty (verb not yet routed in this build)",
    };
  },
};
