//! `datatree step <op> [arg]` — Step Ledger ops.
//!
//! Per design §5.5 / §7.2:
//!
//! - `status`           — current step + ledger snapshot
//! - `show <step_id>`   — detail of one step
//! - `verify <step_id>` — run acceptance check
//! - `complete <step_id>` — mark complete (only if verify passes)
//! - `resume`           — emit resumption bundle
//! - `plan-from <md>`   — ingest a markdown roadmap into the ledger

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::{CliError, CliResult};
use crate::ipc::IpcRequest;

/// CLI args for `datatree step`.
#[derive(Debug, Args)]
pub struct StepArgs {
    /// Operation: `status`, `show`, `verify`, `complete`, `resume`, `plan-from`.
    pub op: String,

    /// Optional argument:
    /// - `status` / `resume`     → ignored
    /// - `show` / `verify` / `complete` → step id (e.g. `"3.2.1"`)
    /// - `plan-from`             → path to a markdown roadmap
    pub arg: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: StepArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // Surface obvious mistakes early.
    let needs_arg = matches!(
        args.op.as_str(),
        "show" | "verify" | "complete" | "plan-from"
    );
    if needs_arg && args.arg.is_none() {
        return Err(CliError::Other(format!(
            "step {} requires an argument",
            args.op
        )));
    }

    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Step {
            op: args.op,
            arg: args.arg,
        })
        .await?;
    handle_response(resp)
}
