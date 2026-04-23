/**
 * MCP tool: snapshot
 *
 * Manual snapshot of the current shard set. Sub-layer 7 (LIFECYCLE).
 */

import {
  SnapshotInput,
  SnapshotOutput,
  type ToolDescriptor,
} from "../types.ts";
import { lifecycle } from "../db.ts";

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
    const result = await lifecycle.snapshot(undefined, input.label);
    return result;
  },
};
