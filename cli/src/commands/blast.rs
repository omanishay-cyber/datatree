//! `mneme blast <file_or_function> [--depth=N]` — blast radius lookup.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `mneme blast`.
#[derive(Debug, Args)]
pub struct BlastArgs {
    /// File path or fully-qualified function name (e.g. `src/auth.ts:login`).
    pub target: String,

    /// Max traversal depth.
    #[arg(long, default_value_t = 2)]
    pub depth: usize,
}

/// Entry point used by `main.rs`.
pub async fn run(args: BlastArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Blast {
            target: args.target,
            depth: args.depth,
        })
        .await?;
    handle_response(resp)
}
