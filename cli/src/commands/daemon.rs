//! `datatree daemon <op>` — start/stop/restart/status/logs subcommands for
//! the supervisor process.
//!
//! `start` and `restart` shell out to the supervisor binary (which lives
//! in the same directory as `datatree`, or on PATH as
//! `datatree-supervisor`). `stop`, `status`, and `logs` go over the IPC
//! socket so we never have to know the supervisor's PID.

use clap::Args;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tracing::{info, warn};

use crate::commands::build::{handle_response, make_client};
use crate::error::{CliError, CliResult};
use crate::ipc::IpcRequest;

/// CLI args for `datatree daemon`.
#[derive(Debug, Args)]
pub struct DaemonArgs {
    /// Sub-op: `start` | `stop` | `restart` | `status` | `logs`.
    pub op: String,

    /// Override path to the supervisor binary (used by `start` / `restart`).
    #[arg(long, env = "DATATREE_SUPERVISOR_BIN")]
    pub bin: Option<PathBuf>,

    /// For `logs`: how many tail lines to fetch.
    #[arg(long, default_value_t = 200)]
    pub lines: usize,
}

/// Entry point used by `main.rs`.
pub async fn run(args: DaemonArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    match args.op.as_str() {
        "start" => start_daemon(args.bin),
        "stop" => stop_daemon(socket_override).await,
        "restart" => {
            // Best-effort stop, then start. Order matters; we don't want
            // two supervisors fighting over the IPC socket.
            let _ = stop_daemon(socket_override.clone()).await;
            tokio::time::sleep(Duration::from_millis(200)).await;
            start_daemon(args.bin)
        }
        "status" => status_daemon(socket_override).await,
        "logs" => logs_daemon(socket_override, args.lines).await,
        other => Err(CliError::Other(format!("unknown daemon op: {other}"))),
    }
}

fn start_daemon(bin: Option<PathBuf>) -> CliResult<()> {
    let path = bin.unwrap_or_else(default_supervisor_binary);
    info!(path = %path.display(), "spawning supervisor");
    if !path.exists() {
        return Err(CliError::Other(format!(
            "supervisor binary not found at {}; install datatree first or pass --bin",
            path.display()
        )));
    }
    Command::new(&path)
        .spawn()
        .map_err(|e| CliError::io(&path, e))?;
    println!("supervisor started ({})", path.display());
    Ok(())
}

async fn stop_daemon(socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    match client.request(IpcRequest::Stop).await {
        Ok(_) => {
            println!("supervisor stop requested");
            Ok(())
        }
        Err(e) => {
            // If the socket's gone we treat that as "already stopped".
            warn!(error = %e, "stop request failed; supervisor may already be down");
            println!("supervisor not reachable (probably already stopped)");
            Ok(())
        }
    }
}

async fn status_daemon(socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    if !client.is_running().await {
        println!("supervisor: NOT RUNNING");
        return Ok(());
    }
    let resp = client.request(IpcRequest::Status { project: None }).await?;
    handle_response(resp)
}

async fn logs_daemon(socket_override: Option<PathBuf>, lines: usize) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = client
        .request(IpcRequest::Logs {
            child: None,
            n: lines,
        })
        .await?;
    handle_response(resp)
}

fn default_supervisor_binary() -> PathBuf {
    // Look for `datatree-supervisor` next to the current binary first.
    if let Ok(this) = std::env::current_exe() {
        if let Some(parent) = this.parent() {
            let mut candidate = parent.join("datatree-supervisor");
            if cfg!(windows) {
                candidate.set_extension("exe");
            }
            if candidate.exists() {
                return candidate;
            }
        }
    }
    let mut p = PathBuf::from("datatree-supervisor");
    if cfg!(windows) {
        p.set_extension("exe");
    }
    p
}
