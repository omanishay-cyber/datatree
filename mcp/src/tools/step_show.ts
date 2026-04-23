/**
 * MCP tool: step_show
 *
 * Detail view of a single step.
 */

import {
  StepShowInput,
  StepShowOutput,
  type Step,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof StepShowInput.parse>,
  ReturnType<typeof StepShowOutput.parse>
> = {
  name: "step_show",
  description:
    "Show the full detail of one step: description, acceptance command, status, proof, artifacts, notes, blocker, drift score.",
  inputSchema: StepShowInput,
  outputSchema: StepShowOutput,
  category: "step",
  async handler(input) {
    const rows = await dbQuery
      .select<Step>("tasks", "step_id = ? LIMIT 1", [input.step_id])
      .catch(() => [] as Step[]);
    const step = rows[0];
    if (!step) {
      throw new Error(`step_show: no step with id ${input.step_id}`);
    }
    return { step };
  },
};
