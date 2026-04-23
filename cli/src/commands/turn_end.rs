//! `mneme turn-end` — Stop hook entry point.
//!
//! Triggers the summarizer and updates the Step Ledger drift score.
//! The optional flags let the same handler be reused for PreCompact (so
//! we can flush the ledger to disk before compaction) and SubagentStop
//! (so per-subagent turns are accounted separately).

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `mneme turn-end`.
#[derive(Debug, Args)]
pub struct TurnEndArgs {
    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: String,

    /// This hook fired right before a compaction event.
    #[arg(long = "pre-compact")]
    pub pre_compact: bool,

    /// This hook fired at the end of a subagent turn rather than the main
    /// model.
    #[arg(long)]
    pub subagent: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: TurnEndArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let suffix = match (args.pre_compact, args.subagent) {
        (true, _) => ":pre-compact",
        (_, true) => ":subagent",
        _ => "",
    };
    let session_id = format!("{}{}", args.session_id, suffix);
    if let Err(e) = client
        .request(IpcRequest::TurnEnd { session_id })
        .await
    {
        warn!(error = %e, "turn-end notification skipped (supervisor unreachable)");
    }
    Ok(())
}
