/**
 * MCP tool: compare
 *
 * Diff two snapshots: files added/removed/modified, decisions added,
 * findings resolved/introduced.
 */

import {
  CompareInput,
  CompareOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof CompareInput.parse>,
  ReturnType<typeof CompareOutput.parse>
> = {
  name: "compare",
  description:
    "Diff two snapshots. Returns files added/removed/modified counts, decisions added, findings resolved/introduced. Use to understand what changed across a range of time.",
  inputSchema: CompareInput,
  outputSchema: CompareOutput,
  category: "time",
  async handler(input) {
    const diff = await dbQuery
      .raw<ReturnType<typeof CompareOutput.parse>["diff"]>(
        "lifecycle.compare_snapshots",
        { snapshot_a: input.snapshot_a, snapshot_b: input.snapshot_b },
      )
      .catch(() => ({
        files_added: [],
        files_removed: [],
        files_modified: [],
        decisions_added: 0,
        findings_resolved: 0,
        findings_introduced: 0,
      }));
    return { diff };
  },
};
