//! Mneme Supervisor library.
//!
//! Re-exports every module so the binary (`main.rs`) and external integration
//! tests can use a stable surface. Nothing here performs side effects — see
//! [`run`] for the entry point that actually spawns workers.

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod child;
pub mod config;
pub mod error;
pub mod health;
pub mod ipc;
pub mod log_ring;
pub mod manager;
pub mod service;
pub mod watchdog;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use child::{ChildHandle, ChildSpec, ChildStatus, RestartStrategy};
pub use config::{RestartPolicy, SupervisorConfig};
pub use error::SupervisorError;
pub use health::{HealthServer, SlaSnapshot};
pub use ipc::{ControlCommand, ControlResponse, IpcServer};
pub use log_ring::{LogEntry, LogLevel, LogRing};
pub use manager::ChildManager;
pub use watchdog::Watchdog;
pub use watcher::{run_watcher, WatcherStats, WatcherStatsHandle, DEFAULT_DEBOUNCE};

use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{error, info};

/// Top-level supervisor result alias.
pub type Result<T> = std::result::Result<T, SupervisorError>;

/// Boot the supervisor. Spawns the [`ChildManager`], [`Watchdog`],
/// [`HealthServer`], and [`IpcServer`], then awaits a shutdown signal
/// (Ctrl+C, SIGTERM, or an IPC `Stop` command).
pub async fn run(config: SupervisorConfig) -> Result<()> {
    info!(
        version = env!("CARGO_PKG_VERSION"),
        children = config.children.len(),
        ipc = %config.ipc_socket_path.display(),
        "supervisor starting"
    );

    // Advertise the PID-scoped IPC pipe path so CLI clients can discover it
    // (Windows named pipes are PID-unique to avoid "Access denied" zombies).
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let disco = std::path::Path::new(&home).join(".mneme").join("supervisor.pipe");
        if let Some(parent) = disco.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&disco, config.ipc_socket_path.to_string_lossy().as_bytes());
    }

    let log_ring = Arc::new(LogRing::new(10_000));
    let manager = Arc::new(ChildManager::new(config.clone(), log_ring.clone()));
    let watchdog = Arc::new(Watchdog::new(manager.clone(), config.health_check_interval));
    let shutdown = Arc::new(Notify::new());

    // 0. Start the restart-request processor BEFORE any child is spawned
    //    so a child that crashes during spawn_all() is still eligible for
    //    auto-restart. The receiver is taken exactly once.
    let restart_handle = if let Some(rx) = manager.take_restart_rx().await {
        let mgr = manager.clone();
        Some(tokio::spawn(async move { mgr.run_restart_loop(rx).await }))
    } else {
        None
    };

    // 1. Spawn every configured child.
    manager.spawn_all().await?;

    // 2. Start the watchdog loop.
    let wd_handle = {
        let wd = watchdog.clone();
        let sd = shutdown.clone();
        tokio::spawn(async move { wd.run(sd).await })
    };

    // 3. Start the SLA dashboard HTTP server (localhost:7777/health).
    let health_server = HealthServer::new(manager.clone(), config.health_port);
    let health_handle = {
        let sd = shutdown.clone();
        tokio::spawn(async move { health_server.serve(sd).await })
    };

    // 4. Start the IPC control plane (Unix socket / Windows named pipe).
    let ipc = IpcServer::new(manager.clone(), config.ipc_socket_path.clone());
    let ipc_handle = {
        let sd = shutdown.clone();
        tokio::spawn(async move { ipc.serve(sd).await })
    };

    // 5. Wait for OS signal OR an IPC-triggered shutdown.
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(e) = result {
                error!(error = %e, "ctrl_c handler failed");
            }
            info!("ctrl-c received, initiating graceful shutdown");
        }
        _ = shutdown.notified() => {
            info!("shutdown notified by control plane");
        }
    }

    shutdown.notify_waiters();

    // 6. Stop children.
    manager.shutdown_all().await?;

    // 7. Join background tasks. Errors are logged, never panicked.
    if let Err(e) = wd_handle.await {
        error!(error = %e, "watchdog task join error");
    }
    if let Err(e) = health_handle.await {
        error!(error = %e, "health task join error");
    }
    if let Err(e) = ipc_handle.await {
        error!(error = %e, "ipc task join error");
    }
    if let Some(h) = restart_handle {
        h.abort();
        let _ = h.await;
    }

    info!("supervisor stopped cleanly");
    Ok(())
}
