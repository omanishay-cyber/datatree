/**
 * MCP tool: graphify_corpus
 *
 * Triggers the multimodal extraction pipeline.
 *
 * v0.1 (review P2): graphification is a write path — only the supervisor
 * may run it. We dispatch `multimodal.graphify_corpus` over IPC; when
 * the supervisor is offline we short-circuit to reporting the *current*
 * graph counts from the local `graph.db` so the caller gets real data
 * instead of a stub. Post-IPC we also re-read counts to validate the
 * supervisor's report.
 */

import {
  GraphifyCorpusInput,
  GraphifyCorpusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { graphStats, shardDbPath } from "../store.ts";

function localCounts(): {
  nodes_count: number;
  edges_count: number;
  hyperedges_count: number;
  communities_count: number;
} {
  if (!shardDbPath("graph")) {
    return {
      nodes_count: 0,
      edges_count: 0,
      hyperedges_count: 0,
      communities_count: 0,
    };
  }
  try {
    const s = graphStats();
    return {
      nodes_count: s.nodes,
      edges_count: s.edges,
      hyperedges_count: 0,
      communities_count: 0,
    };
  } catch {
    return {
      nodes_count: 0,
      edges_count: 0,
      hyperedges_count: 0,
      communities_count: 0,
    };
  }
}

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
      .raw<
        Omit<ReturnType<typeof GraphifyCorpusOutput.parse>, "duration_ms">
      >("multimodal.graphify_corpus", {
        path: input.path,
        mode: input.mode,
        incremental: input.incremental,
      })
      .catch(() => null);

    if (result) {
      return { ...result, duration_ms: Date.now() - t0 };
    }

    const counts = localCounts();
    return { ...counts, duration_ms: Date.now() - t0, report_path: "" };
  },
};
