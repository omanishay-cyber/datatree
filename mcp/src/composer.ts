/**
 * Context-bundle composer.
 *
 * Builds <mneme-context>, <mneme-primer>, and <mneme-resume> blocks
 * by querying the per-project shards (history, decisions, constraints, tasks,
 * findings) and ranking results by relevance to a seed query/prompt. Output is
 * always token-budget bounded — see TokenBudgets in types.ts.
 *
 * The composer is the only place that decides what makes the final cut. Tools
 * SHOULD compose via these helpers rather than concatenating raw query output.
 */

import {
  TokenBudgets,
  type Decision,
  type Finding,
  type Step,
  type Constraint,
  type Todo,
  type ConversationTurn,
} from "./types.ts";
import { query as dbQuery } from "./db.ts";

// ---------------------------------------------------------------------------
// Token estimation (chars/4 rule of thumb — good enough for budget gating)
// ---------------------------------------------------------------------------

const CHARS_PER_TOKEN = 4;

export function estimateTokens(text: string): number {
  if (!text) return 0;
  return Math.ceil(text.length / CHARS_PER_TOKEN);
}

function clampToBudget(text: string, tokenBudget: number): string {
  const charBudget = tokenBudget * CHARS_PER_TOKEN;
  if (text.length <= charBudget) return text;
  return `${text.slice(0, charBudget)}\n... (truncated at ~${tokenBudget} tokens)`;
}

// ---------------------------------------------------------------------------
// Section builders
// ---------------------------------------------------------------------------

interface ComposeContext {
  cwd: string;
  sessionId: string;
}

interface PrimerInputs {
  ctx: ComposeContext;
  goal?: string;
  decisions: Decision[];
  constraints: Constraint[];
  todos: Todo[];
  dirtyFiles: string[];
  redFindings: Finding[];
}

interface SmartInjectInputs {
  ctx: ComposeContext;
  prompt: string;
  decisions: Decision[];
  conversationTurns: ConversationTurn[];
  constraints: Constraint[];
  findings: Finding[];
  driftActive: boolean;
}

interface ResumeInputs {
  ctx: ComposeContext;
  goalText: string;
  goalStack: Step[];
  completedSteps: Step[];
  currentStep: Step | null;
  plannedSteps: Step[];
  constraints: Constraint[];
}

function joinSection(title: string, lines: string[]): string {
  if (lines.length === 0) return "";
  return `## ${title}\n${lines.map((l) => `- ${l}`).join("\n")}`;
}

// ---------------------------------------------------------------------------
// Compose: SessionStart primer (≤ 1.5K tokens — design §4.1)
// ---------------------------------------------------------------------------

export function composePrimer(inputs: PrimerInputs): string {
  const sections: string[] = [];
  sections.push("<mneme-primer>");
  sections.push(`Project: ${inputs.ctx.cwd}`);

  if (inputs.goal) {
    sections.push(`\n## Active goal\n${inputs.goal}`);
  }

  sections.push(
    joinSection(
      "Top constraints (must honor)",
      inputs.constraints.slice(0, 3).map(
        (c) => `[${c.severity}] ${c.rule}  (source: ${c.source})`,
      ),
    ),
  );

  sections.push(
    joinSection(
      "Open TODOs",
      inputs.todos.slice(0, 5).map(
        (t) => `${t.id}: ${t.text}${t.tags.length ? ` (${t.tags.join(",")})` : ""}`,
      ),
    ),
  );

  sections.push(
    joinSection(
      "Recent decisions",
      inputs.decisions.slice(0, 3).map(
        (d) => `${d.topic}: ${d.chosen} — ${d.reasoning.slice(0, 100)}`,
      ),
    ),
  );

  if (inputs.dirtyFiles.length > 0) {
    sections.push(
      joinSection("Dirty files (uncommitted)", inputs.dirtyFiles.slice(0, 10)),
    );
  }

  if (inputs.redFindings.length > 0) {
    sections.push(
      joinSection(
        "Critical drift findings",
        inputs.redFindings
          .slice(0, 5)
          .map((f) => `${f.file}: ${f.rule} — ${f.message}`),
      ),
    );
  }

  sections.push("</mneme-primer>");
  const raw = sections.filter(Boolean).join("\n\n");
  return clampToBudget(raw, TokenBudgets.primer);
}

// ---------------------------------------------------------------------------
// Compose: UserPromptSubmit smart-inject (≤ 2.5K tokens — design §4.2)
// ---------------------------------------------------------------------------

