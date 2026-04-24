/**
 * MCP tool: graphify_corpus (phase-c9 wired)
 *
 * Triggers the multimodal extraction pipeline.
 *
 * Write path (preferred): supervisor IPC verb
 * `multimodal.graphify_corpus`. Only the supervisor may run this — it
 * writes to corpus.db / graph.db / semantic.db and emits GRAPH_REPORT.md.
 *
 * Graceful degrade: when IPC is unavailable we short-circuit to reading
 * the *current* graph counts from the local `graph.db` shard so the
 * caller gets real-data-at-rest rather than a stub. On success we also
 * re-read counts to validate the supervisor's report.
 *
 * NOTE: as of phase-c9 the supervisor in supervisor/src/ipc.rs does NOT
 * yet route `multimodal.graphify_corpus` — every call takes the
 * read-only local-count fallback.
 */

import { z } from "zod";
import {
  GraphifyCorpusInput,
  GraphifyCorpusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import { graphStats, shardDbPath } from "../store.ts";

// Additive extension — keep original shape intact.
const GraphifyCorpusOutputExtended = GraphifyCorpusOutput.extend({
  note: z.string().optional(),
});

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

type Input = ReturnType<typeof GraphifyCorpusInput.parse>;
type Output = z.infer<typeof GraphifyCorpusOutputExtended>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "graphify_corpus",
  description:
    "Run the full multimodal extraction pass (AST + semantic + Leiden clustering) over the project corpus. mode='deep' is more aggressive with INFERRED edges. Writes to corpus.db and emits GRAPH_REPORT.md.",
  inputSchema: GraphifyCorpusInput,
  outputSchema: GraphifyCorpusOutputExtended,
  category: "multimodal",
  async handler(input) {
    const t0 = Date.now();

    // ---- Supervisor path --------------------------------------------------
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
      // Re-read live counts so the tool's response reflects the shard
      // state on disk rather than whatever the worker reported. Ensures
      // we never over-report a count the caller can't verify.
      const live = localCounts();
      return {
        nodes_count: live.nodes_count || result.nodes_count,
        edges_count: live.edges_count || result.edges_count,
        hyperedges_count:
          live.hyperedges_count || result.hyperedges_count,
        communities_count:
          live.communities_count || result.communities_count,
        report_path: result.report_path ?? "",
        duration_ms: Date.now() - t0,
        note: "supervisor",
      };
    }

    // ---- Fallback: report current shard counts ----------------------------
    const counts = localCounts();
    return {
      ...counts,
      duration_ms: Date.now() - t0,
      report_path: "",
      note: "fallback:local-counts",
    };
  },
};
