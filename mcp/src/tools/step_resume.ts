/**
 * MCP tool: step_resume
 *
 * Emits the resumption bundle (design §7.3) — used after a context
 * compaction to recover the full plan and current step.
 */

import {
  StepResumeInput,
  StepResumeOutput,
  type ToolDescriptor,
} from "../types.ts";
import { buildResume } from "../composer.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof StepResumeInput.parse>,
  ReturnType<typeof StepResumeOutput.parse>
> = {
  name: "step_resume",
  description:
    "Emit the resumption bundle: original goal, completed steps with proofs, YOU ARE HERE marker, planned steps, active constraints, verification gates. Use after compaction or when resuming a session.",
  inputSchema: StepResumeInput,
  outputSchema: StepResumeOutput,
  category: "step",
  async handler(input, ctx) {
    const result = await buildResume({
      cwd: ctx.cwd,
      sessionId: input.session_id ?? ctx.sessionId,
    });
    return result;
  },
};
