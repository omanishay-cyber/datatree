//! `datatree inject` — UserPromptSubmit hook entry point.
//!
//! Claude Code calls this after the user submits a prompt. We forward the
//! prompt to the supervisor, which composes a "smart inject bundle"
//! (§4.2): recent decisions, active constraints, blast-radius previews,
//! drift redirect, and the current step from the ledger.
//!
//! Output format is the JSON shape Claude Code expects from a
//! UserPromptSubmit hook:
//!
//! ```json
//! { "hookEventName": "UserPromptSubmit",
//!   "additional_context": "<datatree-context>...</datatree-context>" }
//! ```

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `datatree inject`.
#[derive(Debug, Args)]
pub struct InjectArgs {
    /// The user prompt as captured by the hook.
    #[arg(long)]
    pub prompt: String,

    /// Session id assigned by the host.
    #[arg(long = "session-id")]
    pub session_id: String,

    /// Working directory at the time the hook fired.
    #[arg(long)]
    pub cwd: PathBuf,
}

/// Entry point used by `main.rs`.
pub async fn run(args: InjectArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::Inject {
            prompt: args.prompt,
            session_id: args.session_id,
            cwd: args.cwd,
        })
        .await;

    let payload = match response {
        Ok(IpcResponse::Ok { message }) => message.unwrap_or_default(),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "supervisor returned error; emitting empty additional_context");
            String::new()
        }
        Ok(IpcResponse::Pong)
        | Ok(IpcResponse::Status { .. })
        | Ok(IpcResponse::Logs { .. }) => String::new(),
        Err(e) => {
            warn!(error = %e, "supervisor unreachable; emitting empty additional_context");
            String::new()
        }
    };

    let out = json!({
        "hookEventName": "UserPromptSubmit",
        "additional_context": payload,
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
