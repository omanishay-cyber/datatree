//! `mneme turn-end` — Stop hook entry point.
//!
//! Triggers the summarizer and updates the Step Ledger drift score.
//! The optional flags let the same handler be reused for PreCompact (so
//! we can flush the ledger to disk before compaction) and SubagentStop
//! (so per-subagent turns are accounted separately).
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code Stop / SubagentStop / PreCompact events all deliver:
//! ```json
//! { "session_id": "...", "hook_event_name": "Stop",
//!   "stop_hook_active": true|false }
//! ```
//!
//! Stop-hook failures ALWAYS exit 0 with a stderr warning. Claude Code
//! retries Stop hooks up to 5× on non-zero exit (see report-002.md §F-016
//! / §9.3 R5); on v0.3.0 this amplified the self-trap into a ~20-round
//! retry loop. v0.3.1's rule: a broken turn-end never gates the user's
//! reply. Log the problem, exit 0.
//!
//! We also honor `stop_hook_active=true` as an immediate no-op — that
//! flag means a previous turn-end already emitted `decision: "block"`;
//! firing again would loop forever.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
use crate::ipc::IpcRequest;

/// CLI args for `mneme turn-end`.
#[derive(Debug, Args)]
pub struct TurnEndArgs {
    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,

    /// This hook fired right before a compaction event.
    #[arg(long = "pre-compact")]
    pub pre_compact: bool,

    /// This hook fired at the end of a subagent turn rather than the main
    /// model.
    #[arg(long)]
    pub subagent: bool,
}

/// Entry point used by `main.rs`. Always exits 0.
pub async fn run(args: TurnEndArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "turn-end STDIN parse failed; falling back to CLI flags");
            None
        }
    };

    // Short-circuit loops per Claude Code's retry contract.
    if let Some(p) = stdin_payload.as_ref() {
        if p.stop_hook_active.unwrap_or(false) {
            // Previous turn-end already blocked; re-running would loop.
            return Ok(());
        }
    }

    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    let stdin_event = stdin_payload
        .as_ref()
        .and_then(|p| p.hook_event_name.clone())
        .unwrap_or_default();
    // The payload's hook_event_name lets STDIN-only invocations distinguish
    // Stop / SubagentStop / PreCompact without extra CLI flags.
    let pre_compact_from_stdin = stdin_event == "PreCompact";
    let subagent_from_stdin = stdin_event == "SubagentStop";
    let pre_compact = args.pre_compact || pre_compact_from_stdin;
    let subagent = args.subagent || subagent_from_stdin;

    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());

    let client = make_client(socket_override);
    let suffix = match (pre_compact, subagent) {
        (true, _) => ":pre-compact",
        (_, true) => ":subagent",
        _ => "",
    };
    let session_id = format!("{}{}", session_id, suffix);
    if let Err(e) = client
        .request(IpcRequest::TurnEnd { session_id })
        .await
    {
        warn!(error = %e, "turn-end notification skipped (supervisor unreachable)");
    }
    Ok(())
}
