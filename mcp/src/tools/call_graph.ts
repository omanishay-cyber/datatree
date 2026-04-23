/**
 * MCP tool: call_graph
 *
 * Returns the direct + transitive call graph for a function.
 *
 * v0.1 (review P2): reads `graph.db → edges` WHERE kind='calls' via
 * `bun:sqlite` read-only. BFS up to `depth` hops in the requested
 * direction (callers, callees, both). For each visited node we pull its
 * source location from `nodes`. Missing shard → `{ nodes: [], edges: [] }`.
 */

import {
  CallGraphInput,
  CallGraphOutput,
  type ToolDescriptor,
} from "../types.ts";
import { callGraphBfs, shardDbPath } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof CallGraphInput.parse>,
  ReturnType<typeof CallGraphOutput.parse>
> = {
  name: "call_graph",
  description:
    "Get the call graph for a function. direction='callers' returns who calls it, 'callees' returns what it calls, 'both' returns the union. Bounded by depth (default 3).",
  inputSchema: CallGraphInput,
  outputSchema: CallGraphOutput,
  category: "graph",
  async handler(input) {
    if (!shardDbPath("graph")) {
      return { nodes: [], edges: [] };
    }
    const { nodes, edges } = callGraphBfs(
      input.function,
      input.direction,
      input.depth,
    );
    return { nodes, edges };
  },
};
