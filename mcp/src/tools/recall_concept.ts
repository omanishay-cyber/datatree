/**
 * MCP tool: recall_concept
 *
 * v0.1: substring match over `nodes.name` / `nodes.qualified_name` in the
 * graph.db shard. Upgraded in v0.2 to vector similarity once the brain
 * worker pushes embeddings.
 */

import {
  RecallConceptInput,
  RecallConceptOutput,
  type Concept,
  type ToolDescriptor,
} from "../types.ts";
import { recallNode } from "../store.ts";

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
      const rows = recallNode(input.query, input.limit ?? 20);
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
