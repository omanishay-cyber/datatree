/**
 * MCP tool: god_nodes
 *
 * Returns the top-N most-connected concepts in the corpus graph.
 */

import {
  GodNodesInput,
  GodNodesOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const result = await dbQuery
      .raw<ReturnType<typeof GodNodesOutput.parse>>("multimodal.god_nodes", {
        project: input.project,
        top_n: input.top_n,
      })
      .catch(() => null);
    return result ?? { gods: [] };
  },
};
