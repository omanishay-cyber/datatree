/**
 * MCP tool: graphify_corpus
 *
 * Triggers the multimodal extraction pipeline (AST + semantic + clustering)
 * over the project corpus. Returns counts and the on-disk report path.
 */

import {
  GraphifyCorpusInput,
  GraphifyCorpusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof GraphifyCorpusInput.parse>,
  ReturnType<typeof GraphifyCorpusOutput.parse>
> = {
  name: "graphify_corpus",
  description:
    "Run the full multimodal extraction pass (AST + semantic + Leiden clustering) over the project corpus. mode='deep' is more aggressive with INFERRED edges. Writes to corpus.db and emits GRAPH_REPORT.md.",
  inputSchema: GraphifyCorpusInput,
  outputSchema: GraphifyCorpusOutput,
  category: "multimodal",
  async handler(input) {
    const t0 = Date.now();
    const result = await dbQuery
      .raw<Omit<ReturnType<typeof GraphifyCorpusOutput.parse>, "duration_ms">>(
        "multimodal.graphify_corpus",
        {
          path: input.path,
          mode: input.mode,
          incremental: input.incremental,
        },
      )
      .catch(() => null);

    if (result) {
      return { ...result, duration_ms: Date.now() - t0 };
    }
    return {
      nodes_count: 0,
      edges_count: 0,
      hyperedges_count: 0,
      communities_count: 0,
      duration_ms: Date.now() - t0,
      report_path: "",
    };
  },
};
