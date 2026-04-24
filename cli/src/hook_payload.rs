//! Shared STDIN payload parsing for every hook entry point.
//!
//! Claude Code (and every other AI platform that supports hooks in the MCP
//! ecosystem) delivers hook payloads as a single JSON object written to
//! the hook binary's STDIN. The binary is expected to:
//!
//!   * read the entire STDIN,
//!   * parse a JSON object (UTF-8, no framing),
//!   * exit 0 to allow the underlying operation (or emit any STDOUT the
//!     host's hook protocol expects),
//!   * exit non-zero ONLY to block the operation.
//!
//! Before v0.3.1 mneme's hook binaries required `--tool / --params /
//! --session-id / --prompt / --cwd` as CLI flags. Claude Code never
//! passes flags — it passes JSON on STDIN. The result was the self-trap
//! documented in `report-002.md §F-012` / `txt proof.txt lines 426+,
//! 6369-6393`: every invocation exited non-zero with "required arguments
//! not provided", Claude Code interpreted that as BLOCK, every tool call
//! was denied, and the user was muted at the UserPromptSubmit hook.
//!
//! This module supplies:
//!
//!   * [`HookPayload`]   — the shape Claude Code writes on STDIN
//!   * [`read_stdin_payload`] — read + parse, TTY-aware
//!   * [`choose`]        — helper combinator: CLI flag wins over STDIN
//!                         field wins over default
//!
//! Individual hook commands call `read_stdin_payload()` once at the top,
//! then resolve each field via `choose(args.foo, payload.foo, default)`.
//! CLI flags retain their priority so manual testing (`mneme pre-tool
//! --tool Bash --params '{}' --session-id t`) still works.

use serde::Deserialize;
use serde_json::Value;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

/// The payload Claude Code writes to a hook binary's STDIN.
///
/// Field coverage is a superset of the Claude Code hook schema — every
/// known hook event (PreToolUse, PostToolUse, UserPromptSubmit, Stop,
/// SubagentStop, SessionStart, SessionEnd, PreCompact) delivers some
/// subset of these fields, so we keep them all `Option` and let each
/// callsite pick what it needs.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct HookPayload {
    /// Host session id — every event includes this.
    pub session_id: Option<String>,
    /// Hook event name (e.g. `"PreToolUse"`). Informational; the binary
    /// usually already knows what event it handles.
    pub hook_event_name: Option<String>,
    /// Tool name (PreToolUse / PostToolUse only).
    pub tool_name: Option<String>,
    /// Tool parameters (PreToolUse only). Opaque JSON.
    pub tool_input: Option<Value>,
    /// Tool result (PostToolUse only). Opaque JSON.
    pub tool_response: Option<Value>,
    /// User's typed prompt (UserPromptSubmit only).
    pub prompt: Option<String>,
    /// Working directory (UserPromptSubmit / SessionStart).
    pub cwd: Option<PathBuf>,
    /// Transcript file path (SessionStart / SessionEnd / PreCompact).
    pub transcript_path: Option<PathBuf>,
    /// How the session started ("startup" / "resume" / "clear" / "compact")
    /// on SessionStart only.
    pub source: Option<String>,
    /// True when the Stop hook is re-firing because a previous Stop hook
    /// already emitted `decision: "block"`. Binaries should short-circuit
    /// to prevent infinite retry loops.
    pub stop_hook_active: Option<bool>,
}

/// Read STDIN if it isn't a terminal and parse it as `HookPayload`.
///
/// Returns:
///   * `Ok(None)`  — STDIN is a TTY (interactive invocation with flags)
///                    OR STDIN is empty. Caller should use CLI flags.
///   * `Ok(Some)`  — STDIN had JSON; payload is populated.
///   * `Err(msg)`  — STDIN wasn't a TTY and wasn't empty, but parsing
///                    failed. Hook should exit 0 with a warning — we
///                    never block on our own parse bug.
pub fn read_stdin_payload() -> Result<Option<HookPayload>, String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }
    let mut buf = String::new();
    stdin
        .lock()
        .read_to_string(&mut buf)
        .map_err(|e| format!("stdin read: {e}"))?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let payload: HookPayload = serde_json::from_str(trimmed)
        .map_err(|e| format!("hook JSON parse: {e}"))?;
    Ok(Some(payload))
}

/// Three-way fallback: prefer CLI flag value, then STDIN-payload value,
/// then a caller-supplied default. Used by every hook command so the
/// three-way merge is consistent across binaries.
pub fn choose<T>(cli: Option<T>, stdin_val: Option<T>, default: T) -> T {
    cli.or(stdin_val).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_prefers_cli() {
        let out = choose(Some(1u32), Some(2), 3);
        assert_eq!(out, 1);
    }

    #[test]
    fn choose_falls_back_to_stdin() {
        let out: u32 = choose(None, Some(2), 3);
        assert_eq!(out, 2);
    }

    #[test]
    fn choose_falls_back_to_default() {
        let out: u32 = choose(None, None, 3);
        assert_eq!(out, 3);
    }

    #[test]
    fn payload_parses_claude_code_pre_tool_shape() {
        let json = r#"{
            "session_id": "abc",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }"#;
        let p: HookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(p.session_id.as_deref(), Some("abc"));
        assert_eq!(p.tool_name.as_deref(), Some("Bash"));
        assert!(p.tool_input.is_some());
        assert!(p.prompt.is_none());
    }

    #[test]
    fn payload_parses_user_prompt_submit_shape() {
        let json = r#"{
            "session_id": "xyz",
            "hook_event_name": "UserPromptSubmit",
            "prompt": "help me debug",
            "cwd": "/some/dir"
        }"#;
        let p: HookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(p.prompt.as_deref(), Some("help me debug"));
        assert_eq!(p.cwd.as_deref().and_then(|p| p.to_str()), Some("/some/dir"));
    }

    #[test]
    fn payload_ignores_unknown_fields() {
        // Claude Code may add fields in future versions; we must not
        // error on them.
        let json = r#"{
            "session_id": "abc",
            "some_future_field": {"anything": true}
        }"#;
        let p: HookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(p.session_id.as_deref(), Some("abc"));
    }

    #[test]
    fn payload_empty_object_ok() {
        let p: HookPayload = serde_json::from_str("{}").unwrap();
        assert!(p.session_id.is_none());
    }
}
