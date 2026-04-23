//! `mneme session-end` — SessionEnd hook entry point.
//!
//! Final flush + manifest update (per design §6.6). Best-effort; we never
//! want this hook to take down the host with a non-zero exit.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `mneme session-end`.
#[derive(Debug, Args)]
pub struct SessionEndArgs {
    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: SessionEndArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    if let Err(e) = client
        .request(IpcRequest::SessionEnd {
            session_id: args.session_id,
        })
        .await
    {
        warn!(error = %e, "session-end flush skipped (supervisor unreachable)");
    }
    Ok(())
}
