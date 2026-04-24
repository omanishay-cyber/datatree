//! `mneme inject` — UserPromptSubmit hook entry point.
//!
//! Claude Code calls this after the user submits a prompt. We forward the
//! prompt to the supervisor, which composes a "smart inject bundle"
//! (§4.2): recent decisions, active constraints, blast-radius previews,
//! drift redirect, and the current step from the ledger.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code delivers the payload on STDIN as JSON:
//!
//! ```json
//! { "session_id": "...", "hook_event_name": "UserPromptSubmit",
//!   "prompt": "...", "cwd": "..." }
//! ```
//!
//! Manual testing from a shell uses `--prompt`, `--session-id`, `--cwd`.
//! Both paths work; CLI flags win when both present. See
//! [`crate::hook_payload`] for the merge logic.
//!
//! If STDIN is a TTY and no flags are passed, all fields default to
//! safe empty values and we emit an empty `additional_context`. The
//! rule is hard: **this hook NEVER exits non-zero**. It was the
//! deepest-blast-radius hook in the v0.3.0 self-trap (it gated
//! UserPromptSubmit — a non-zero exit muted the user), and must never
//! block a prompt because of an internal failure of mneme.
//!
//! ## v0.3.1+ — skill prescription
//!
//! When the payload carries a `prompt`, the hook also runs a minimal
//! in-process skill matcher against `~/.mneme/plugin/skills/` (see
//! [`crate::skill_matcher`]) and, if the top suggestion fires at
//! `medium` or `high` confidence, appends a
//! `<mneme-skill-prescription>` block to the emitted
//! `additional_context`. Pass `--no-skill-hint` to skip this.
//!
//! Output format is the JSON shape Claude Code expects from a
//! UserPromptSubmit hook:
//!
//! ```json
//! { "hookEventName": "UserPromptSubmit",
//!   "additional_context": "<mneme-context>...</mneme-context>" }
//! ```

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
use crate::ipc::{IpcRequest, IpcResponse};
use crate::skill_matcher::{reason_for, suggest, Confidence, Suggestion};

/// CLI args for `mneme inject`. All optional — STDIN JSON fills in
/// anything missing.
#[derive(Debug, Args)]
pub struct InjectArgs {
    /// The user prompt as captured by the hook. If absent, read from
    /// STDIN payload `.prompt` or treated as empty.
    #[arg(long)]
    pub prompt: Option<String>,

    /// Session id assigned by the host. If absent, read from STDIN
    /// `.session_id` or defaulted to `"unknown"`.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,

    /// Working directory at the time the hook fired. If absent, read
    /// from STDIN `.cwd` or the process CWD.
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Skip the `<mneme-skill-prescription>` block. Useful when the
    /// user wants the supervisor's context without any skill-router
    /// nudge.
    #[arg(long = "no-skill-hint", default_value_t = false)]
    pub no_skill_hint: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: InjectArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // Read STDIN payload; log and continue on any parse error so we never
    // block the user's prompt because of our own bug. See module docs.
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "hook STDIN parse failed; falling back to CLI flags / empty");
            None
        }
    };

    let stdin_prompt = stdin_payload.as_ref().and_then(|p| p.prompt.clone());
    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    let stdin_cwd = stdin_payload.as_ref().and_then(|p| p.cwd.clone());

    let prompt = choose(args.prompt, stdin_prompt, String::new());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());
    let cwd = choose(
        args.cwd,
        stdin_cwd,
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );

    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::Inject {
            prompt: prompt.clone(),
            session_id,
            cwd,
        })
        .await;

    let mut payload = match response {
        Ok(IpcResponse::Ok { message }) => message.unwrap_or_default(),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "supervisor returned error; emitting empty additional_context");
            String::new()
        }
        Ok(IpcResponse::Pong)
        | Ok(IpcResponse::Status { .. })
        | Ok(IpcResponse::Logs { .. })
        | Ok(IpcResponse::JobQueued { .. })
        | Ok(IpcResponse::JobQueue { .. })
        | Ok(IpcResponse::RecallResults { .. })
        | Ok(IpcResponse::BlastResults { .. })
        | Ok(IpcResponse::GodNodesResults { .. }) => String::new(),
        Err(e) => {
            warn!(error = %e, "supervisor unreachable; emitting empty additional_context");
            String::new()
        }
    };

    // Append the skill-router recommendation when:
    //   - the user actually typed something (skip empty prompts),
    //   - the caller did not pass --no-skill-hint,
    //   - the top suggestion fires at medium or high confidence.
    if !args.no_skill_hint && !prompt.trim().is_empty() {
        match std::panic::catch_unwind(|| suggest(&prompt, 1)) {
            Ok(hits) => {
                if let Some(hit) = hits.into_iter().next() {
                    if matches!(hit.confidence, Confidence::Medium | Confidence::High) {
                        let block = render_skill_block(&prompt, &hit);
                        if payload.is_empty() {
                            payload = block;
                        } else {
                            payload.push_str("\n\n");
                            payload.push_str(&block);
                        }
                    }
                }
            }
            Err(_) => {
                warn!("skill matcher panicked; dropping skill prescription");
            }
        }
    }

    let out = json!({
        "hookEventName": "UserPromptSubmit",
        "additional_context": payload,
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}

/// Render a single `<mneme-skill-prescription>` block. Kept ASCII-only
/// — the user's Windows cp1252 terminal breaks on em-dashes and other
/// fancy punctuation.
fn render_skill_block(prompt: &str, hit: &Suggestion) -> String {
    let excerpt = excerpt(prompt, 120);
    // `to_load` is a plain `cat` against the absolute SKILL.md path so
    // the assistant can load the skill without the MCP server being up.
    // The path is the one mneme actually parsed, so dev-tree runs work
    // the same as installed-plugin runs.
    let source = hit.source_path.to_string_lossy();
    let reason = reason_for(hit);
    format!(
        concat!(
            "<mneme-skill-prescription>\n",
            "  task: {task}\n",
            "  recommended_skill: {skill}\n",
            "  confidence: {confidence}\n",
            "  reason: {reason}\n",
            "  to_load: cat {path}\n",
            "</mneme-skill-prescription>",
        ),
        task = excerpt,
        skill = hit.skill,
        confidence = hit.confidence.as_str(),
        reason = reason,
        path = source,
    )
}

/// Collapse whitespace + truncate so the excerpt never blows out the
/// hook JSON. Keeps output single-line-friendly.
fn excerpt(raw: &str, max_chars: usize) -> String {
    let collapsed: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(max_chars).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn excerpt_collapses_and_truncates() {
        let long = "  hello\n  world  this  is   a   very   long   prompt   that   must   be   truncated ";
        let out = excerpt(long, 30);
        assert!(out.starts_with("hello world"));
        assert!(out.len() <= 33); // 30 chars + "..."
        assert!(out.ends_with("..."));
    }

    #[test]
    fn render_block_is_ascii() {
        let hit = Suggestion {
            skill: "fireworks-debug".to_string(),
            triggers_matched: vec!["debug".to_string()],
            tags_matched: Vec::new(),
            confidence: Confidence::Medium,
            source_path: PathBuf::from("/tmp/SKILL.md"),
            score: 2,
        };
        let block = render_skill_block("debug a test", &hit);
        assert!(block.is_ascii());
        assert!(block.contains("recommended_skill: fireworks-debug"));
        assert!(block.contains("confidence: medium"));
        assert!(block.contains("to_load: cat /tmp/SKILL.md"));
    }
}
