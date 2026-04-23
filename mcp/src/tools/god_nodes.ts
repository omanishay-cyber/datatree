/**
 * MCP tool: god_nodes
 *
 * Returns the top-N most-connected concepts in the corpus graph.
 *
 * v0.1 (review P2): reads `graph.db` directly via `bun:sqlite`. Query shape:
 * UNION ALL over (source_qualified, target_qualified) in `edges`, grouped
 * and ordered by total degree DESC, LIMIT top_n. Betweenness is not yet
 * computed client-side (requires a full graph traversal in Rust's
 * `brain::god_nodes`); we report 0 until the supervisor push-path lands.
 *
 * Graceful degrade: missing graph shard → `{ gods: [] }`.
 */

import {
  GodNodesInput,
  GodNodesOutput,
  type ToolDescriptor,
} from "../types.ts";
import { godNodesTopN, shardDbPath } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof GodNodesInput.parse>,
  ReturnType<typeof GodNodesOutput.parse>
> = {
  name: "god_nodes",
  description:
    "Get the top-N most-connected nodes in the project's concept graph. High-degree nodes are usually architectural anchors. Use to understand a codebase you are new to.",
  inputSchema: GodNodesInput,
  outputSchema: GodNodesOutput,
  category: "multimodal",
  async handler(input) {
    if (!shardDbPath("graph")) {
      return { gods: [] };
    }

    const rows = godNodesTopN(input.top_n);

    return {
      gods: rows.map((r) => ({
        id: r.qualified_name,
        label: r.qualified_name,
        degree: r.degree,
        betweenness: 0,
        community_id: null,
      })),
    };
  },
};
