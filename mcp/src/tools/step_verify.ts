/**
 * MCP tool: step_verify
 *
 * Runs the acceptance check for a step (shell command or structured check).
 * Returns pass/fail + captured proof. Does NOT mark the step complete.
 */

import {
  StepVerifyInput,
  StepVerifyOutput,
  type ToolDescriptor,
} from "../types.ts";
import { query as dbQuery } from "../db.ts";

export const tool: ToolDescriptor<
  ReturnType<typeof StepVerifyInput.parse>,
  ReturnType<typeof StepVerifyOutput.parse>
> = {
  name: "step_verify",
  description:
    "Run the acceptance check for a step. Returns passed/proof/exit_code. Does NOT mark complete — call step_complete after a passing verify.",
  inputSchema: StepVerifyInput,
  outputSchema: StepVerifyOutput,
  category: "step",
  async handler(input) {
    const t0 = Date.now();
    const result = await dbQuery
      .raw<{ passed: boolean; proof: string; exit_code: number }>(
        "step.verify",
        { step_id: input.step_id, dry_run: input.dry_run },
      )
      .catch((err) => ({
        passed: false,
        proof: `verify failed: ${(err as Error).message}`,
        exit_code: 127,
      }));

    return {
      step_id: input.step_id,
      passed: result.passed,
      proof: result.proof,
      exit_code: result.exit_code,
      duration_ms: Date.now() - t0,
    };
  },
};
