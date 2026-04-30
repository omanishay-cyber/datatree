/**
 * MCP tool: god_nodes
 *
 * Returns the top-N most-connected concepts in the corpus graph.
 *
 * v0.1 (review P2): reads `graph.db` directly via `bun:sqlite`. Query shape:
 * UNION ALL over (source_qualified, target_qualified) in `edges`, grouped
 * and ordered by total degree DESC, LIMIT top_n.
 *
 * v0.2: kind-prefixed label + community_id lookup via the semantic shard.
 *
 * v0.3 (Phase A H3 / H6):
 *   - JOIN against the `nodes` table on `qualified_name` so the response
 *     carries a resolved `file_path`. Previously callers only got opaque
 *     `n_f62d…` ids and had no way to map them to source files.
 *   - The `betweenness` field is removed entirely. Computing Brandes O(VE)
 *     here would block the MCP event loop; rather than emit a misleading
 *     constant 0, we drop the field. When the supervisor's
 *     `brain::god_nodes` push-path lands it will own betweenness.
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
          file_path: r.file_path,
          community_id: community_id ?? null,
        };
      }),
    };
  },
};