export function composeSmartInject(inputs: SmartInjectInputs): string {
  const sections: string[] = [];
  sections.push("<mneme-context>");

  if (inputs.driftActive) {
    sections.push(
      "<mneme-redirect>\n" +
        "Drift detected: recent assistant responses diverged from the goal stack.\n" +
        "Re-anchor to the active step before proceeding.\n" +
        "</mneme-redirect>",
    );
  }

  sections.push(
    joinSection(
      "Prior decisions relevant to your prompt",
      inputs.decisions
        .slice(0, 5)
        .map(
          (d) =>
            `${d.topic} → ${d.chosen} (because: ${d.reasoning.slice(0, 80)})`,
        ),
    ),
  );

  sections.push(
    joinSection(
      "Related conversation history",
      inputs.conversationTurns
        .slice(0, 5)
        .map(
          (t) =>
            `${t.role} @ ${t.timestamp.slice(0, 19)}: ${t.content.slice(0, 140)}`,
        ),
    ),
  );

  sections.push(
    joinSection(
      "Active constraints",
      inputs.constraints.slice(0, 6).map((c) => `${c.rule} (${c.severity})`),
    ),
  );

  if (inputs.findings.length > 0) {
    sections.push(
      joinSection(
        "Open findings in scope",
        inputs.findings
          .slice(0, 5)
          .map((f) => `[${f.severity}] ${f.file}:${f.line ?? "?"} ${f.rule}`),
      ),
    );
  }

  sections.push("</mneme-context>");
  const raw = sections.filter(Boolean).join("\n\n");
  return clampToBudget(raw, TokenBudgets.smart_inject);
}

// ---------------------------------------------------------------------------
// Compose: resume bundle (design §7.3)
// ---------------------------------------------------------------------------

export function composeResume(inputs: ResumeInputs): string {
  const cur = inputs.currentStep;
  const total =
    inputs.completedSteps.length + (cur ? 1 : 0) + inputs.plannedSteps.length;
  const k = inputs.completedSteps.length + 1;

  const sections: string[] = [];
  sections.push("<mneme-resume>");
  sections.push(`You are paused at STEP ${k} of ${total}.`);
  sections.push(`\n## Original goal (verbatim from session start)\n${inputs.goalText}`);

  sections.push(
    `\n## Goal stack (root → current leaf)\n` +
      inputs.goalStack
        .map((s, i) => `${"  ".repeat(i)}- ${s.step_id}: ${s.description}`)
        .join("\n"),
  );

  sections.push(
    `\n## Completed steps (1..${k - 1})\n` +
      inputs.completedSteps
        .map(
          (s) =>
            `- [${s.step_id}] ${s.description}\n  proof: ${s.verification_proof?.slice(0, 120) ?? "—"}`,
        )
        .join("\n"),
  );

  if (cur) {
    sections.push(
      `\n## YOU ARE HERE — Step ${cur.step_id}\n` +
        `Description: ${cur.description}\n` +
        `Started: ${cur.started_at ?? "not started"}\n` +
        `Last action: ${(cur.notes ?? "").split("\n").pop() ?? "—"}\n` +
        `Stuck on: ${cur.blocker ?? "—"}\n` +
        `Acceptance: ${cur.acceptance_cmd ?? JSON.stringify(cur.acceptance_check) ?? "—"}`,
    );
  }

  sections.push(
    `\n## Planned steps (${k + 1}..${total})\n` +
      inputs.plannedSteps
        .map((s) => `- [${s.step_id}] ${s.description}`)
        .join("\n"),
  );

  sections.push(
    joinSection(
      "Active constraints (must honor)",
      inputs.constraints.map((c) => c.rule),
    ),
  );

  if (cur?.acceptance_cmd) {
    sections.push(`\n## Verification gate\n\`${cur.acceptance_cmd}\` must exit 0.`);
  }

  sections.push("</mneme-resume>");
  return clampToBudget(sections.join("\n"), TokenBudgets.max_total_per_turn);
}

// ---------------------------------------------------------------------------
// High-level convenience composers (used by hooks)
// ---------------------------------------------------------------------------

export interface ComposeOptions {
  cwd: string;
  sessionId: string;
  /** Override the per-call token budget. Defaults to the design-spec value. */
  tokenBudget?: number;
}

/**
 * Build the Project Identity Kernel block (F9) and return it as a compact
 * markdown string. Safe to call before the shard exists — returns "" on
 * any failure so the primer always gets *something*.
 */
