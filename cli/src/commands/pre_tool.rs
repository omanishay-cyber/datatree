//! `mneme pre-tool` — PreToolUse hook entry point.
//!
//! Per design §6.3 the hook can short-circuit the tool call by returning
//! `{"skip": true, "result": "<cached>"}` (e.g. when an identical Read /
//! Bash hits the tool-call cache, §21.6.2 row 1) or pass through with
//! enrichment metadata that the supervisor's brain layer adds.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code PreToolUse payload shape:
//! ```json
//! { "session_id": "...", "hook_event_name": "PreToolUse",
//!   "tool_name": "...", "tool_input": { ... } }
//! ```
//!
//! `tool_input` is an opaque object — we forward it to the supervisor
//! as a JSON string (the existing IPC contract expects `params: String`).
//!
//! The hook ALWAYS exits 0 on internal error. Claude Code treats
//! non-zero as BLOCK — blocking a tool call because mneme's supervisor
//! is down would be a regression of F-012.

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `mneme pre-tool`. All optional — STDIN fills in.
#[derive(Debug, Args)]
pub struct PreToolArgs {
    /// Tool name about to be invoked.
    #[arg(long)]
    pub tool: Option<String>,

    /// JSON-encoded tool params.
    #[arg(long)]
    pub params: Option<String>,

    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: PreToolArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "pre-tool STDIN parse failed; passing through");
            None
        }
    };

    let stdin_tool = stdin_payload.as_ref().and_then(|p| p.tool_name.clone());
    let stdin_params = stdin_payload
        .as_ref()
        .and_then(|p| p.tool_input.as_ref().map(|v| v.to_string()));
    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());

    let tool = choose(args.tool, stdin_tool, String::new());
    let params = choose(args.params, stdin_params, "{}".to_string());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());

    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::PreTool {
            tool,
            params,
            session_id,
        })
        .await;

    // The supervisor's response either carries `skip + result` (cache hit
    // or constraint violation) or `skip: false` (let the tool run). Any
    // error path emits `skip: false` so the tool always runs — we never
    // BLOCK on our own bug.
    let body = match response {
        Ok(IpcResponse::Ok { message }) => json!({ "skip": false, "note": message }),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "pre-tool supervisor error; passing through");
            json!({ "skip": false })
        }
        Ok(IpcResponse::Pong)
        | Ok(IpcResponse::Status { .. })
        | Ok(IpcResponse::Logs { .. })
        | Ok(IpcResponse::JobQueued { .. })
        | Ok(IpcResponse::JobQueue { .. }) => json!({ "skip": false }),
        Err(e) => {
            warn!(error = %e, "pre-tool supervisor unreachable; passing through");
            json!({ "skip": false })
        }
    };

    println!("{}", serde_json::to_string(&body)?);
    Ok(())
}
