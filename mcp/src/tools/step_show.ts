/**
 * MCP tool: step_show
 *
 * Full detail for one step by id.
 *
 * v0.1 (review P2): reads `tasks.db → steps` WHERE step_id=? via
 * `bun:sqlite` read-only. JSON columns (`acceptance_check`, `artifacts`)
 * parse to `unknown`; missing shard or missing step throws a clear error.
 */

import {
  StepShowInput,
  StepShowOutput,
  StepStatusEnum,
  type Step,
  type StepStatus,
  type ToolDescriptor,
} from "../types.ts";
import { shardDbPath, singleStep } from "../store.ts";

function coerceStatus(s: string): StepStatus {
  const parsed = StepStatusEnum.safeParse(s);
  return parsed.success ? parsed.data : "not_started";
}

function safeJson(raw: string | null | undefined): unknown {
  if (raw == null || raw === "") return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

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
    if (!shardDbPath("tasks")) {
      throw new Error(
        `step_show: tasks shard not yet created (run \`mneme build .\`)`,
      );
    }
    const r = singleStep(input.step_id);
    if (!r) {
      throw new Error(`step_show: no step with id ${input.step_id}`);
    }
    const step: Step = {
      step_id: r.step_id,
      parent_step_id: r.parent_step_id,
      session_id: r.session_id,
      description: r.description,
      acceptance_cmd: r.acceptance_cmd,
      acceptance_check: safeJson(r.acceptance_check),
      status: coerceStatus(r.status),
      started_at: r.started_at,
      completed_at: r.completed_at,
      verification_proof: r.verification_proof,
      artifacts: safeJson(r.artifacts),
      notes: r.notes ?? "",
      blocker: r.blocker,
      drift_score: r.drift_score,
    };
    return { step };
  },
};
