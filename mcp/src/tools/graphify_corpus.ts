/**
 * MCP tool: graphify_corpus
 *
 * Triggers the multimodal extraction pipeline.
 *
 * NEW-019 fix:
 *   1. First try the supervisor's `graphify_corpus` IPC verb (Bucket B
 *      wires this in supervisor/src/ipc.rs). The supervisor enumerates
 *      the project's md/txt corpus and queues an `Job::Ingest` per file
 *      onto the shared job queue. Returns the queued count immediately.
 *   2. On `UnknownVerbError` (the supervisor returned `BadRequest`,
 *      meaning the verb isn't routed in this build) we transparently
 *      fall back to reporting current local shard counts so the tool
 *      still produces meaningful data on the way to v0.3.1.
 *   3. On any other failure (timeout, unreachable daemon) we likewise
 *      surface the local shard counts plus a diagnostic note. The tool
 *      never throws — graceful degrade is the contract.
 */

import { z } from "zod";
import {
  GraphifyCorpusInput,
  GraphifyCorpusOutput,
  type ToolDescriptor,
} from "../types.ts";
import { graphStats, findProjectRoot, shardDbPath } from "../store.ts";
import { supervisorCommand, UnknownVerbError } from "../db.ts";

// Additive extension — keep original shape intact.
//
// Phase A B2: when no `graph` shard exists for the cwd we surface a structured
// error envelope up-front instead of timing out on a 30s supervisor RPC. The
// caller can then suggest `mneme build .` immediately.
const GraphifyCorpusOutputExtended = GraphifyCorpusOutput.extend({
  note: z.string().optional(),
  error: z.literal("shard_missing").optional(),
  message: z.string().optional(),
  suggested_action: z.string().optional(),
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

/** Wire shape returned by `Snapshot/SnapshotCombined` once Bucket B's verb lands. */
interface GraphifyCorpusReply {
  response: "graphify_corpus_queued";
  queued: number;
  project: string;
}

export const tool: ToolDescriptor<Input, Output> = {
  name: "graphify_corpus",
  description:
    "Run the full multimodal extraction pass (AST + semantic + Leiden clustering) over the project corpus. mode='deep' is more aggressive with INFERRED edges. Writes to corpus.db and emits GRAPH_REPORT.md.",
  inputSchema: GraphifyCorpusInput,
  outputSchema: GraphifyCorpusOutputExtended,
  category: "multimodal",
  async handler(_input) {
    const t0 = Date.now();

    // ---- 0) Up-front shard check (Phase A B2) ----------------------------
    // If no graph shard exists for this cwd, the supervisor RPC will spin
    // for 30s and silently return zeros. Detect missing shard now and tell
    // the caller exactly what to do.
    if (!shardDbPath("graph")) {
      return {
        nodes_count: 0,
        edges_count: 0,
        hyperedges_count: 0,
        communities_count: 0,
        duration_ms: Date.now() - t0,
        report_path: "",
        error: "shard_missing",
        message:
          "No mneme graph shard found for the current project. Run `mneme build .` first to index the project, then re-invoke graphify_corpus.",
        suggested_action: "mneme build .",
      };
    }

    // ---- 1) Supervisor IPC path (NEW-019) -------------------------------
    const projectRoot = findProjectRoot(process.cwd());
    if (projectRoot) {
      try {
        const reply = await supervisorCommand<GraphifyCorpusReply>(
          "graphify_corpus",
          { project_id: projectRoot },
        );
        // Once the supervisor has queued the work, surface the queued
        // count alongside the live shard snapshot so the caller has both
        // a "throughput in flight" and "current totals" signal.
        const counts = localCounts();
        return {
          ...counts,
          duration_ms: Date.now() - t0,
          report_path: "",
          note: `supervisor: queued ${reply.queued} job(s)`,
        };
      } catch (err) {
        if (err instanceof UnknownVerbError) {
          // Verb not yet routed in this build — fall through to local.
        } else {
          // Timeout, unreachable, or runtime error — fall through too,
          // but record the reason for diagnostic clarity.
          const counts = localCounts();
          const msg = err instanceof Error ? err.message : String(err);
          return {
            ...counts,
            duration_ms: Date.now() - t0,
            report_path: "",
            note: `local-counts (supervisor unreachable: ${msg})`,
          };
        }
      }
    }

    // ---- 2) Local shard counts (graceful degrade) -----------------------
    const counts = localCounts();
    return {
      ...counts,
      duration_ms: Date.now() - t0,
      report_path: "",
      note: "local-counts (verb not yet routed in this build)",
    };
  },
};
