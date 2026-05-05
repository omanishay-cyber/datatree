//! `mneme userprompt-submit` — Wave 1E Layer 1 hook entry.
//!
//! Claude Code's settings.json registers this hook to fire on every
//! UserPromptSubmit (alongside the existing `mneme inject` hook from
//! v0.3.x). It injects a self-ping reminder block listing the top 3
//! mneme tools relevant to the user's prompt, plus an optional
//! "trespass log" of grep/read calls in this session that bypassed
//! mneme.
//!
//! ## BUG-NEW-Q fix (2026-05-05)
//!
//! HOOK_SPECS in `cli/src/platforms/claude_code.rs` registered this
//! command in Wave 1E but the Rust CLI subcommand was never wired up.
//! Every UserPromptSubmit fired `mneme.exe userprompt-submit` →
//! clap-errored with "unrecognized subcommand" → silent failure of
//! self-ping enforcement (and a brief stderr-with-non-zero-exit
//! that contributed to BUG-NEW-H's "terminal flash" report).
//!
//! This file ships the missing subcommand. The classifier mirrors
//! `mcp/src/hooks/userprompt-submit.ts` (Item #119): three tiers —
//! simple / code / resume — with simple paying zero tokens, code
//! getting the light reminder, and resume getting the heavy
//! compaction-recovery cue.
//!
//! Hook output protocol (Claude Code spec):
//!   `{ "hook_specific": { "additionalContext": "<block>" } }` on stdout.
//!   Exit 0 always — fail-open.
//!
//! Trespass-log integration is intentionally OMITTED from this Rust
//! port. The MCP TS hook reads `tool_cache.db` for that signal; doing
//! it from Rust would mean adding a sql.js dep + IPC client just for
//! the hook. v0.4.1 follow-up. The classifier alone is the
//! load-bearing piece for the token-savings story.

use clap::Args;
use serde::Deserialize;
use serde_json::json;
use std::io::{self, Read};

use crate::error::CliResult;

/// CLI args for `mneme userprompt-submit`. All optional — payload comes
/// from stdin per Claude Code's hook contract.
#[derive(Debug, Args, Default)]
pub struct UserPromptSubmitArgs {
    /// Override stdin's `prompt` field. Mostly for manual testing.
    #[arg(long)]
    pub prompt: Option<String>,
}

/// Subset of Claude Code's hook payload we actually consume.
#[derive(Debug, Default, Deserialize)]
struct StdinPayload {
    /// User's typed prompt for this turn. Empty when the hook is
    /// invoked outside a real Claude Code turn (TTY tests, manual
    /// shell invocations).
    #[serde(default)]
    prompt: String,
}

/// Three-tier classification mirroring `mcp/src/hooks/userprompt-submit.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptIntent {
    /// Simple ack / chat. Inject nothing.
    Simple,
    /// Code-related question. Inject the light reminder.
    Code,
    /// Continuation cue. Inject the heavy compaction-recovery block.
    Resume,
}

/// Public for unit tests and parity with the TS classifier.
pub fn classify_prompt_intent(prompt: &str) -> PromptIntent {
    let lower = prompt.to_lowercase();
    let lower = lower.trim();

    // Resume signals — short prompts implying continuation. Generic
    // acks ("ok", "next", "go ahead") are deliberately excluded —
    // they're more often "ok thanks" / "what's next on the list" than
    // genuine continuation cues. Length gate (≤40 chars) prevents
    // "continue editing the spawn function" from being misclassified.
    const RESUME_SIGNALS: &[&str] = &[
        "continue",
        "resume",
        "where was i",
        "where were we",
        "carry on",
        "keep going",
        "proceed",
    ];
    if lower.len() <= 40 {
        for sig in RESUME_SIGNALS {
            if lower == *sig
                || lower.starts_with(&format!("{sig} "))
                || lower.starts_with(&format!("{sig},"))
            {
                return PromptIntent::Resume;
            }
        }
    }

    // Code signals — broad enough to catch real engineering questions,
    // narrow enough not to swallow casual chat. Order doesn't matter;
    // any single hit promotes from "simple" to "code".
    const CODE_SIGNALS: &[&str] = &[
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
    for sig in CODE_SIGNALS {
        if lower.contains(sig) {
            return PromptIntent::Code;
        }
    }

    PromptIntent::Simple
}

/// One recommended tool entry. Same shape as the TS hook's
/// `ToolRecommendation` so the prompt the AI sees stays identical
/// across the two hook surfaces.
struct ToolRec {
    name: &'static str,
    why: &'static str,
    keywords: &'static [&'static str],
    priority: u8,
}

