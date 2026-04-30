//! `mneme-supervisor` binary entry point.
//!
//! Subcommands:
//!   - `start`   — boot the supervisor in the foreground.
//!   - `service-run` — used by the Windows service control manager; do not
//!     invoke directly.
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
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::error;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

#[derive(Debug, Parser)]
#[command(name = "mneme-supervisor", version, about = "Mneme process supervisor", long_about = None)]
struct Cli {
    /// Path to the supervisor TOML config.
    #[arg(long, env = "MNEME_CONFIG")]
    config: Option<PathBuf>,

    /// Override the IPC socket / pipe path for client subcommands.
    #[arg(long, env = "MNEME_IPC")]
    ipc: Option<PathBuf>,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Start the supervisor in the foreground.
    Start {
        /// K10 chaos-test-only: panic the worker selected to handle the
        /// Nth dispatched job (1-indexed). Counts down from N; when it
        /// hits zero the supervisor invokes `panic!()` inside the
        /// dispatcher, which is captured by the per-child monitor as
        /// "exit code -1" and the restart loop respawns the worker.
        ///
        /// Only available when the binary is built with
        /// `--features test-hooks`. Production users see no `--inject-crash`
        /// in `--help`. Acts as a no-op (`0`) when omitted.
        #[cfg(feature = "test-hooks")]
        #[arg(long, default_value_t = 0)]
        inject_crash: u64,
    },
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
        #[arg(long, env = "MNEME_LIVEBUS")]
        livebus: Option<PathBuf>,
        /// Debounce window in milliseconds.
        #[arg(long, default_value_t = 250)]
        debounce_ms: u64,
    },
}

fn main() -> std::process::ExitCode {
    // The non-blocking file-appender guard MUST live for the entire run
    // of `main`; dropping it flushes & shuts down the writer thread, so
    // we bind it explicitly even though it looks unused. B-005 fix.
    let _file_log_guard = init_tracing();

    let cli = Cli::parse();
    // I-4 / I-5 / NEW-008: cap the tokio runtime. The supervisor is a
    // long-lived control-plane process — the worker_threads/blocking
    // pools should NOT scale with `num_cpus` on a 32-core box and stay
    // pinned even when nothing is running. min(4) covers typical IPC
    // burst (status/metrics/logs/dispatch) while keeping baseline RSS
    // predictable. max_blocking_threads(8) keeps stdin/stdout forwarder
    // tasks from accreting on a flapping worker pool.
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_cpus::get().clamp(1, 4))
        .max_blocking_threads(8)
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

/// Wire up tracing for the supervisor process.
///
/// Two layers compose into a single subscriber:
///
/// * **stdout layer** — JSON, `WARN+` only. Live operator visibility for
///   anyone running the supervisor in the foreground (`mneme daemon
///   start` falls through to this codepath after detaching). Workers
///   that emit color codes are stripped at the [`crate::log_ring`]
///   boundary, but the daemon's own log lines never colorise.
/// * **file layer** — JSON, `DEBUG+`. Rolling daily appender at
///   `<MNEME_HOME>/logs/supervisor.log`, rotated daily, keeping the most
///   recent 7 days. This is the canonical durable log surface that
///   `mneme daemon logs` tails when the in-memory ring is empty (B-005).
///
/// `MNEME_LOG` overrides both layers' filters at once (`info` keeps the
/// pre-fix behaviour). On a read-only filesystem the file layer is
/// disabled and a single warning is printed to stdout — the supervisor
/// still boots.
///
/// Returns the [`WorkerGuard`] for the non-blocking file writer; the
/// caller must keep it alive for the lifetime of the program. Drop ⇒
/// flush + writer-thread shutdown.
fn init_tracing() -> Option<WorkerGuard> {
    let stdout_filter = EnvFilter::try_from_env("MNEME_LOG")
        .unwrap_or_else(|_| EnvFilter::new("warn,mneme_supervisor=warn"));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(std::io::stdout)
        .with_filter(stdout_filter);

    // Build the file appender. If the dir can't be created (read-only
    // FS, permission error in a sandbox) we fall back to stdout-only and
    // surface a warning — supervisors without persistent logs still
    // boot.
    let file_layer_with_guard: Option<(_, WorkerGuard)> = match mneme_daemon::ensure_logs_dir() {
        Ok(dir) => {
            match RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix("supervisor.log")
                .max_log_files(7)
                .build(&dir)
            {
                Ok(appender) => {
                    let (nb, guard) = tracing_appender::non_blocking(appender);
                    let file_filter = EnvFilter::try_from_env("MNEME_LOG")
                        .unwrap_or_else(|_| EnvFilter::new("debug,mneme_supervisor=debug"));
                    let layer = tracing_subscriber::fmt::layer()
                        .json()
                        .with_current_span(false)
                        .with_span_list(false)
                        .with_ansi(false)
                        .with_writer(nb)
                        .with_filter(file_filter);
                    Some((layer, guard))
                }
                Err(e) => {
                    eprintln!(
                        "warning: could not build supervisor log appender ({e}); \
                             file logging disabled, stdout only"
                    );
                    None
                }
            }
        }
        Err(e) => {
            eprintln!(
                "warning: could not create supervisor logs dir ({e}); \
                     file logging disabled, stdout only"
            );
            None
        }
    };

    match file_layer_with_guard {
        Some((file_layer, guard)) => {
            let _ = tracing_subscriber::registry()
                .with(stdout_layer)
                .with(file_layer)
                .try_init();
            Some(guard)
        }
        None => {
            let _ = tracing_subscriber::registry().with(stdout_layer).try_init();
            None
        }
    }
}

