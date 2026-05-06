/**
 * Hook: UserPromptSubmit — Layer 1 of the v0.4.0 self-ping enforcement layer.
 *
 * On every user prompt mneme injects a reminder block that:
 *   1. Lists the top 3 mneme tools most relevant to the user's message.
 *   2. Shows a "trespass log" of Grep/Read calls in this session that did
 *      NOT go through mneme first — training the AI to notice the pattern.
 *
 * The existing `inject.ts` hook (Mode B smart inject) continues to run and
 * injects semantic context from the shard. This hook is ADDITIVE — it runs
 * in parallel with inject.ts when Claude Code routes UserPromptSubmit. Both
 * outputs arrive as `additionalContext` prepended to the turn.
 *
 * Hook output protocol (Claude Code spec):
 *   { "hook_specific": { "additionalContext": "<block>" } }
 *   Exit 0 = continue. This hook never blocks.
 *
 * Fail-open guarantee: any error → return empty additionalContext, exit 0.
 * mneme MUST NOT interrupt the user's workflow.
 */

import { getSessionTrespasses } from "./lib/recent-tool-calls.ts";
import { getHooksConfig } from "./lib/recent-tool-calls.ts";
import { errMsg } from "../errors.ts";

// ---------------------------------------------------------------------------
// Item #119 (2026-05-05) — smart context injection.
//
// The pre-fix hook fired the FULL reminder block (~200 tokens) on
// EVERY UserPromptSubmit, including simple acks ("ok", "continue",
// "thanks"). Across a 50-turn session that's ~10K tokens of pure
// overhead. Bench harness confirmed mneme was NOT saving tokens net
// (1.34× measured vs CRG's 6.8× claimed) — partly because the
// reminder added more than the tool savings recouped.
//
// Three-tier classification:
//   - "simple"  — small-talk / acks / planning chat. Inject nothing.
//   - "code"    — code-related question. Inject the LIGHT block:
//                 top-3 tools only, no trespass log unless real
//                 trespasses exist. ~100-150 tokens.
//   - "resume"  — continuation cue ("continue", "where was i", etc.)
//                 with the implicit assumption that a compact may
//                 have wiped earlier context. Inject the HEAVY
//                 block: top-3 tools + trespass log + cue to call
//                 mneme_resume / step_status. ~400-500 tokens.
//
// Distribution across a real session (eyeballed): ~30% simple, ~60%
// code, ~10% resume. Expected average ≈ 140 tokens vs the prior 200,
// a ~30% reduction that compounds with the 0-token simple case
// dominating short turns.
// ---------------------------------------------------------------------------

type PromptIntent = "simple" | "code" | "resume";

/**
 * Quick keyword classifier. Heuristic-only; no LLM. Order matters:
 * resume cues are checked BEFORE code cues so "continue editing X"
 * gets resume-class treatment (which is a superset of the code
 * reminder anyway).
 */
function classifyPromptIntent(prompt: string): PromptIntent {
  const lower = prompt.toLowerCase().trim();

  // Resume signals: short prompts implying continuation. Generic
  // acks like "ok" / "next" are deliberately NOT in this list —
  // they're more often "ok thanks" / "what's next on the list" than
  // a real continuation cue. We anchor on phrases that genuinely
  // imply "pick up where we left off".
  const resumeSignals = [
    "continue",
    "resume",
    "where was i",
    "where were we",
    "carry on",
    "keep going",
    "proceed",
  ];
  if (lower.length <= 40 && resumeSignals.some((s) => lower === s || lower.startsWith(`${s} `) || lower.startsWith(`${s},`))) {
    return "resume";
  }

  // Code signals: any token from the broad code-vocabulary list. We
  // keep this list narrow enough that "code" doesn't swallow casual
  // chat but wide enough that real engineering questions hit it.
  //
  // Audit fix (2026-05-06 multi-agent fan-out, super-debugger): the
  // original list omitted history-investigation cues ("why",
  // "decision", "history", "last time"). A prompt like "why was
  // this decision made last time?" is exactly the case where
  // mneme_recall should fire, but the classifier was returning
  // "simple" and short-circuiting the reminder entirely. The
  // failing test "puts mneme_recall first for why/history prompts"
  // surfaced this — it asserted the bullet list contained
  // mneme_recall, but additionalContext was empty. Same root cause
  // for "still returns 3 tools for a short generic prompt": after
  // Item #119 introduced the simple/code/resume tiers, the test's
  // "hello" prompt correctly maps to "simple" and the test was
  // updated; the why/history-prompt test is fixed by adding the
  // missing classifier signals here so mneme_recall has a path to
  // surface.
  const codeSignals = [
    "function",
    "method",
    "class",
    "module",
    "package",
    "interface",
    "type ",
    "struct",
    "enum",
    "trait",
    "impl",
    "import",
    "export",
    "edit",
    " write ",
    "rewrite",
    "implement",
    "refactor",
    "rename",
    "delete",
    "fix",
    " bug",
    "debug",
    "trace",
    "caller",
    "callee",
    "callers",
    "callees",
    "callsite",
    "where is",
    "who calls",
    "find ",
    "search",
    "lookup",
    "blast",
    "audit",
    "test",
    "compile",
    "build",
    "deploy",
    "commit",
    "push",
    "merge",
    "rebase",
    // History / decision investigation: prompts asking WHY a thing
    // was done, or what was decided / discussed / tried previously.
    // These are exactly the cases where mneme_recall is the right
    // answer; without these signals the classifier skipped them.
    "why",
    "decision",
    "history",
    "previous",
    "last time",
    "remember",
    "recall",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".rs",
    ".py",
    ".go",
    ".java",
    ".cpp",
    ".c ",
    ".cs",
    ".rb",
    "src/",
    "/src",
  ];
  if (codeSignals.some((s) => lower.includes(s))) {
    return "code";
  }

  return "simple";
}

