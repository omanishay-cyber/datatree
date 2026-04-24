/**
 * MCP tool: step_resume
 *
 * Emits the resumption bundle (design §7.3) — the killer feature Claude
 * calls first after any context compaction or session restart.
 *
 * v0.1 base: assembles the bundle from local-shard reads
 * (`tasks.db → steps`, `tasks.db → ledger_entries`, `memory.db → constraints`)
 * via `bun:sqlite` read-only, falling back to the IPC-based `buildResume`
 * composer if local reads fail. Mirrors `brain::Ledger::resume_summary`
 * without the embeddings similarity ranking.
 *
 * v0.2 (Step Ledger wiring): the output is now additively enriched with
 *   - `goal` — resolved via root-step description → most recent decision
 *     summary → most recent ledger summary,
 *   - `completed_steps` / `current_step` / `planned_steps` projected onto
 *     a compact {id, description, hint?, proof?} shape,
 *   - `active_constraints` in the same pointer shape as step_status,
 *   - `transcript_refs` — the `TranscriptRef` JSON payload attached to
 *     each recent ledger entry, so the model can reopen the exact turn
 *     that produced a prior decision / implementation / open question.
 *
 * The legacy `{bundle, current_step_id, total_steps}` contract is preserved
 * so existing consumers keep working.
 *
 * Graceful degrade: every shard read is independently fallible — missing
 * tasks.db, empty ledger, empty constraints all still produce a valid
 * bundle. When ALL local reads return empty AND IPC also fails, we emit
 * the "no ledger yet" placeholder bundle.
 */

import { z } from "zod";
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
  goalForSession,
  ledgerEntriesWithRefs,
  safeJsonRecord,
  safeJsonStringArray,
  sessionSteps,
  shardDbPath,
  verificationGateForSession,
} from "../store.ts";
import { buildResume, composeResume } from "../composer.ts";

// ---------------------------------------------------------------------------
// Extended schema (additive over StepResumeOutput).
// ---------------------------------------------------------------------------

const StepPointer = z.object({
  id: z.string(),
  description: z.string(),
  status: StepStatusEnum,
});

const CompletedStep = StepPointer.extend({
  proof: z.string().nullable(),
});

const CurrentStepHint = StepPointer.extend({
  hint: z.string().nullable(),
});

const ActiveConstraint = z.object({
  id: z.string(),
  rule: z.string(),
  scope: z.string(),
  source: z.string(),
});

const TranscriptRef = z.object({
  entry_id: z.string(),
  kind: z.string(),
  summary: z.string(),
  session_id: z.string().nullable(),
  turn_index: z.number().int().nullable(),
  message_id: z.string().nullable(),
  touched_files: z.array(z.string()),
  timestamp: z.number(),
});

const StepResumeOutputExtended = StepResumeOutput.extend({
  goal: z.string(),
  completed_steps: z.array(CompletedStep),
  current_step: CurrentStepHint.nullable(),
  planned_steps: z.array(StepPointer),
  active_constraints: z.array(ActiveConstraint),
  transcript_refs: z.array(TranscriptRef),
  verification_gate: z.string().nullable(),
});

type StepResumeInputT = ReturnType<typeof StepResumeInput.parse>;
type StepResumeOutputExtendedT = z.infer<typeof StepResumeOutputExtended>;

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

