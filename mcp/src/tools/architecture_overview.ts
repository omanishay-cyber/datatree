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
 * The Rust supervisor method `architecture.overview` runs the analyser in
 * `scanners::scanners::architecture` over the current graph and, when
 * `refresh` is true, writes a fresh row to `architecture_snapshots`
 * before returning it.
 *
 * Hot-reload safe: no module-level mutable state.
 */

import {
  ArchitectureOverviewInput,
  ArchitectureOverviewOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

type Input = ReturnType<typeof ArchitectureOverviewInput.parse>;
type Output = ReturnType<typeof ArchitectureOverviewOutput.parse>;

export const tool: ToolDescriptor<Input, Output> = {
  name: "architecture_overview",
  description:
    "Return a structured architecture report: coupling matrix between Leiden communities, per-community risk_index, top bridge nodes by betweenness, and top hubs by degree. Set refresh=true to recompute from the current graph; otherwise returns the most recent cached snapshot.",
  inputSchema: ArchitectureOverviewInput,
  outputSchema: ArchitectureOverviewOutput,
  category: "graph",
  async handler(input) {
    const raw = await dbQuery
      .raw<Partial<Output>>("architecture.overview", {
        project: input.project,
        refresh: input.refresh,
        top_k: input.top_k,
      })
      .catch(() => null);

    if (!raw) {
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
    return {
      community_count: raw.community_count ?? 0,
      node_count: raw.node_count ?? 0,
      edge_count: raw.edge_count ?? 0,
      coupling_matrix: raw.coupling_matrix ?? [],
      risk_index: raw.risk_index ?? [],
      bridge_nodes: raw.bridge_nodes ?? [],
      hub_nodes: raw.hub_nodes ?? [],
      captured_at: raw.captured_at ?? new Date().toISOString(),
    };
  },
};
