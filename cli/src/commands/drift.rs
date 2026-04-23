//! `datatree drift [--severity=...]` — current drift findings.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree drift`.
#[derive(Debug, Args)]
pub struct DriftArgs {
    /// Severity filter: `info` | `warn` | `error` | `critical`.
    #[arg(long)]
    pub severity: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: DriftArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Drift {
            severity: args.severity,
        })
        .await?;
    handle_response(resp)
}
