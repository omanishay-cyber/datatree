/**
 * MCP tool: rewind
 *
 * Returns the content of a file at a past point in time. Pulls from the
 * snapshot+WAL store (sub-layer 7).
 */

import {
  RewindInput,
  RewindOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RewindInput.parse>,
  ReturnType<typeof RewindOutput.parse>
> = {
  name: "rewind",
  description:
    "Show file content as of a past timestamp (RFC3339) or named snapshot id. Returns content + hash. Use to compare current state against any historical version.",
  inputSchema: RewindInput,
  outputSchema: RewindOutput,
  category: "time",
  async handler(input) {
    const result = await dbQuery
      .raw<{ content: string; hash: string }>("lifecycle.rewind_file", {
        file: input.file,
        when: input.when,
      })
      .catch(() => ({ content: "", hash: "" }));

    return {
      file: input.file,
      when: input.when,
      content: result.content,
      hash: result.hash,
    };
  },
};
