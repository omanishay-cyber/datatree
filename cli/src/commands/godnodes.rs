//! `mneme godnodes [--n=N]` — top-N most-connected concepts.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `mneme godnodes`.
#[derive(Debug, Args)]
pub struct GodNodesArgs {
    /// Optional project path. Defaults to CWD.
    pub project: Option<PathBuf>,

    /// How many to return.
    #[arg(long, default_value_t = 10)]
    pub n: usize,
}

/// Entry point used by `main.rs`.
pub async fn run(args: GodNodesArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::GodNodes {
            project: args.project,
            n: args.n,
        })
        .await?;
    handle_response(resp)
}
