//! `mneme session-prime` — SessionStart hook entry point.
//!
//! Claude Code calls this when a new session starts. We respond with the
//! initial primer block (recent decisions, open todos, last drift findings)
//! and the resumption bundle if a Step Ledger task is mid-flight.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code SessionStart payload:
//! ```json
//! { "session_id": "...", "hook_event_name": "SessionStart",
//!   "source": "startup" | "resume" | "clear" | "compact",
//!   "cwd": "...", "transcript_path": "..." }
//! ```
//!
//! Project is derived from `cwd` when Claude Code doesn't send an
//! explicit project path — which it doesn't. The old `--project` CLI
//! flag is still accepted for manual testing.

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `mneme session-prime`. All optional — STDIN fills in.
#[derive(Debug, Args)]
pub struct SessionPrimeArgs {
    /// Active project root. If absent, resolved from STDIN `cwd` or
    /// process CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// Session id assigned by the host.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: SessionPrimeArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "session-prime STDIN parse failed; falling back to defaults");
            None
        }
    };

    // Prefer explicit --project; else STDIN `cwd` (where the user
    // launched Claude Code); else process CWD.
    let stdin_cwd = stdin_payload.as_ref().and_then(|p| p.cwd.clone());
    let project = choose(
        args.project,
        stdin_cwd,
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );

    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());

    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::SessionPrime {
            project,
            session_id,
        })
        .await;

    let payload = match response {
        Ok(IpcResponse::Ok { message }) => message.unwrap_or_default(),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "supervisor returned error");
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
            warn!(error = %e, "supervisor unreachable");
            String::new()
        }
    };

    let out = json!({
        "hookEventName": "SessionStart",
        "additional_context": payload,
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
