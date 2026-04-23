/**
 * MCP tool: step_complete
 *
 * Marks a step complete IF its acceptance check passes (unless force=true).
 * Returns the next step id (or null if the plan is finished).
 */

import {
  StepCompleteInput,
  StepCompleteOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof StepCompleteInput.parse>,
  ReturnType<typeof StepCompleteOutput.parse>
> = {
  name: "step_complete",
  description:
    "Mark a step complete. Refuses to advance if the acceptance check has not passed (override with force=true). Returns the next step id if any.",
  inputSchema: StepCompleteInput,
  outputSchema: StepCompleteOutput,
  category: "step",
  async handler(input) {
    const result = await dbQuery
      .raw<{ completed: boolean; next_step_id: string | null }>(
        "step.complete",
        { step_id: input.step_id, force: input.force },
      )
      .catch(() => ({ completed: false, next_step_id: null }));

    return {
      step_id: input.step_id,
      completed: result.completed,
      next_step_id: result.next_step_id,
    };
  },
};
