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

    const [topTools, trespasses] = await Promise.all([
      Promise.resolve(pickTopTools(args.prompt, 3)),
      getSessionTrespasses(args.sessionId, 5),
    ]);

    const additionalContext = buildReminderBlock(topTools, trespasses);

    return {
      hook_specific: { additionalContext },
    };
  } catch (err) {
    console.error("[mneme-mcp] userprompt-submit hook failed:", errMsg(err));
    return empty;
  }
}

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
): string {
  const lines: string[] = [];

  lines.push("<mneme-self-ping>");
  lines.push("IMPORTANT: Use mneme MCP tools BEFORE grep/read/bash for code exploration.");
  lines.push("");
  lines.push("Top 3 mneme tools for this prompt:");
  for (const t of tools) {
    lines.push(`  • ${t.name}`);
    lines.push(`    Why: ${t.why}`);
  }

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
