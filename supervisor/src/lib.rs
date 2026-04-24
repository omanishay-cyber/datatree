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
pub mod job_queue;
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
pub use job_queue::{JobQueue, JobQueueSnapshot};
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

    // 0a. Attach the job queue BEFORE any child spawns, so that a
    // worker that dies during startup can still have its in-flight
    // (empty) queue snapshot recorded without panicking.
    let job_queue = Arc::new(JobQueue::new(16 * 1024));
    manager.attach_job_queue(job_queue.clone()).await;

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

    // 3a. Router task: drains the job queue and dispatches each job to
    // the matching worker pool. Runs in its own task so the IPC server
    // never blocks on stdin writes and so router panics cannot take
    // down the control plane.
    let router_handle = {
        let mgr = manager.clone();
        let queue = job_queue.clone();
        let sd = shutdown.clone();
        tokio::spawn(async move { run_router(mgr, queue, sd).await })
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
    router_handle.abort();
    let _ = router_handle.await;
    if let Some(h) = restart_handle {
        h.abort();
        let _ = h.await;
    }

    info!("supervisor stopped cleanly");
    Ok(())
}

/// Drain the shared [`JobQueue`] forever, dispatching each job to the
/// worker pool identified by its `pool_prefix()`. The router runs in
/// its own tokio task. Design notes:
///
/// * Pulls at most one pending job per iteration. Small by design —
///   `dispatch_to_pool` has to grab a write lock on the target worker's
///   stdin handle and we want to give other workers a fair chance.
/// * Uses the queue's `Notify` to wake on submits without busy-polling.
/// * On dispatch failure (e.g. no worker in the pool is running yet),
///   puts the job back on the front of the queue and sleeps 100 ms so
///   the supervisor can (re)spawn the missing pool.
/// * Honours the shared `shutdown` notify so that Ctrl-C leaves the
///   queue quiescent.
async fn run_router(
    manager: Arc<ChildManager>,
    queue: Arc<JobQueue>,
    shutdown: Arc<Notify>,
) {
    info!("router task online");
    let waker = queue.router_waker();
    loop {
        tokio::select! {
            _ = shutdown.notified() => {
                info!("router shutting down");
                break;
            }
            _ = waker.notified() => {}
            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                // Periodic wake-up covers the edge case where a pool
                // came online AFTER a job was queued and returned to
                // pending with no notification.
            }
        }

        // Drain everything we can in a burst. Keeps per-iteration
        // overhead low when the CLI submits a Build of 10k files.
        while let Some((id, job)) = queue.next_pending() {
            let prefix = job.pool_prefix();
            // Translate our Job enum into the worker-native wire format.
            // Each worker's stdin reader predates v0.3 and expects a
            // flat per-worker JSON shape — we keep it that way so
            // routing is additive (no worker behaviour change). If the
            // translation fails (e.g. a file we can't read), we fail
            // the job rather than crash the router.
            let line = match encode_for_worker(id, &job) {
                Ok(s) => s,
                Err(e) => {
                    error!(%id, kind = job.kind_label(), error = %e, "router: encode failed");
                    queue.complete(
                        id,
                        common::jobs::JobOutcome::Err {
                            message: format!("router encode: {e}"),
                        },
                    );
                    continue;
                }
            };
            match manager.dispatch_to_pool(prefix, &line).await {
                Ok(worker) => {
                    queue.mark_assigned(id, worker.clone());
                    tracing::debug!(
                        %id,
                        kind = job.kind_label(),
                        worker = %worker,
                        "router: dispatched job"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        %id,
                        kind = job.kind_label(),
                        prefix,
                        error = %e,
                        "router: no worker available; re-queuing"
                    );
                    queue.return_pending(id);
                    // Pool isn't ready yet (worker not spawned, or all
                    // stdins closed). Back off briefly so we don't spin.
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    break;
                }
            }
        }
    }
    info!("router task offline");
}

/// Translate a v0.3 [`Job`] into the worker-native JSON line shape.
///
/// Each worker predates the `Job` enum; they deserialize their own
/// historical wire formats. Rather than break those, the router emits
/// per-worker JSON so the handoff is fully additive. On the worker side
/// the only thing we need to add is an "emit result back to supervisor"
/// path — tracked as a v0.4 follow-up (for now the parse-worker still
/// writes to stdout, which the supervisor's `monitor_child` captures as
/// log lines).
fn encode_for_worker(
    id: common::jobs::JobId,
    job: &common::jobs::Job,
) -> std::result::Result<String, String> {
    use common::jobs::Job;
    match job {
        Job::Parse { file_path, .. } => {
            // parse-worker's JobWire expects {file_path, language, content,
            // prev_tree_id?, job_id}. Read the file synchronously here —
            // the router task is dedicated and a blocking read is cheap
            // compared to the downstream tree-sitter parse.
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| format!("read {}: {e}", file_path.display()))?;
            let language = infer_language_tag(file_path)
                .ok_or_else(|| format!("no language for {}", file_path.display()))?;
            Ok(serde_json::json!({
                "job_id": id.0,
                "file_path": file_path,
                "language": language,
                "content": content,
            })
            .to_string())
        }
        Job::Scan {
            file_path,
            ast_id,
            ..
        } => {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| format!("read {}: {e}", file_path.display()))?;
            Ok(serde_json::json!({
                "job_id": id.0,
                "file_path": file_path,
                "content": content,
                "ast_id": ast_id,
                "scanner_filter": [],
            })
            .to_string())
        }
        Job::Embed {
            node_qualified,
            text,
            ..
        } => Ok(serde_json::json!({
            "job_id": id.0,
            "node_qualified": node_qualified,
            "text": text,
        })
        .to_string()),
        Job::Ingest { md_file, .. } => Ok(serde_json::json!({
            "job_id": id.0,
            "md_file": md_file,
        })
        .to_string()),
    }
}

/// Best-effort language tag for the parse-worker's JobWire.
///
/// Kept as a tiny hardcoded map so the supervisor doesn't depend on the
/// parsers crate (which pulls in tree-sitter). Extensions we don't know
/// map to `None` and the job fails at encode time; the CLI's walker
/// already filters most of them out before submit.
fn infer_language_tag(path: &std::path::Path) -> Option<&'static str> {
    match path.extension().and_then(|s| s.to_str())?.to_ascii_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "hpp" | "hh" | "cxx" => Some("cpp"),
        "md" | "markdown" => Some("markdown"),
        "json" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        _ => None,
    }
}
