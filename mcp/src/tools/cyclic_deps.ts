/**
 * MCP tool: cyclic_deps
 *
 * Detects circular dependency chains across the project graph.
 */

import {
  CyclicDepsInput,
  CyclicDepsOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof CyclicDepsInput.parse>,
  ReturnType<typeof CyclicDepsOutput.parse>
> = {
  name: "cyclic_deps",
  description:
    "Detect circular dependencies in the project graph. Returns each cycle as an ordered list of file paths. Run after large refactors or before merging a PR that touches imports.",
  inputSchema: CyclicDepsInput,
  outputSchema: CyclicDepsOutput,
  category: "graph",
  async handler(input) {
    const result = await dbQuery
      .raw<ReturnType<typeof CyclicDepsOutput.parse>>("graph.cyclic_deps", {
        scope: input.scope,
      })
      .catch(() => null);
    return result ?? { cycles: [], count: 0 };
  },
};
