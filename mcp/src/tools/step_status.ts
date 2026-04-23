/**
 * MCP tool: step_status
 *
 * Returns the current step + entire ledger snapshot for a session.
 */

import {
  StepStatusInput,
  StepStatusOutput,
  type Step,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof StepStatusInput.parse>,
  ReturnType<typeof StepStatusOutput.parse>
> = {
  name: "step_status",
  description:
    "Get the current step and full ledger snapshot for the session. Use at the start of every turn to know where you are in the plan. Compaction-resilient: this is the source of truth across context resets.",
  inputSchema: StepStatusInput,
  outputSchema: StepStatusOutput,
  category: "step",
  async handler(input, ctx) {
    const sessionId = input.session_id ?? ctx.sessionId;
    const steps = await dbQuery
      .select<Step>(
        "tasks",
        "session_id = ? ORDER BY step_id ASC",
        [sessionId],
      )
      .catch(() => [] as Step[]);

    const current =
      steps.find((s) => s.status === "in_progress") ??
      steps.find((s) => s.status === "blocked") ??
      null;
    const root = steps.find((s) => s.parent_step_id === null);
    const driftTotal = steps.reduce((acc, s) => acc + s.drift_score, 0);

    return {
      current_step_id: current?.step_id ?? null,
      steps,
      drift_score_total: driftTotal,
      goal_root: root?.description ?? null,
    };
  },
};
