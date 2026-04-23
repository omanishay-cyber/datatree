/**
 * MCP tool: recall_file
 *
 * Returns the full per-project file state: hash, summary, last-read, blast
 * radius count, test coverage, language. Useful before reading a file to
 * decide if a Read tool call is even necessary.
 */

import {
  RecallFileInput,
  FileState,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RecallFileInput.parse>,
  ReturnType<typeof FileState.parse>
> = {
  name: "recall_file",
  description:
    "Get full file state: hash, summary, last-read timestamp, blast-radius count, test coverage, language. Returns null hash if the file does not exist. Use BEFORE Read to decide if you can skip the Read entirely (file unchanged since last read).",
  inputSchema: RecallFileInput,
  outputSchema: FileState,
  category: "recall",
  async handler(input) {
    const result = await dbQuery
      .raw<ReturnType<typeof FileState.parse>>("query.recall_file", {
        path: input.path,
      })
      .catch(() => null);

    if (result) return result;

    // Fall back to a "doesn't exist in index" reply.
    return {
      path: input.path,
      exists: false,
      hash: null,
      size_bytes: null,
      language: null,
      summary: null,
      last_read_at: null,
      last_modified_at: null,
      blast_radius_count: null,
      test_coverage: null,
    };
  },
};
