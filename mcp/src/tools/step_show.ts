/**
 * MCP tool: step_show
 *
 * Full detail for one step by id.
 *
 * v0.1 (review P2): reads `tasks.db → steps` WHERE step_id=? via
 * `bun:sqlite` read-only. JSON columns (`acceptance_check`, `artifacts`)
 * parse to `unknown`; missing shard or missing step throws a clear error.
 */

import { z } from "zod";
import {
  StepShowInput,
  StepShowOutput,
  StepStatusEnum,
  type Step,
  type StepStatus,
  type ToolDescriptor,
} from "../types.ts";
import { shardDbPath, singleStep } from "../store.ts";

// F2 fix (NEW-052): additive `note` so callers can distinguish a
// "step actually exists" payload from a graceful "no step / no shard"
// placeholder. The legacy {step} contract is preserved.
const StepShowOutputExtended = StepShowOutput.extend({
  note: z.string().optional(),
});

function placeholderStep(stepId: string): Step {
  return {
    step_id: stepId,
    parent_step_id: null,
    session_id: "",
    description: "",
    acceptance_cmd: null,
    acceptance_check: null,
    status: "not_started",
    started_at: null,
    completed_at: null,
    verification_proof: null,
    artifacts: null,
    notes: "",
    blocker: null,
    drift_score: 0,
  };
}

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

type StepShowInputT = ReturnType<typeof StepShowInput.parse>;
type StepShowOutputExtendedT = z.infer<typeof StepShowOutputExtended>;

export const tool: ToolDescriptor<StepShowInputT, StepShowOutputExtendedT> = {
  name: "step_show",
  description:
    "Show the full detail of one step: description, acceptance command, status, proof, artifacts, notes, blocker, drift score.",
  inputSchema: StepShowInput,
  outputSchema: StepShowOutputExtended,
  category: "step",
  async handler(input) {
    if (!shardDbPath("tasks")) {
      return {
        step: placeholderStep(input.step_id),
        note: "tasks shard not yet created (run `mneme build .`)",
      };
    }
    const r = singleStep(input.step_id);
    if (!r) {
      return {
        step: placeholderStep(input.step_id),
        note: `no step with id ${input.step_id}`,
      };
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
