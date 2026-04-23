/**
 * MCP tool: call_graph
 *
 * Returns the direct + transitive call graph for a function in either
 * direction (callers, callees, or both).
 */

import {
  CallGraphInput,
  CallGraphOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const result = await dbQuery
      .raw<ReturnType<typeof CallGraphOutput.parse>>("graph.call_graph", {
        function: input.function,
        direction: input.direction,
        depth: input.depth,
      })
      .catch(() => null);
    return result ?? { nodes: [], edges: [] };
  },
};
