//! `mneme session-end` — SessionEnd hook entry point.
//!
//! Final flush + manifest update (per design §6.6). Best-effort; we never
//! want this hook to take down the host with a non-zero exit.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code SessionEnd delivers `{ session_id, hook_event_name,
//! transcript_path }` on STDIN. We just forward session_id to the
//! supervisor to trigger the flush.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
use crate::ipc::IpcRequest;

/// CLI args for `mneme session-end`. Session id optional — STDIN fills in.
#[derive(Debug, Args)]
pub struct SessionEndArgs {
    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,
}

/// Entry point used by `main.rs`. Always exits 0.
pub async fn run(args: SessionEndArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "session-end STDIN parse failed; falling back");
            None
        }
    };

    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());

    let client = make_client(socket_override);
    if let Err(e) = client
        .request(IpcRequest::SessionEnd { session_id })
        .await
    {
        warn!(error = %e, "session-end flush skipped (supervisor unreachable)");
    }
    Ok(())
}