async function buildIdentityBlock(opts: ComposeOptions): Promise<string> {
  try {
    const { tool: identityTool } = await import("./tools/identity.ts");
    const result = await identityTool.handler(
      { project: opts.cwd },
      { cwd: opts.cwd, sessionId: opts.sessionId },
    );
    const lines: string[] = [];
    lines.push("<mneme-identity>");
    lines.push(`Project: ${result.name}`);
    if (result.stack.length > 0) {
      lines.push("\n## Stack");
      for (const t of result.stack) {
        lines.push(t.version ? `- ${t.name} (${t.version})` : `- ${t.name}`);
      }
    }
    if (result.domain_summary) {
      lines.push("\n## What it does");
      lines.push(result.domain_summary);
    }
    if (result.key_concepts.length > 0) {
      lines.push("\n## Key concepts");
      for (const c of result.key_concepts.slice(0, 10)) {
        lines.push(`- ${c.term}`);
      }
    }
    if (result.conventions.length > 0) {
      lines.push("\n## Conventions (top by confidence)");
      for (const c of result.conventions.slice(0, 5)) {
        lines.push(`- [${Math.round(c.confidence * 100)}%] ${c.description}`);
      }
    }
    if (result.recent_goals.length > 0) {
      lines.push("\n## Recent goals");
      for (const g of result.recent_goals.slice(0, 5)) {
        lines.push(`- ${g}`);
      }
    }
    if (result.open_questions.length > 0) {
      lines.push("\n## Open questions");
      for (const q of result.open_questions.slice(0, 5)) {
        lines.push(`- ${q}`);
      }
    }
    lines.push("</mneme-identity>");
    return lines.join("\n");
  } catch {
    return "";
  }
}

/** Build a SessionStart primer by querying every relevant shard. */
export async function buildPrimer(opts: ComposeOptions): Promise<string> {
  const [decisions, constraints, todos, findings, dirtyFiles, goal, identity] = await Promise.all([
    dbQuery
      .select<Decision>("decisions", "1=1 ORDER BY timestamp DESC LIMIT 3")
      .catch(() => []),
    dbQuery
      .select<Constraint>("constraints", "1=1 ORDER BY severity DESC LIMIT 5")
      .catch(() => []),
    dbQuery
      .select<Todo>("tasks", "status = 'open' ORDER BY created_at DESC LIMIT 5")
      .catch(() => []),
    dbQuery
      .select<Finding>(
        "findings",
        "severity IN ('high','critical') ORDER BY detected_at DESC LIMIT 5",
      )
      .catch(() => []),
    dbQuery
      .raw<string[]>("query.dirty_files", { cwd: opts.cwd })
      .catch(() => [] as string[]),
    dbQuery
      .raw<string | null>("query.current_goal", { session_id: opts.sessionId })
      .catch(() => null),
    buildIdentityBlock(opts).catch(() => ""),
  ]);

  const baseline = composePrimer({
    ctx: { cwd: opts.cwd, sessionId: opts.sessionId },
    ...(goal != null ? { goal } : {}),
    decisions,
    constraints,
    todos,
    dirtyFiles,
    redFindings: findings,
  });

  // F9: prepend the Identity Kernel so the model sees "who/what/why" before
  // the goal/constraints block. Kept separate from composePrimer so unit
  // tests for that function remain stable.
  if (identity) {
    return `${identity}\n\n${baseline}`;
  }
  return baseline;
}

/** Build a UserPromptSubmit smart-inject bundle. */
export async function buildSmartInject(
  opts: ComposeOptions & { prompt: string },
): Promise<string> {
  const [decisions, conversationTurns, constraints, findings, driftActive] =
    await Promise.all([
      dbQuery
        .semanticSearch<Decision>("decisions", opts.prompt, 5)
        .catch(() => []),
      dbQuery
        .semanticSearch<ConversationTurn>("history", opts.prompt, 5)
        .catch(() => []),
      dbQuery
        .select<Constraint>(
          "constraints",
          "scope IN ('global','project') ORDER BY severity DESC LIMIT 6",
        )
        .catch(() => []),
      dbQuery
        .select<Finding>(
          "findings",
          "severity IN ('high','critical') ORDER BY detected_at DESC LIMIT 5",
        )
        .catch(() => []),
      dbQuery
        .raw<boolean>("query.drift_active", { session_id: opts.sessionId })
        .catch(() => false),
    ]);

  return composeSmartInject({
    ctx: { cwd: opts.cwd, sessionId: opts.sessionId },
    prompt: opts.prompt,
    decisions,
    conversationTurns,
    constraints,
    findings,
    driftActive,
  });
}

/** Build the resumption bundle (called by /mn-step or after compaction). */
export async function buildResume(opts: ComposeOptions): Promise<{
  bundle: string;
  current_step_id: string | null;
  total_steps: number;
}> {
  const allSteps = await dbQuery
    .select<Step>("tasks", "session_id = ?", [opts.sessionId])
    .catch(() => [] as Step[]);

  const completed = allSteps.filter((s) => s.status === "completed");
  const planned = allSteps.filter((s) => s.status === "not_started");
  const current =
    allSteps.find((s) => s.status === "in_progress") ??
    allSteps.find((s) => s.status === "blocked") ??
    null;

  const constraints = await dbQuery
    .select<Constraint>("constraints", "scope IN ('global','project') LIMIT 10")
    .catch(() => [] as Constraint[]);

  const goalText = await dbQuery
    .raw<string | null>("query.session_goal_text", { session_id: opts.sessionId })
    .catch(() => null);

  const bundle = composeResume({
    ctx: { cwd: opts.cwd, sessionId: opts.sessionId },
    goalText: goalText ?? "(no recorded goal)",
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
