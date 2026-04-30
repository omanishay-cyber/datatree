/**
 * MCP tool: recall_file
 *
 * Returns the full per-project file state: hash, summary, last-read, blast
 * radius count, test coverage, language. Useful before reading a file to
 * decide if a Read tool call is even necessary.
 *
 * v0.1 (review P2): reads directly from the project's graph.db shard via
 * `bun:sqlite` (see store.ts). Queries:
 *   - `files` by path for hash/size/language/timestamp
 *   - `nodes`+`edges` for the top-N neighbors of this file
 *   - edge count touching the file for blast_radius_count
 *
 * Graceful degrade: if the shard hasn't been built yet, returns
 * `{ exists: false, hash: null, ... }` without throwing.
 */

import {
  RecallFileInput,
  FileState,
  type ToolDescriptor,
} from "../types.ts";
import { blastRadiusCount, fileNodeState, shardDbPath } from "../store.ts";

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
    // Bail early if the graph shard hasn't been built — this is a typed
    // "doesn't exist in index" reply, NOT an exception.
    if (!shardDbPath("graph")) {
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
    }

    const state = fileNodeState(input.path, 10);
    if (!state) {
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
    }

    const blastCount = blastRadiusCount(input.path);

    // Build a short summary from the top neighbors so the caller sees
    // something meaningful without a follow-up query.
    const summary =
      state.neighbors.length > 0
        ? `Top ${state.neighbors.length} neighbor(s): ` +
          state.neighbors
            .slice(0, 5)
            .map((n) => `${n.qualified_name} (${n.edge_kind})`)
            .join(", ")
        : null;

    return {
      path: state.file_path,
      exists: true,
      hash: state.sha256,
      size_bytes: state.byte_count,
      language: state.language,
      summary,
      last_read_at: null,
      last_modified_at: state.last_parsed_at,
      blast_radius_count: blastCount,
      test_coverage: null,
    };
  },
};