async fn run_cli(cli: Cli) -> Result<(), SupervisorError> {
    let config_path = cli.config.clone().unwrap_or_else(default_config_path);

    match cli.command {
        #[cfg(feature = "test-hooks")]
        Cmd::Start { inject_crash } => {
            let config = SupervisorConfig::load(&config_path)?;
            // K10 chaos-test hook: store the configured countdown in a
            // process-wide atomic that `ChildManager::dispatch_to_pool`
            // reads on every dispatch. `0` (the default) disables the
            // hook entirely so the production-feature build is a no-op
            // even if the user somehow passes `--inject-crash 0`.
            if inject_crash > 0 {
                mneme_daemon::test_hooks::set_inject_crash(inject_crash);
                tracing::warn!(
                    n = inject_crash,
                    "K10 test hook armed: worker dispatch will panic on job N",
                );
            }
            service::execute(ServiceAction::RunForeground, config).await
        }
        #[cfg(not(feature = "test-hooks"))]
        Cmd::Start {} => {
            let config = SupervisorConfig::load(&config_path)?;
            service::execute(ServiceAction::RunForeground, config).await
        }
        Cmd::ServiceRun => {
            // Under SCM, this code path runs INSIDE the service worker
            // process. SCM gives us only ~30s after start before flagging
            // "service did not respond in time" (NEW-013). A bad config
            // file would otherwise propagate as an error here BEFORE we
            // ever hand control to the dispatcher, leaving SCM hung.
            // Fall back to the default layout so the dispatcher always
            // gets called; the dispatcher's own service_main then signals
            // RUNNING immediately and re-loads config with the same
            // fallback so the service still boots even on misconfigured
            // installs.
            let config = SupervisorConfig::load(&config_path).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "service-run: config load failed; using default_layout");
                SupervisorConfig::default_layout()
            });
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
            let root = project
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
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
    socket: &Path,
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

/// Resolve the IPC socket / pipe path for client subcommands.
///
/// Discovery order (first hit wins):
///   1. The `MNEME_IPC` env var or `--ipc` CLI flag (handled upstream).
///   2. `~/.mneme/supervisor.pipe` — written by the supervisor on boot in
///      [`crate::run`] (`lib.rs:56-62`). On Windows this resolves the
///      PID-scoped pipe name a running supervisor actually advertises;
///      without it, raw `mneme-daemon status` would always miss because
///      the unscoped legacy name is never bound (I-8).
///   3. Platform-specific fallback (legacy unscoped pipe on Windows, the
///      default `~/.mneme/supervisor.sock` on Unix). Kept so brand-new
///      installs without a discovery file still produce a coherent
///      "supervisor not running" error rather than panicking.
fn default_ipc_path() -> PathBuf {
    // K10 test hook: when `MNEME_TEST_SOCKET_NAME` is set, the test
    // suite is driving both the daemon and its client; use the same
    // override here so client subcommands (`status`, `logs`, etc.)
    // talk to the test-scoped pipe instead of the system-wide one.
    if let Ok(custom) = std::env::var("MNEME_TEST_SOCKET_NAME") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            #[cfg(windows)]
            {
                return PathBuf::from(format!(r"\\.\pipe\{}", trimmed));
            }
            #[cfg(unix)]
            {
                let mut base = home_dir();
                base.push(".mneme");
                base.push(trimmed);
                return base;
            }
        }
    }
    let mut disco = home_dir();
    disco.push(".mneme");
    disco.push("supervisor.pipe");
    if let Ok(contents) = std::fs::read_to_string(&disco) {
        let trimmed = contents.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    default_ipc_fallback()
}

#[cfg(windows)]
fn default_ipc_fallback() -> PathBuf {
    PathBuf::from(r"\\.\pipe\mneme-supervisor")
}

#[cfg(unix)]
fn default_ipc_fallback() -> PathBuf {
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
