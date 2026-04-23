//! `mneme status` — graph stats, drift findings count, last build time.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `mneme status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Optional project path. Defaults to CWD.
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: StatusArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Status {
            project: args.project,
        })
        .await?;
    handle_response(resp)
}
