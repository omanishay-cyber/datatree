//! `datatree rebuild` — drop everything and re-parse from scratch.
//!
//! Last-resort recovery; per design §13 / §5.7 (`rebuild(scope?)`).

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client, resolve_project};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree rebuild`.
#[derive(Debug, Args)]
pub struct RebuildArgs {
    /// Project root. Defaults to CWD.
    pub project: Option<PathBuf>,

    /// Don't ask for confirmation.
    #[arg(long)]
    pub yes: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: RebuildArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project)?;
    if !args.yes {
        eprintln!(
            "warning: rebuild will discard the cached graph for {} and re-parse from scratch.",
            project.display()
        );
        eprintln!("re-run with --yes to confirm.");
        return Ok(());
    }
    let client = make_client(socket_override);
    let resp = client.request(IpcRequest::Rebuild { project }).await?;
    handle_response(resp)
}
