//! `datatree doctor` — health check / self-test.
//!
//! Performs both an in-process check (binary version, runtime/state dirs
//! writeable, IPC socket reachable) and an IPC `Doctor` request to the
//! supervisor for the live SLA snapshot.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::{handle_response, make_client};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Skip the live IPC probe (in-process diagnostics only).
    #[arg(long)]
    pub offline: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: DoctorArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    println!("datatree v{}", env!("CARGO_PKG_VERSION"));

    let runtime = crate::runtime_dir();
    let state = crate::state_dir();
    println!("runtime dir: {}", runtime.display());
    println!("state   dir: {}", state.display());
    print_writeable("runtime", &runtime);
    print_writeable("state", &state);

    if args.offline {
        return Ok(());
    }

    let client = make_client(socket_override);
    if !client.is_running().await {
        warn!("supervisor is not reachable at the IPC socket");
        println!("supervisor: NOT REACHABLE");
        return Ok(());
    }

    // Supervisor has no explicit Doctor command — we compose our own
    // health summary from its Status reply.
    let resp = client.request(IpcRequest::Status { project: None }).await?;
    println!("supervisor:");
    handle_response(resp)
}

fn print_writeable(label: &str, path: &std::path::Path) {
    let writeable = std::fs::create_dir_all(path)
        .and_then(|_| {
            let probe = path.join(".datatree-probe");
            std::fs::write(&probe, b"")?;
            std::fs::remove_file(&probe)
        })
        .is_ok();
    println!(
        "{label:>9} writeable: {}",
        if writeable { "yes" } else { "NO" }
    );
}
