//! `datatree post-tool` — PostToolUse hook entry point.
//!
//! Fire-and-forget capture (per design §6.4). We send the request to the
//! supervisor but don't wait for its response — the host doesn't read
//! anything from this hook's stdout.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree post-tool`.
#[derive(Debug, Args)]
pub struct PostToolArgs {
    /// Tool name that ran.
    #[arg(long)]
    pub tool: String,

    /// Path to the file holding the tool's serialized result.
    #[arg(long = "result-file")]
    pub result_file: PathBuf,

    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: PostToolArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    if let Err(e) = client
        .request(IpcRequest::PostTool {
            tool: args.tool,
            result_file: args.result_file,
            session_id: args.session_id,
        })
        .await
    {
        // Non-blocking: if the supervisor is down, we shouldn't fail the
        // hook. Just log and exit 0 so the host moves on.
        warn!(error = %e, "post-tool capture skipped (supervisor unreachable)");
    }
    Ok(())
}
