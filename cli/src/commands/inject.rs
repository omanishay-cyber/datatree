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
            prompt,
            session_id,
            cwd,
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
        | Ok(IpcResponse::Logs { .. })
        | Ok(IpcResponse::JobQueued { .. })
        | Ok(IpcResponse::JobQueue { .. }) => String::new(),
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
