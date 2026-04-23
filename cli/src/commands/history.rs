//! `datatree history <query> [--since=...]` — search the conversation log.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree history`.
#[derive(Debug, Args)]
pub struct HistoryArgs {
    /// Free-form query.
    pub query: String,

    /// ISO-8601 lower bound (e.g. `2026-04-01T00:00:00Z`).
    #[arg(long)]
    pub since: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: HistoryArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::History {
            query: args.query,
            since: args.since,
        })
        .await?;
    handle_response(resp)
}