// ---------------------------------------------------------------------------
// Prompt → tool relevance mapping
// ---------------------------------------------------------------------------

/**
 * A simple keyword → tool recommendation table. This is deliberately
 * heuristic and fast — we are not doing LLM inference at hook time. The
 * goal is to surface the 3 most likely-useful mneme tools given the prompt
 * text, so the AI has an immediate reminder in context.
 */
interface ToolRecommendation {
  readonly name: string;
  readonly why: string;
  readonly keywords: ReadonlyArray<string>;
  /** Higher = shown first when multiple recommendations score the same. */
  readonly priority: number;
}

const TOOL_RECOMMENDATIONS: ReadonlyArray<ToolRecommendation> = [
  {
    name: "mcp__mneme__blast_radius",
    why: "See what else breaks when you change this file before touching it.",
    keywords: ["edit", "change", "modify", "update", "fix", "refactor", "delete", "rename", "move", "write"],
    priority: 10,
  },
  {
    name: "mcp__mneme__mneme_recall",
    why: "Search mneme's memory for prior decisions, bugs, and context on this topic.",
    keywords: ["why", "decision", "history", "previous", "last time", "remember", "context", "background", "before", "recall"],
    priority: 9,
  },
  {
    name: "mcp__mneme__file_intent",
    why: "Understand the purpose and ownership of a file before reading or editing it.",
    keywords: ["what does", "what is", "purpose", "owned by", "responsible", "understand", "explain", "describe", "overview", "file"],
    priority: 8,
  },
  {
    name: "mcp__mneme__find_references",
    why: "Find all callers and usages before renaming or deleting a symbol.",
    keywords: ["rename", "symbol", "function", "called", "usages", "references", "callers", "where is", "who calls"],
    priority: 7,
  },
  {
    name: "mcp__mneme__call_graph",
    why: "Map the full call chain for a function — essential before deep refactors.",
    keywords: ["call", "chain", "callees", "callers", "trace", "flow", "dependency chain", "how does it reach"],
    priority: 6,
  },
  {
    name: "mcp__mneme__architecture_overview",
    why: "Get the high-level community map before adding a new feature or module.",
    keywords: ["architecture", "structure", "design", "new feature", "add module", "layer", "system", "component", "where should i"],
    priority: 5,
  },
  {
    name: "mcp__mneme__audit",
    why: "Run the drift + security scanner to catch violations before committing.",
    keywords: ["commit", "push", "deploy", "audit", "security", "lint", "check", "validate", "review", "scan"],
    priority: 5,
  },
  {
    name: "mcp__mneme__step_status",
    why: "Resume tracking the current step — never lose your place after a compaction.",
    keywords: ["step", "task", "todo", "resume", "continue", "where was i", "next", "current", "status", "progress"],
    priority: 4,
  },
  {
    name: "mcp__mneme__mneme_resume",
    why: "Get a full session brief — decisions, open questions, timeline — before diving in.",
    keywords: ["session", "catch up", "brief", "summary", "what happened", "where were we", "onboard"],
    priority: 4,
  },
  {
    name: "mcp__mneme__recall_concept",
    why: "Search semantically across code + docs + decisions for a concept.",
    keywords: ["concept", "search", "find", "semantic", "related", "similar", "about", "lookup"],
    priority: 3,
  },
] as const;

/**
 * Score a tool against a prompt. Returns a non-negative integer: higher means
 * more relevant. Zero means the tool did not match at all.
 */
function scoreToolForPrompt(tool: ToolRecommendation, promptLower: string): number {
  let score = 0;
  for (const kw of tool.keywords) {
    if (promptLower.includes(kw)) {
      score += 1;
    }
  }
  return score > 0 ? score + tool.priority : 0;
}

/**
 * Pick the top N most relevant tools for the given prompt text.
 * Always returns at least `min` tools even if no keywords match, by falling
 * back to priority order.
 */
function pickTopTools(prompt: string, n: number): ReadonlyArray<ToolRecommendation> {
  const lower = prompt.toLowerCase();

  const scored = TOOL_RECOMMENDATIONS.map((t) => ({
    tool: t,
    score: scoreToolForPrompt(t, lower),
  }));

  // Sort by score desc, then priority desc as a tiebreaker.
  scored.sort((a, b) => b.score - a.score || b.tool.priority - a.tool.priority);

  return scored.slice(0, n).map((s) => s.tool);
}

