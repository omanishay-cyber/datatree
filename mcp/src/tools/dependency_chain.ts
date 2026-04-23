/**
 * MCP tool: dependency_chain
 *
 * Returns the forward (what file imports) + reverse (what imports file) chain.
 */

import {
  DependencyChainInput,
  DependencyChainOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

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
    const result = await dbQuery
      .raw<ReturnType<typeof DependencyChainOutput.parse>>(
        "graph.dependency_chain",
        { file: input.file, direction: input.direction },
      )
      .catch(() => null);
    return result ?? { file: input.file, forward: [], reverse: [] };
  },
};
