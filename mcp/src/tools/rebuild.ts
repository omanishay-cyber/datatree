/**
 * MCP tool: rebuild
 *
 * Last-resort: re-parse a scope from scratch (graph, semantic, or all).
 * Requires confirm=true to actually run.
 */

import {
  RebuildInput,
  RebuildOutput,
  type ToolDescriptor,
} from "../types.ts";
import { lifecycle } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof RebuildInput.parse>,
  ReturnType<typeof RebuildOutput.parse>
> = {
  name: "rebuild",
  description:
    "Re-parse the requested scope from scratch (graph | semantic | all). Last resort — clears existing data and rebuilds. Requires confirm=true.",
  inputSchema: RebuildInput,
  outputSchema: RebuildOutput,
  category: "health",
  async handler(input) {
    if (!input.confirm) {
      throw new Error(
        "rebuild: refused without confirm=true (this clears existing data)",
      );
    }
    const result = await lifecycle.rebuild(input.scope);
    return result;
  },
};
