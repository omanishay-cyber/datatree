/**
 * MCP tool: rebuild
 *
 * Re-parse the requested scope from scratch.
 *
 * v0.1 (review P2): rebuild is always a write — we hand off to
 * `lifecycle.rebuild` over IPC which runs `mneme rebuild <scope>` under
 * the supervisor's single-writer lock. We keep the safety rail: we
 * refuse without `confirm=true`. If IPC is down we report a clear
 * zero-duration empty-rebuilt result rather than silently succeeding.
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
    const t0 = Date.now();
    try {
      const r = await lifecycle.rebuild(input.scope);
      return r;
    } catch {
      return { rebuilt: [], duration_ms: Date.now() - t0 };
    }
  },
};