/// Top-N picker. Keyword overlap + priority. Always returns N items
/// even on a no-keyword prompt by falling back to priority order.
const TOOL_RECS: &[ToolRec] = &[
    ToolRec {
        name: "mcp__mneme__blast_radius",
        why: "See what else breaks when you change this file before touching it.",
        keywords: &[
            "edit", "change", "modify", "update", "fix", "refactor", "delete", "rename", "move",
            "write",
        ],
        priority: 10,
    },
    ToolRec {
        name: "mcp__mneme__mneme_recall",
        why: "Search mneme's memory for prior decisions, bugs, and context on this topic.",
        keywords: &[
            "why",
            "decision",
            "history",
            "previous",
            "last time",
            "remember",
            "context",
            "background",
            "before",
            "recall",
        ],
        priority: 9,
    },
    ToolRec {
        name: "mcp__mneme__file_intent",
        why: "Understand the purpose and ownership of a file before reading or editing it.",
        keywords: &[
            "what does",
            "what is",
            "purpose",
            "owned by",
            "responsible",
            "understand",
            "explain",
            "describe",
            "overview",
            "file",
        ],
        priority: 8,
    },
    ToolRec {
        name: "mcp__mneme__find_references",
        why: "Find all callers and usages before renaming or deleting a symbol.",
        keywords: &[
            "rename",
            "symbol",
            "function",
            "called",
            "usages",
            "references",
            "callers",
            "where is",
            "who calls",
        ],
        priority: 7,
    },
    ToolRec {
        name: "mcp__mneme__call_graph",
        why: "Map the full call chain for a function — essential before deep refactors.",
        keywords: &[
            "call",
            "chain",
            "callees",
            "callers",
            "trace",
            "flow",
            "dependency chain",
            "how does it reach",
        ],
        priority: 6,
    },
    ToolRec {
        name: "mcp__mneme__architecture_overview",
        why: "Get the high-level community map before adding a new feature or module.",
        keywords: &[
            "architecture",
            "structure",
            "design",
            "new feature",
            "add module",
            "layer",
            "system",
            "component",
            "where should i",
        ],
        priority: 5,
    },
    ToolRec {
        name: "mcp__mneme__audit",
        why: "Run the drift + security scanner to catch violations before committing.",
        keywords: &[
            "commit", "push", "deploy", "audit", "security", "lint", "check", "validate", "review",
            "scan",
        ],
        priority: 5,
    },
    ToolRec {
        name: "mcp__mneme__step_status",
        why: "Resume tracking the current step — never lose your place after a compaction.",
        keywords: &[
            "step",
            "task",
            "todo",
            "resume",
            "continue",
            "where was i",
            "next",
            "current",
            "status",
            "progress",
        ],
        priority: 4,
    },
    ToolRec {
        name: "mcp__mneme__mneme_resume",
        why: "Get a full session brief — decisions, open questions, timeline — before diving in.",
        keywords: &[
            "session",
            "catch up",
            "brief",
            "summary",
            "what happened",
            "where were we",
            "onboard",
        ],
        priority: 4,
    },
    ToolRec {
        name: "mcp__mneme__recall_concept",
        why: "Search semantically across code + docs + decisions for a concept.",
        keywords: &[
            "concept", "search", "find", "semantic", "related", "similar", "about", "lookup",
        ],
        priority: 3,
    },
];

fn pick_top_tools(prompt: &str, n: usize) -> Vec<&'static ToolRec> {
    let lower = prompt.to_lowercase();
    let mut scored: Vec<(u32, &'static ToolRec)> = TOOL_RECS
        .iter()
        .map(|t| {
            let mut s = 0u32;
            for kw in t.keywords {
                if lower.contains(kw) {
                    s += 1;
                }
            }
            let total = if s > 0 { s + u32::from(t.priority) } else { 0 };
            (total, t)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.priority.cmp(&a.1.priority)));
    scored.into_iter().take(n).map(|(_, t)| t).collect()
}