function rowsToSteps(rows: ReturnType<typeof sessionSteps>): Step[] {
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

/** Pick the most useful "hint" for the current step — prioritises an
 *  explicit blocker (why stuck), falls back to the last note line, then
 *  to the started_at timestamp. Returns null when nothing is informative. */
function currentHint(step: Step): string | null {
  if (step.blocker && step.blocker.trim().length > 0) {
    return `blocked: ${step.blocker}`;
  }
  const notes = step.notes ?? "";
  const lastNote = notes.split("\n").map((l) => l.trim()).filter(Boolean).pop();
  if (lastNote) return lastNote;
  if (step.started_at) return `started ${step.started_at}`;
  return null;
}

/** Parse the `transcript_ref` JSON column into a typed record. Shape
 *  matches `brain::ledger::TranscriptRef` — {session_id, turn_index?, message_id?}. */
function parseTranscriptRef(raw: string | null): {
  session_id: string | null;
  turn_index: number | null;
  message_id: string | null;
} {
  const parsed = safeJsonRecord(raw);
  if (!parsed) {
    return { session_id: null, turn_index: null, message_id: null };
  }
  const sid = typeof parsed["session_id"] === "string" ? (parsed["session_id"] as string) : null;
  const ti = typeof parsed["turn_index"] === "number" ? (parsed["turn_index"] as number) : null;
  const mid = typeof parsed["message_id"] === "string" ? (parsed["message_id"] as string) : null;
  return { session_id: sid, turn_index: ti, message_id: mid };
}

export const tool: ToolDescriptor<StepResumeInputT, StepResumeOutputExtendedT> = {
  name: "step_resume",
  description:
    "Emit the resumption bundle — the KILLER feature. Returns the original goal, completed steps with proofs, YOU ARE HERE marker with a hint, planned steps, active constraints, verification gate, and transcript refs to the exact turns that produced prior decisions. Call this FIRST after any compaction or session restart.",
  inputSchema: StepResumeInput,
  outputSchema: StepResumeOutputExtended,
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
      const activeConstraintPointers = constraintsRows.map((c) => ({
        id: String(c.id),
        rule: c.rule,
        scope: c.scope,
        source: c.source ?? "unknown",
      }));

      // Pull recent ledger entries (last 48h) with their JSON side-columns
      // so we can expose transcript refs and touched files. Order: newest first.
      const sinceMs = Date.now() - 48 * 60 * 60 * 1000;
      const recent = ledgerEntriesWithRefs(sessionId, sinceMs, 20);

      const transcriptRefs = recent.map((e) => {
        const tr = parseTranscriptRef(e.transcript_ref);
        return {
          entry_id: e.id,
          kind: e.kind,
          summary: e.summary,
          session_id: tr.session_id,
          turn_index: tr.turn_index,
          message_id: tr.message_id,
          touched_files: safeJsonStringArray(e.touched_files),
          timestamp: e.timestamp,
        };
      });

      // Goal resolution: root step → recent decision → recent anything.
      const goalText = goalForSession(sessionId) ?? "(no recorded goal)";

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

      const completedSteps = completed.map((s) => ({
        id: s.step_id,
        description: s.description,
        status: s.status,
        proof: s.verification_proof ?? null,
      }));

      const plannedSteps = planned.map((s) => ({
        id: s.step_id,
        description: s.description,
        status: s.status,
      }));

      const currentStepHint = current
        ? {
            id: current.step_id,
            description: current.description,
            status: current.status,
            hint: currentHint(current),
          }
        : null;

      const verificationGate =
        current?.acceptance_cmd ??
        verificationGateForSession(sessionId) ??
        null;

      return {
        bundle,
        current_step_id: current?.step_id ?? null,
        total_steps: allSteps.length,
        goal: goalText,
        completed_steps: completedSteps,
        current_step: currentStepHint,
        planned_steps: plannedSteps,
        active_constraints: activeConstraintPointers,
        transcript_refs: transcriptRefs,
        verification_gate: verificationGate,
      };
    }

    // Final fallback: IPC-based composer (may succeed via supervisor even
    // when direct shard reads fail — e.g., in multi-project setups).
    try {
      const ipc = await buildResume({ cwd: ctx.cwd, sessionId });
      return {
        bundle: ipc.bundle,
        current_step_id: ipc.current_step_id,
        total_steps: ipc.total_steps,
        goal: "(no recorded goal)",
        completed_steps: [],
        current_step: null,
        planned_steps: [],
        active_constraints: [],
        transcript_refs: [],
        verification_gate: null,
      };
    } catch {
      return {
        bundle:
          "<mneme-resume>\n(no ledger yet — run `mneme build .`)\n</mneme-resume>",
        current_step_id: null,
        total_steps: 0,
        goal: "(no recorded goal)",
        completed_steps: [],
        current_step: null,
        planned_steps: [],
        active_constraints: [],
        transcript_refs: [],
        verification_gate: null,
      };
    }
  },
};
