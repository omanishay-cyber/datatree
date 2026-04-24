/**
 * MCP tool: recall_concept
 *
 * v0.1: substring match over `nodes.name` / `nodes.qualified_name` in the
 * graph.db shard. Upgraded in v0.2 to vector similarity once the brain
 * worker pushes embeddings.
 *
 * phase-c10: prefer FTS5 (`searchNodesFts`) for a ~25x speedup vs the LIKE
 * scan. Falls back to the LIKE path when the virtual table is absent or
 * the query yields no FTS hits (graph.db files from before the FTS5
 * migration, or unusual queries the sanitizer rejects).
 */

import {
  RecallConceptInput,
  RecallConceptOutput,
  type Concept,
  type ToolDescriptor,
} from "../types.ts";
import { recallNode, searchNodesFts } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RecallConceptInput.parse>,
  ReturnType<typeof RecallConceptOutput.parse>
> = {
  name: "recall_concept",
  description:
    "Search across extracted symbols (functions, classes, imports) in the project graph. Matches by name or qualified name. Use to discover where a concept lives in the codebase.",
  inputSchema: RecallConceptInput,
  outputSchema: RecallConceptOutput,
  category: "recall",
  async handler(input) {
    try {
      const limit = input.limit ?? 20;
      // Prefer FTS5; fall back to LIKE when the virtual table is missing or
      // the sanitized query doesn't match anything under FTS5's grammar.
      let rows = searchNodesFts(input.query, limit);
      if (rows === null || rows.length === 0) {
        rows = recallNode(input.query, limit);
      }
      const concepts: Concept[] = rows.map((r) => ({
        id: r.qualified_name,
        label: r.qualified_name,
        modality: "code",
        similarity: 1.0,
        community: r.kind,
        community_id: 0,
        source_file: r.file_path,
        source_location: r.file_path ?? "",
        context: r.kind,
      }));
      return { concepts };
    } catch {
      return { concepts: [] };
    }
  },
};
