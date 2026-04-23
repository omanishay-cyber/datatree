/**
 * MCP tool: find_references
 *
 * Returns all usages of a symbol (definitions, calls, imports, generic uses).
 */

import {
  FindReferencesInput,
  FindReferencesOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof FindReferencesInput.parse>,
  ReturnType<typeof FindReferencesOutput.parse>
> = {
  name: "find_references",
  description:
    "Find all references to a symbol across the project (or workspace). Returns ReferenceHit[] with file, line, column, kind, and surrounding context. Use INSTEAD of Grep when you want structural certainty.",
  inputSchema: FindReferencesInput,
  outputSchema: FindReferencesOutput,
  category: "graph",
  async handler(input) {
    const result = await dbQuery
      .raw<ReturnType<typeof FindReferencesOutput.parse>>(
        "graph.find_references",
        { symbol: input.symbol, scope: input.scope },
      )
      .catch(() => null);
    return result ?? { symbol: input.symbol, hits: [] };
  },
};
