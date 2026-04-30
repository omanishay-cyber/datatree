/**
 * MCP tool: step_status
 *
 * Returns the current step pointer + entire ledger snapshot for a session.
 *
 * v0.1 base: reads `tasks.db → steps` via `bun:sqlite` read-only.
 * Query shape: `SELECT * FROM steps WHERE session_id = ?` ordered
 * parent-first then by step_id. JSON columns (`artifacts`,
 * `acceptance_check`) are parsed into `unknown` for the schema.
 *
 * v0.2 (Step Ledger wiring): additively enriches the output with
 *   - `completed` / `pending` buckets projected onto a tiny
 *     {id, description, status} shape for cheap "where am I?" reads,
 *   - `active_constraints` (pulled from memory.db), and
 *   - `verification_gate` — the acceptance_cmd the model must pass
 *     before the current step may be closed.
 *
 * All new fields are additive: the legacy `StepStatusOutput` contract
 * (`current_step_id`, `steps`, `drift_score_total`, `goal_root`) is
 * preserved so downstream callers that don't know about the enrichment
 * keep working.
 *
 * Graceful degrade: missing tasks shard → empty steps + null current,
 * empty constraints, null verification_gate.
 */

import { z } from "zod";
import {
  StepStatusInput,
  StepStatusOutput,
  StepStatusEnum,
  type Step,
  type StepStatus,
  type ToolDescriptor,
} from "../types.ts";
import {
  activeConstraints,
  sessionSteps,
  shardDbPath,
  verificationGateForSession,
} from "../store.ts";

// ---------------------------------------------------------------------------
// Extended schema (additive over StepStatusOutput).
// ---------------------------------------------------------------------------

const StepPointer = z.object({
  id: z.string(),
  description: z.string(),
  status: StepStatusEnum,
});

const ActiveConstraint = z.object({
  id: z.string(),
  rule: z.string(),
  scope: z.string(),
  source: z.string(),
});

const StepStatusOutputExtended = StepStatusOutput.extend({
  current_step: StepPointer.nullable(),
  completed: z.array(StepPointer),
  pending: z.array(StepPointer),
  constraints: z.array(ActiveConstraint),
  verification_gate: z.string().nullable(),
});

type StepStatusInputT = ReturnType<typeof StepStatusInput.parse>;
type StepStatusOutputExtendedT = z.infer<typeof StepStatusOutputExtended>;

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

function toPointer(s: Step): z.infer<typeof StepPointer> {
  return { id: s.step_id, description: s.description, status: s.status };
}

export const tool: ToolDescriptor<StepStatusInputT, StepStatusOutputExtendedT> = {
  name: "step_status",
  description:
    "Get the current step pointer, completed/pending buckets, active constraints, and the verification gate that must pass before the current step may be closed. Use at the start of every turn to know where you are in the plan — compaction-resilient, the source of truth across context resets.",
  inputSchema: StepStatusInput,
  outputSchema: StepStatusOutputExtended,
  category: "step",
  async handler(input, ctx) {
    const sessionId = input.session_id ?? ctx.sessionId;

    if (!shardDbPath("tasks")) {
      return {
        current_step_id: null,
        steps: [],
        drift_score_total: 0,
        goal_root: null,
        current_step: null,
        completed: [],
        pending: [],
        constraints: [],
        verification_gate: null,
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

    const completed = steps
      .filter((s) => s.status === "completed")
      .map(toPointer);
    const pending = steps
      .filter((s) => s.status === "not_started")
      .map(toPointer);

    // Pull project-scope constraints (includes globals). Kept small — the
    // model only needs the rules that could block the current action.
    let constraints: z.infer<typeof ActiveConstraint>[] = [];
    try {
      const rawConstraints = activeConstraints("project", undefined, 10);
      constraints = rawConstraints.map((c) => ({
        id: String(c.id),
        rule: c.rule,
        scope: c.scope,
        source: c.source ?? "unknown",
      }));
    } catch {
      constraints = [];
    }

    // Verification gate: prefer the current step's acceptance_cmd (already on
    // the step row); fall back to the dedicated helper for parity with
    // resume's gate resolution. Keep a simple string so the model can run it.
    const verificationGate =
      current?.acceptance_cmd ??
      verificationGateForSession(sessionId) ??
      null;

    return {
      current_step_id: current?.step_id ?? null,
      steps,
      drift_score_total: driftTotal,
      goal_root: root?.description ?? null,
      current_step: current ? toPointer(current) : null,
      completed,
      pending,
      constraints,
      verification_gate: verificationGate,
    };
  },
};
