/**
 * MCP tool: step_status
 *
 * Returns the current step + entire ledger snapshot for a session.
 *
 * v0.1 (review P2): reads `tasks.db → steps` via `bun:sqlite` read-only.
 * Query shape: `SELECT * FROM steps WHERE session_id = ?` ordered
 * parent-first then by step_id. JSON columns (`artifacts`,
 * `acceptance_check`) are parsed into `unknown` for the schema.
 *
 * Graceful degrade: missing tasks shard → empty steps + null current.
 */

import {
  StepStatusInput,
  StepStatusOutput,
  StepStatusEnum,
  type Step,
  type StepStatus,
  type ToolDescriptor,
} from "../types.ts";
import { sessionSteps, shardDbPath } from "../store.ts";

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

    if (!shardDbPath("tasks")) {
      return {
        current_step_id: null,
        steps: [],
        drift_score_total: 0,
        goal_root: null,
      };
    }

    const rows = sessionSteps(sessionId);

    const steps: Step[] = rows.map((r) => ({
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
    }));

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
