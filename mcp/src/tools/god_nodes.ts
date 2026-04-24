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
 * v0.2 (this change):
 *   - Label is prefixed with the node's kind when known so callers can
 *     tell a `function:` from a `class:` or `file:` at a glance.
 *   - `community_id` is looked up in the semantic shard's
 *     `community_membership` table via `nodeCommunityIds` — a single
 *     bulk query. If the semantic shard hasn't been built yet (Leiden
 *     hasn't run) we return `null` per node, matching the schema.
 *
 * Graceful degrade: missing graph shard → `{ gods: [] }`.
 */

import {
  GodNodesInput,
  GodNodesOutput,
  type ToolDescriptor,
} from "../types.ts";
import {
  godNodesTopN,
  nodeCommunityIds,
  shardDbPath,
} from "../store.ts";

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
    if (rows.length === 0) {
      return { gods: [] };
    }

    // Best-effort enrichment: community_id comes from the semantic shard.
    // If Leiden hasn't run we simply get an empty map and emit null per node.
    const communityMap = nodeCommunityIds(rows.map((r) => r.qualified_name));

    return {
      gods: rows.map((r) => {
        const label = r.kind ? `${r.kind}:${r.qualified_name}` : r.qualified_name;
        const community_id = communityMap[r.qualified_name];
        return {
          id: r.qualified_name,
          label,
          degree: r.degree,
          betweenness: 0,
          community_id: community_id ?? null,
        };
      }),
    };
  },
};
