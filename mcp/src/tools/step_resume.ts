/**
 * MCP tool: step_resume
 *
 * Emits the resumption bundle (design §7.3) — used after a context
 * compaction to recover the full plan and current step.
 *
 * v0.1 (review P2): assembles the bundle from local-shard reads
 * (`tasks.db → steps`, `tasks.db → ledger_entries` for recent open questions,
 * `memory.db → constraints` for active rules) via `bun:sqlite` read-only,
 * then falls back to the IPC-based `buildResume` composer if the local reads
 * return empty. Mirrors `brain::Ledger::resume_summary` without the
 * embeddings similarity ranking.
 *
 * Graceful degrade: every shard read is independently fallible — if none
 * succeed we still emit a valid bundle with `current_step_id: null` and
 * `total_steps: 0`.
 */

import {
  StepResumeInput,
  StepResumeOutput,
  StepStatusEnum,
  type Step,
  type Constraint,
  type StepStatus,
  type ToolDescriptor,
} from "../types.ts";
import {
  activeConstraints,
  recentLedger,
  sessionSteps,
  shardDbPath,
} from "../store.ts";
import { buildResume, composeResume } from "../composer.ts";

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

function rowsToSteps(
  rows: ReturnType<typeof sessionSteps>,
): Step[] {
  return rows.map((r) => ({
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
}

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
    const sessionId = input.session_id ?? ctx.sessionId;
    const tasksAvailable = shardDbPath("tasks") !== null;

    // Prefer the local-shard path — IPC is optional.
    if (tasksAvailable) {
      const allSteps = rowsToSteps(sessionSteps(sessionId));
      const constraintsRows = activeConstraints("project", undefined, 10);
      const constraints: Constraint[] = constraintsRows.map((r) => ({
        id: String(r.id),
        rule: r.rule,
        scope: r.scope,
        source: r.source ?? "unknown",
        severity: "medium",
        enforcement: "warn" as const,
      }));

      // Pull the most recent open_question in the last 48h as a goal hint.
      const sinceMs = Date.now() - 48 * 60 * 60 * 1000;
      const recent = recentLedger(sessionId, sinceMs, 20);
      const goalText =
        recent.find((e) => e.kind === "decision")?.summary ??
        recent[0]?.summary ??
        "(no recorded goal)";

      const completed = allSteps.filter((s) => s.status === "completed");
      const planned = allSteps.filter((s) => s.status === "not_started");
      const current =
        allSteps.find((s) => s.status === "in_progress") ??
        allSteps.find((s) => s.status === "blocked") ??
        null;

      const bundle = composeResume({
        ctx: { cwd: ctx.cwd, sessionId },
        goalText,
        goalStack: allSteps.filter((s) => s.parent_step_id === null),
        completedSteps: completed,
        currentStep: current,
        plannedSteps: planned,
        constraints,
      });

      return {
        bundle,
        current_step_id: current?.step_id ?? null,
        total_steps: allSteps.length,
      };
    }

    // Final fallback: IPC-based composer (may succeed via supervisor even
    // when direct shard reads fail — e.g., in multi-project setups).
    try {
      return await buildResume({ cwd: ctx.cwd, sessionId });
    } catch {
      return {
        bundle: "<mneme-resume>\n(no ledger yet — run `mneme build .`)\n</mneme-resume>",
        current_step_id: null,
        total_steps: 0,
      };
    }
  },
};