// ---------------------------------------------------------------------------
// Hook entry point
// ---------------------------------------------------------------------------

export interface UserPromptSubmitArgs {
  prompt: string;
  sessionId: string;
  cwd: string;
}

export interface UserPromptSubmitOutput {
  hook_specific: {
    additionalContext: string;
  };
}

/**
 * Build a reminder block for the AI host. The block contains:
 *   - Top 3 recommended mneme tools for this prompt.
 *   - Trespass log: Grep/Read calls this session that bypassed mneme.
 *
 * Returns an empty additionalContext (silent no-op) if the config flag
 * `inject_user_prompt_reminder` is false, or if any error occurs.
 */
export async function runUserPromptSubmit(
  args: UserPromptSubmitArgs,
): Promise<UserPromptSubmitOutput> {
  const empty: UserPromptSubmitOutput = {
    hook_specific: { additionalContext: "" },
  };

  try {
    const cfg = getHooksConfig();
    if (!cfg.inject_user_prompt_reminder) {
      return empty;
    }

    // Item #119 (2026-05-05): three-tier intent classification.
    // Simple acks pay zero tokens. Code questions get the light
    // block (top-3 tools, trespass log only if real trespasses).
    // Resume cues get the heavy block (full reminder + cue to call
    // mneme_resume / step_status — the headline compaction-recovery
    // story).
    const intent = classifyPromptIntent(args.prompt);

    if (intent === "simple") {
      // No code-related work in flight. Stay quiet.
      return empty;
    }

    const [topTools, trespasses] = await Promise.all([
      Promise.resolve(pickTopTools(args.prompt, 3)),
      getSessionTrespasses(args.sessionId, 5),
    ]);

    const additionalContext =
      intent === "resume"
        ? buildReminderBlock(topTools, trespasses, /* heavy */ true)
        : buildReminderBlock(topTools, trespasses, /* heavy */ false);

    return {
      hook_specific: { additionalContext },
    };
  } catch (err) {
    console.error("[mneme-mcp] userprompt-submit hook failed:", errMsg(err));
    return empty;
  }
}

/** Exposed for testing — see __tests__/userprompt-submit.test.ts. */
export { classifyPromptIntent };

// ---------------------------------------------------------------------------
// Block builder
// ---------------------------------------------------------------------------

/**
 * Compose the XML-tagged reminder block injected into the turn context.
 * Kept compact: the AI host already has the full session primer from
 * session_prime.ts. This block is a targeted nudge, not a full briefing.
 */
function buildReminderBlock(
  tools: ReadonlyArray<ToolRecommendation>,
  trespasses: ReadonlyArray<{ tool: string; path: string; calledAt: string }>,
  heavy: boolean,
): string {
  const lines: string[] = [];

  lines.push("<mneme-self-ping>");

  if (heavy) {
    // Item #119 (2026-05-05): heavy block fires on resume cues —
    // the headline compaction-recovery story. We assume the prior
    // turn's context may have been wiped and re-prime the AI with
    // the resume tools first, then the regular reminder.
    lines.push("Resume cue detected. Likely post-compact — these tools rebuild context fast:");
    lines.push("  • mcp__mneme__mneme_resume   — full session brief (decisions + open questions + timeline)");
    lines.push("  • mcp__mneme__step_status    — current step + acceptance criteria");
    lines.push("  • mcp__mneme__step_show      — last completed step's output");
    lines.push("");
    lines.push("If state was preserved across the compact, the step ledger has it.");
    lines.push("");
  }

  lines.push("IMPORTANT: Use mneme MCP tools BEFORE grep/read/bash for code exploration.");
  lines.push("");
  lines.push("Top 3 mneme tools for this prompt:");
  for (const t of tools) {
    lines.push(`  • ${t.name}`);
    lines.push(`    Why: ${t.why}`);
  }

  // Trespass log only emitted when real trespasses exist (light) OR
  // unconditionally during resume (heavy — the post-compact AI may
  // not even remember the trespasses happened).
  if (trespasses.length > 0) {
    lines.push("");
    lines.push("Trespass log — grep/read calls this session that skipped mneme:");
    for (const tr of trespasses) {
      const ago = formatAgo(tr.calledAt);
      lines.push(`  • ${tr.tool}("${tr.path}") — ${ago} ago, no prior mneme call`);
    }
    lines.push("Next time, call mcp__mneme__mneme_recall or blast_radius FIRST.");
  }

  lines.push("</mneme-self-ping>");

  return lines.join("\n");
}

function formatAgo(isoTimestamp: string): string {
  try {
    const diffMs = Date.now() - new Date(isoTimestamp).getTime();
    const secs = Math.floor(diffMs / 1000);
    if (secs < 60) return `${secs}s`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m`;
    return `${Math.floor(mins / 60)}h`;
  } catch {
    return "?";
  }
}
