/**
 * MCP tool: step_plan_from
 *
 * Ingests a Markdown roadmap (numbered checklist) and creates a step ledger
 * tree from it.
 */

import {
  StepPlanFromInput,
  StepPlanFromOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof StepPlanFromInput.parse>,
  ReturnType<typeof StepPlanFromOutput.parse>
> = {
  name: "step_plan_from",
  description:
    "Ingest a Markdown roadmap (numbered hierarchical checklist) and create a step-ledger tree. Each numbered item becomes a Step row. Use at the start of any multi-step task to anchor the plan.",
  inputSchema: StepPlanFromInput,
  outputSchema: StepPlanFromOutput,
  category: "step",
  async handler(input, ctx) {
    const sessionId = input.session_id ?? ctx.sessionId;
    const result = await dbQuery
      .raw<{ steps_created: number; root_step_id: string }>(
        "step.plan_from_markdown",
        { markdown_path: input.markdown_path, session_id: sessionId },
      )
      .catch(() => ({ steps_created: 0, root_step_id: "" }));
    return result;
  },
};
