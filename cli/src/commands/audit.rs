//! `datatree audit [--scope=...]` — run all configured scanners.
//!
//! Returns the JSON findings list straight from the scanners worker.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree audit`.
#[derive(Debug, Args)]
pub struct AuditArgs {
    /// Scope filter: `theme`, `security`, `a11y`, `perf`, `types`, or `all`.
    #[arg(long, default_value = "all")]
    pub scope: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: AuditArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Audit { scope: args.scope })
        .await?;
    handle_response(resp)
}
