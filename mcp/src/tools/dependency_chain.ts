/**
 * MCP tool: dependency_chain
 *
 * Forward (what file imports transitively) + reverse (what imports file
 * transitively) chain.
 *
 * v0.1 (review P2): BFS over `graph.db → edges` with kind IN
 * ('imports', 'import'). Forward uses edges where `file_path = file`;
 * reverse uses joined `nodes.file_path = file` on the target side.
 * Missing shard → `{ forward: [], reverse: [] }`.
 */

import {
  DependencyChainInput,
  DependencyChainOutput,
  type ToolDescriptor,
} from "../types.ts";
import { dependencyChain, shardDbPath } from "../store.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof DependencyChainInput.parse>,
  ReturnType<typeof DependencyChainOutput.parse>
> = {
  name: "dependency_chain",
  description:
    "Get the forward and reverse import chain for a file. Forward = files this file imports (transitively). Reverse = files that import this file (transitively).",
  inputSchema: DependencyChainInput,
  outputSchema: DependencyChainOutput,
  category: "graph",
  async handler(input) {
    if (!shardDbPath("graph")) {
      return { file: input.file, forward: [], reverse: [] };
    }
    const { forward, reverse } = dependencyChain(input.file, input.direction);
    return { file: input.file, forward, reverse };
  },
};
