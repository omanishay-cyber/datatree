//! `mneme-supervisor` binary entry point.
//!
//! Subcommands:
//!   - `start`   — boot the supervisor in the foreground.
//!   - `service-run` — used by the Windows service control manager; do not
//!                     invoke directly.
//!   - `install` / `uninstall` — manage the Windows service registration.
//!   - `stop`    — send a `Stop` over IPC.
//!   - `restart` — send a `RestartAll` (or `Restart {child}`) over IPC.
//!   - `status`  — print the current child snapshot.
//!   - `logs`    — tail recent log entries.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use mneme_daemon::config::SupervisorConfig;
use mneme_daemon::error::SupervisorError;
use mneme_daemon::ipc::{self, ControlCommand, ControlResponse};
use mneme_daemon::service::{self, ServiceAction};
use mneme_daemon::watcher::{self, WatcherStatsHandle, DEFAULT_DEBOUNCE};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::error;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "mneme-supervisor", version, about = "Mneme process supervisor", long_about = None)]
struct Cli {
    /// Path to the supervisor TOML config.
    #[arg(long, env = "MNEME_CONFIG")]
    config: Option<PathBuf>,

    /// Override the IPC socket / pipe path for client subcommands.
    #[arg(long, env = "DATATREE_IPC")]
    ipc: Option<PathBuf>,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Start the supervisor in the foreground.
    Start,
    /// Hand off to the Windows service control manager.
    ServiceRun,
    /// Install as a Windows service (no-op on Unix).
    Install,
    /// Uninstall the Windows service (no-op on Unix).
    Uninstall,
    /// Send a graceful Stop over IPC.
    Stop,
    /// Restart all children (or a single named child).
    Restart {
        /// Optional child name (omit to restart all).
        #[arg(long)]
        child: Option<String>,
    },
    /// Print supervisor + child status as JSON.
    Status,
    /// Tail recent log entries.
    Logs {
        /// Limit to a single child.
        #[arg(long)]
        child: Option<String>,
        /// How many entries to print.
        #[arg(long, default_value_t = 100)]
        n: usize,
    },
    /// Watch a project directory and incrementally re-index on save.
    /// Blocks until Ctrl-C. Writes `file_reindexed` events to livebus if
    /// the socket path is reachable.
    Watch {
        /// Project root to watch (defaults to CWD).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Optional livebus IPC socket path to emit events on.
        #[arg(long, env = "DATATREE_LIVEBUS")]
        livebus: Option<PathBuf>,
        /// Debounce window in milliseconds.
        #[arg(long, default_value_t = 250)]
        debounce_ms: u64,
    },
}

fn main() -> std::process::ExitCode {
    init_tracing();

    let cli = Cli::parse();
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "failed to build tokio runtime");
            return std::process::ExitCode::FAILURE;
        }
    };

    let result = rt.block_on(run_cli(cli));
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            error!(error = %e, "command failed");
            std::process::ExitCode::FAILURE
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env("MNEME_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,datatree_supervisor=info"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

async fn run_cli(cli: Cli) -> Result<(), SupervisorError> {
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(default_config_path);

    match cli.command {
        Cmd::Start => {
            let config = SupervisorConfig::load(&config_path)?;
            service::execute(ServiceAction::RunForeground, config).await
        }
        Cmd::ServiceRun => {
            let config = SupervisorConfig::load(&config_path)?;
            service::execute(ServiceAction::RunAsService, config).await
        }
        Cmd::Install => {
            let config = SupervisorConfig::load(&config_path)?;
            service::execute(ServiceAction::Install, config).await
        }
        Cmd::Uninstall => {
            let config = SupervisorConfig::load(&config_path)?;
            service::execute(ServiceAction::Uninstall, config).await
        }
        Cmd::Stop => {
            let socket = cli.ipc.unwrap_or_else(default_ipc_path);
            let resp = round_trip(&socket, &ControlCommand::Stop).await?;
            print_response(&resp);
            Ok(())
        }
        Cmd::Restart { child } => {
            let socket = cli.ipc.unwrap_or_else(default_ipc_path);
            let cmd = match child {
                Some(c) => ControlCommand::Restart { child: c },
                None => ControlCommand::RestartAll,
            };
            let resp = round_trip(&socket, &cmd).await?;
            print_response(&resp);
            Ok(())
        }
        Cmd::Status => {
            let socket = cli.ipc.unwrap_or_else(default_ipc_path);
            let resp = round_trip(&socket, &ControlCommand::Status).await?;
            print_response(&resp);
            Ok(())
        }
        Cmd::Logs { child, n } => {
            let socket = cli.ipc.unwrap_or_else(default_ipc_path);
            let resp = round_trip(&socket, &ControlCommand::Logs { child, n }).await?;
            print_response(&resp);
            Ok(())
        }
        Cmd::Watch {
            project,
            livebus,
            debounce_ms,
        } => {
            let root = project.unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
            let stats = WatcherStatsHandle::new();
            let debounce = if debounce_ms == 0 {
                DEFAULT_DEBOUNCE
            } else {
                std::time::Duration::from_millis(debounce_ms)
            };
            tracing::info!(
                root = %root.display(),
                debounce_ms = debounce.as_millis() as u64,
                "starting watcher"
            );
            let watch_fut = watcher::run_watcher(root, livebus, stats, debounce);
            tokio::select! {
                result = watch_fut => {
                    if let Err(e) = result {
                        return Err(SupervisorError::Other(format!("watcher exited: {e}")));
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("ctrl-c received, watcher shutting down");
                }
            }
            Ok(())
        }
    }
}

fn print_response(resp: &ControlResponse) {
    match serde_json::to_string_pretty(resp) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("failed to render response: {e}"),
    }
}

async fn round_trip(
    socket: &PathBuf,
    cmd: &ControlCommand,
) -> Result<ControlResponse, SupervisorError> {
    let mut stream = ipc::connect_client(socket).await?;

    let body = serde_json::to_vec(cmd)?;
    let len = (body.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    let mut resp_body = vec![0u8; resp_len];
    stream.read_exact(&mut resp_body).await?;

    let resp: ControlResponse = serde_json::from_slice(&resp_body)?;
    Ok(resp)
}

fn default_config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("MNEME_CONFIG") {
        return PathBuf::from(p);
    }
    let mut base = home_dir();
    base.push(".mneme");
    base.push("supervisor.toml");
    base
}

#[cfg(windows)]
fn default_ipc_path() -> PathBuf {
    PathBuf::from(r"\\.\pipe\mneme-supervisor")
}

#[cfg(unix)]
fn default_ipc_path() -> PathBuf {
    let mut base = home_dir();
    base.push(".mneme");
    base.push("supervisor.sock");
    base
}

fn home_dir() -> PathBuf {
    if let Some(h) = std::env::var_os("MNEME_HOME") {
        return PathBuf::from(h);
    }
    #[cfg(windows)]
    {
        if let Some(h) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(h);
        }
    }
    #[cfg(unix)]
    {
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h);
        }
    }
    PathBuf::from(".")
}
