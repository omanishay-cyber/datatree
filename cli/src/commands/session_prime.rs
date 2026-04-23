//! `datatree session-prime` — SessionStart hook entry point.
//!
//! Claude Code calls this when a new session starts. We respond with the
//! initial primer block (recent decisions, open todos, last drift findings)
//! and the resumption bundle if a Step Ledger task is mid-flight.

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `datatree session-prime`.
#[derive(Debug, Args)]
pub struct SessionPrimeArgs {
    /// Active project root.
    #[arg(long)]
    pub project: PathBuf,

    /// Session id assigned by the host.
    #[arg(long = "session-id")]
    pub session_id: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: SessionPrimeArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::SessionPrime {
            project: args.project,
            session_id: args.session_id,
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
        | Ok(IpcResponse::Logs { .. }) => String::new(),
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