fn build_reminder_block(tools: &[&ToolRec], heavy: bool) -> String {
    let mut s = String::with_capacity(512);
    s.push_str("<mneme-self-ping>\n");
    if heavy {
        s.push_str(
            "Resume cue detected. Likely post-compact — these tools rebuild context fast:\n",
        );
        s.push_str(
            "  • mcp__mneme__mneme_resume   — full session brief (decisions + open questions + timeline)\n",
        );
        s.push_str("  • mcp__mneme__step_status    — current step + acceptance criteria\n");
        s.push_str("  • mcp__mneme__step_show      — last completed step's output\n");
        s.push_str("\nIf state was preserved across the compact, the step ledger has it.\n\n");
    }
    s.push_str("IMPORTANT: Use mneme MCP tools BEFORE grep/read/bash for code exploration.\n\n");
    s.push_str("Top 3 mneme tools for this prompt:\n");
    for t in tools {
        s.push_str(&format!("  • {}\n    Why: {}\n", t.name, t.why));
    }
    s.push_str("</mneme-self-ping>");
    s
}

/// Entry point — wired into `cli/src/main.rs`.
///
/// Fail-open: any error returns an empty additionalContext + exit 0.
/// We MUST NOT block the user's prompt because of a hook bug.
pub async fn run(args: UserPromptSubmitArgs) -> CliResult<()> {
    // Read STDIN payload. Empty / malformed → empty default.
    let prompt = match args.prompt {
        Some(p) => p,
        None => read_stdin_prompt().unwrap_or_default(),
    };

    let intent = classify_prompt_intent(&prompt);

    let additional_context = match intent {
        PromptIntent::Simple => String::new(),
        PromptIntent::Code => {
            let tools = pick_top_tools(&prompt, 3);
            build_reminder_block(&tools, false)
        }
        PromptIntent::Resume => {
            let tools = pick_top_tools(&prompt, 3);
            build_reminder_block(&tools, true)
        }
    };

    let out = json!({
        "hook_specific": { "additionalContext": additional_context },
    });
    println!("{}", out);
    Ok(())
}

/// Best-effort stdin read. Returns None on any IO failure (so the
/// caller defaults to an empty prompt and a Simple intent).
fn read_stdin_prompt() -> Option<String> {
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        return None;
    }
    let payload: StdinPayload = serde_json::from_str(&buf).ok().unwrap_or_default();
    Some(payload.prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_acks_classify_simple() {
        for p in [
            "ok thanks",
            "thanks!",
            "got it",
            "great",
            "nice",
            "what next",
            "hello",
            "how are you",
        ] {
            assert_eq!(classify_prompt_intent(p), PromptIntent::Simple, "{p}");
        }
    }

    #[test]
    fn code_questions_classify_code() {
        for p in [
            "find all callers of WorkerPool::spawn",
            "rewrite the audit function",
            "edit cli/src/main.rs",
            "what's the type of EdgeKind?",
            "show me the bug in build.rs",
            "blast radius of changing PathManager",
            "fix the typescript file at vision/src/api.ts",
        ] {
            assert_eq!(classify_prompt_intent(p), PromptIntent::Code, "{p}");
        }
    }

    #[test]
    fn resume_cues_classify_resume() {
        for p in [
            "continue",
            "where was i",
            "resume",
            "carry on",
            "keep going",
            "proceed",
            "continue, please",
            "resume from where we left off",
        ] {
            assert_eq!(classify_prompt_intent(p), PromptIntent::Resume, "{p}");
        }
    }

    #[test]
    fn long_continue_prompt_is_code_not_resume() {
        // Length gate keeps "continue editing X.rs" out of resume.
        let p =
            "continue editing the WorkerPool spawn function in manager.rs and add error handling";
        assert_eq!(classify_prompt_intent(p), PromptIntent::Code);
    }

    #[test]
    fn pick_top_tools_returns_n_even_with_no_keyword_match() {
        let tools = pick_top_tools("hello", 3);
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn build_reminder_block_heavy_includes_resume_lead_in() {
        let tools = pick_top_tools("continue", 3);
        let block = build_reminder_block(&tools, true);
        assert!(block.contains("Resume cue detected"));
        assert!(block.contains("mneme_resume"));
    }

    #[test]
    fn build_reminder_block_light_omits_resume_lead_in() {
        let tools = pick_top_tools("fix the bug in foo.rs", 3);
        let block = build_reminder_block(&tools, false);
        assert!(!block.contains("Resume cue detected"));
        assert!(block.contains("Top 3 mneme tools"));
    }
}
