//! `mneme pre-tool` — PreToolUse hook entry point.
//!
//! Per design §6.3 the hook can short-circuit the tool call by returning
//! `{"skip": true, "result": "<cached>"}` (e.g. when an identical Read /
//! Bash hits the tool-call cache, §21.6.2 row 1) or pass through with
//! enrichment metadata that the supervisor's brain layer adds.

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `mneme pre-tool`.
#[derive(Debug, Args)]
pub struct PreToolArgs {
    /// Tool name about to be invoked.
    #[arg(long)]
    pub tool: String,

    /// JSON-encoded tool params.
    #[arg(long)]
    pub params: String,

    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: PreToolArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::PreTool {
            tool: args.tool,
            params: args.params,
            session_id: args.session_id,
        })
        .await;

    // The supervisor's response either carries `skip + result` (cache hit
    // or constraint violation) or `skip: false` (let the tool run).
    let body = match response {
        Ok(IpcResponse::Ok { message }) => json!({ "skip": false, "note": message }),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "pre-tool supervisor error; passing through");
            json!({ "skip": false })
        }
        Ok(IpcResponse::Pong)
        | Ok(IpcResponse::Status { .. })
        | Ok(IpcResponse::Logs { .. }) => json!({ "skip": false }),
        Err(e) => {
            warn!(error = %e, "pre-tool supervisor unreachable; passing through");
            json!({ "skip": false })
        }
    };

    println!("{}", serde_json::to_string(&body)?);
    Ok(())
}
