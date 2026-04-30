/**
 * MCP tool: architecture_overview
 *
 * Returns a structured report of the project's architecture:
 *
 *   * community-by-community coupling matrix (edge counts + density)
 *   * per-community risk_index (callers x criticality x security hits)
 *   * top-K bridge nodes by betweenness centrality
 *   * top-K hub nodes by weighted degree
 *
 * v0.2 (phase-c8 wiring): reads `architecture.db → architecture_snapshots`
 * directly via `bun:sqlite` (see store.ts::latestArchitectureSnapshot).
 * When the snapshot table is empty (the analyzer hasn't run yet), we
 * graceful-degrade to a live overview derived from the graph + semantic
 * shards (node/edge counts + hub_nodes via god-node degrees).
 *
 * `refresh=true` is honored only via the supervisor: computing a fresh
 * Leiden partition + betweenness centrality requires the scanners layer.
 * If the supervisor is offline we fall through to the cached snapshot and,
 * failing that, to the live overview — never throws.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  ArchitectureOverviewInput,
  ArchitectureOverviewOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";
import {
  architectureLiveOverview,
  latestArchitectureSnapshot,
  nodeFilePaths,
  shardDbPath,
} from "../store.ts";

type Input = ReturnType<typeof ArchitectureOverviewInput.parse>;
type Output = ReturnType<typeof ArchitectureOverviewOutput.parse>;

function emptyOutput(): Output {
  return {
    community_count: 0,
    node_count: 0,
    edge_count: 0,
    coupling_matrix: [],
    risk_index: [],
    bridge_nodes: [],
    hub_nodes: [],
    captured_at: new Date().toISOString(),
  };
}

export const tool: ToolDescriptor<Input, Output> = {
  name: "architecture_overview",
  description:
    "Return a structured architecture report: coupling matrix between Leiden communities, per-community risk_index, top bridge nodes by betweenness, and top hubs by degree. Set refresh=true to recompute from the current graph; otherwise returns the most recent cached snapshot.",
  inputSchema: ArchitectureOverviewInput,
  outputSchema: ArchitectureOverviewOutput,
  category: "graph",
  async handler(input) {
    // Honor refresh=true via the supervisor — only it can run the analyzer.
    // If the daemon is offline we silently drop through to the cached path.
    if (input.refresh) {
      const raw = await dbQuery
        .raw<Partial<Output>>("architecture.overview", {
          project: input.project,
          refresh: true,
          top_k: input.top_k,
        })
        .catch(() => null);
      if (raw) {
        const parsed = ArchitectureOverviewOutput.safeParse(raw);
        if (parsed.success) return parsed.data;
      }
      // fall through to local read
    }

    // Cached snapshot — the hot path in steady state.
    if (shardDbPath("architecture")) {
      const snap = latestArchitectureSnapshot();
      if (snap) {
        // H3 (Phase A): cached snapshots predate the `file_path` field
        // on hub_nodes. Enrich here from the graph shard's nodes table so
        // schema validation passes and consumers always see resolved paths.
        const hubsRaw = snap.hub_nodes as Array<{
          qualified_name?: string;
          community_id?: number;
          degree?: number;
          file_path?: string | null;
        }>;
        const qns = hubsRaw
          .map((h) => h.qualified_name)
          .filter((q): q is string => typeof q === "string");
        const fpMap = nodeFilePaths(qns);
        const enrichedHubs = hubsRaw.map((h) => ({
          qualified_name: h.qualified_name ?? "",
          community_id: h.community_id ?? -1,
          degree: h.degree ?? 0,
          file_path:
            h.file_path ??
            (h.qualified_name ? fpMap[h.qualified_name] ?? null : null),
        }));
        const parsed = ArchitectureOverviewOutput.safeParse({
          community_count: snap.community_count,
          node_count: snap.node_count,
          edge_count: snap.edge_count,
          coupling_matrix: snap.coupling_matrix,
          risk_index: snap.risk_index,
          bridge_nodes: snap.bridge_nodes,
          hub_nodes: enrichedHubs,
          captured_at: snap.captured_at,
        });
        if (parsed.success) return parsed.data;
      }
    }

    // Live fallback: derive what we can from graph + semantic shards.
    if (shardDbPath("graph")) {
      const live = architectureLiveOverview(input.top_k);
      return {
        community_count: live.community_count,
        node_count: live.node_count,
        edge_count: live.edge_count,
        coupling_matrix: [],
        risk_index: [],
        bridge_nodes: [],
        hub_nodes: live.hub_nodes,
        captured_at: new Date().toISOString(),
      };
    }

    return emptyOutput();
  },
};
