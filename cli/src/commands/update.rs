//! `datatree update` — incremental update sweep against the active project.
//!
//! Triggers the supervisor to walk the file watcher's pending queue, parse
//! changed files, and re-run scanners over the affected blast radius.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client, resolve_project};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree update`.
#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Path to the project root. Defaults to CWD.
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: UpdateArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project)?;
    let client = make_client(socket_override);
    let resp = client.request(IpcRequest::Update { project }).await?;
    handle_response(resp)
}
