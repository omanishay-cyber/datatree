//! `mneme recall <query>` — semantic search across decisions, conversation,
//! concepts, files, todos, and constraints.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `mneme recall`.
#[derive(Debug, Args)]
pub struct RecallArgs {
    /// Free-form query string. Required.
    pub query: String,

    /// Restrict to one source: decision | conversation | concept | file | todo | constraint.
    #[arg(long = "type")]
    pub kind: Option<String>,

    /// Max results to return.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

/// Entry point used by `main.rs`.
pub async fn run(args: RecallArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Recall {
            query: args.query,
            kind: args.kind,
            limit: args.limit,
        })
        .await?;
    handle_response(resp)
}
