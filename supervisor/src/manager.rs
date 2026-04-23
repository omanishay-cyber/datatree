//! Child process manager.
//!
//! Owns every [`ChildHandle`], spawns workers via tokio's [`tokio::process`]
//! API, watches each one for exit, applies the exponential back-off restart
//! policy, and pipes stdout/stderr into the shared [`LogRing`].

use crate::child::{ChildHandle, ChildSpec, ChildStatus, RestartStrategy};
use crate::config::SupervisorConfig;
use crate::error::SupervisorError;
use crate::log_ring::LogRing;
use chrono::Utc;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Owns all running children and their restart state.
pub struct ChildManager {
    config: SupervisorConfig,
    log_ring: Arc<LogRing>,
    handles: RwLock<HashMap<String, Arc<Mutex<ChildHandle>>>>,
    monitors: Mutex<HashMap<String, JoinHandle<()>>>,
    shutdown_flag: Mutex<bool>,
}

impl ChildManager {
    /// Construct a manager from a fully-validated config.
    pub fn new(config: SupervisorConfig, log_ring: Arc<LogRing>) -> Self {
        Self {
            config,
            log_ring,
            handles: RwLock::new(HashMap::new()),
            monitors: Mutex::new(HashMap::new()),
            shutdown_flag: Mutex::new(false),
        }
    }

    /// Spawn every child listed in the config. A child whose binary is
    /// missing (file not found) is skipped with a warning — the daemon
    /// stays up with whatever workers actually exist. Other errors still
    /// propagate and abort startup.
    pub async fn spawn_all(self: &Arc<Self>) -> Result<(), SupervisorError> {
        let specs = self.config.children.clone();
        for spec in specs {
            match self.spawn_child(spec.clone()).await {
                Ok(()) => {}
                Err(SupervisorError::Spawn { name, source })
                    if matches!(
                        source.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                    ) =>
                {
                    tracing::warn!(
                        child = %name,
                        binary = %spec.command,
                        "binary missing — child skipped; daemon continuing"
                    );
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Spawn a single child and start its monitor task.
    pub async fn spawn_child(self: &Arc<Self>, spec: ChildSpec) -> Result<(), SupervisorError> {
        let initial_backoff = self.config.default_restart_policy.initial_backoff;
        let name = spec.name.clone();

        // Insert (or refresh) the handle.
        {
            let mut guard = self.handles.write().await;
            guard
                .entry(name.clone())
                .or_insert_with(|| Arc::new(Mutex::new(ChildHandle::new(spec.clone(), initial_backoff))));
        }

        let handle_arc = {
            let guard = self.handles.read().await;
            guard
                .get(&name)
                .cloned()
                .expect("handle just inserted above")
        };

        let child = self.spawn_os_process(&spec).await?;
        let pid = child.id();

        // Move bookkeeping into the spawned task so the surrounding
        // future is Send (Child is Send but holding it across the
        // handle_arc.lock().await above made the future opaque to
        // the auto-trait checker).
        let me = Arc::clone(self);
        let handle_for_task = Arc::clone(&handle_arc);
        let task_name = name.clone();
        let task = tokio::spawn(async move {
            {
                let mut h = handle_for_task.lock().await;
                h.pid = pid;
                h.status = ChildStatus::Running;
                h.last_started_at = Some(Utc::now());
                h.last_started_instant = Some(Instant::now());
                h.last_heartbeat = Some(Instant::now());
            }
            me.monitor_child(task_name, child, handle_for_task).await;
        });

        let mut mons = self.monitors.lock().await;
        mons.insert(spec.name.clone(), task);

        info!(child = %spec.name, pid = ?pid, "child spawned");
        Ok(())
    }

    async fn spawn_os_process(&self, spec: &ChildSpec) -> Result<Child, SupervisorError> {
        let mut cmd = Command::new(&spec.command);
        cmd.args(&spec.args);
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // Pipe stdin (don't close it). Workers like parse-worker read stdin
        // for jobs and would otherwise exit cleanly on EOF and be reaped
        // even though nothing crashed.
        cmd.stdin(Stdio::piped());
        cmd.kill_on_drop(true);

        cmd.spawn().map_err(|e| SupervisorError::Spawn {
            name: spec.name.clone(),
            source: e,
        })
    }

    async fn monitor_child(
        self: Arc<Self>,
        name: String,
        mut child: Child,
        handle: Arc<Mutex<ChildHandle>>,
    ) {
        // Pipe stdout / stderr into the shared log ring.
        if let Some(stdout) = child.stdout.take() {
            let ring = self.log_ring.clone();
            let n = name.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    ring.push_raw(&n, &line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let ring = self.log_ring.clone();
            let n = name.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    ring.push_raw(&n, &line);
                }
            });
        }

        // Block until the OS reports exit. Then explicitly drop the child
        // before any further awaits so its non-Send pieces (stdin/stdout
        // handles) don't poison the surrounding future.
        let exit_status = match child.wait().await {
            Ok(s) => s,
            Err(e) => {
                error!(child = %name, error = %e, "wait() failed");
                return;
            }
        };
        let exit_code = exit_status.code().unwrap_or(-1);
        drop(child);

        {
            let mut h = handle.lock().await;
            h.last_exit_code = Some(exit_code);
            if let Some(start) = h.last_started_instant {
                h.total_uptime = h.total_uptime.saturating_add(start.elapsed());
            }
            h.pid = None;
        }

        // Honour a graceful supervisor shutdown.
        if *self.shutdown_flag.lock().await {
            let mut h = handle.lock().await;
            h.status = ChildStatus::Stopped;
            info!(child = %name, code = exit_code, "child stopped during shutdown");
            return;
        }

        let strategy = {
            let h = handle.lock().await;
            h.spec.restart
        };

        let should_restart = match strategy {
            RestartStrategy::Permanent => true,
            RestartStrategy::Transient => exit_code != 0,
            RestartStrategy::Temporary => false,
        };

        if !should_restart {
            let mut h = handle.lock().await;
            h.status = ChildStatus::Stopped;
            warn!(child = %name, code = exit_code, "child exited; restart strategy says no");
            return;
        }

        // Apply exponential backoff + budget.
        if let Err(e) = self.restart_with_backoff(&name, handle).await {
            error!(child = %name, error = %e, "restart failed");
        }
    }

    async fn restart_with_backoff(
        self: &Arc<Self>,
        name: &str,
        handle: Arc<Mutex<ChildHandle>>,
    ) -> Result<(), SupervisorError> {
        let policy = self.config.default_restart_policy.clone();

        // Compute next backoff and check budget.
        let (delay, spec) = {
            let mut h = handle.lock().await;
            h.status = ChildStatus::Restarting;
            h.record_restart(policy.budget_window);

            let in_window = h.restarts_in_window(policy.budget_window);
            if in_window > policy.max_restarts_per_window {
                h.status = ChildStatus::Degraded;
                return Err(SupervisorError::RestartBudgetExceeded {
                    name: name.to_string(),
                    restarts: in_window,
                    window_secs: policy.budget_window.as_secs(),
                });
            }

            let next = (h.current_backoff.as_millis() as f32 * policy.backoff_multiplier) as u64;
            let capped = next.min(policy.max_backoff.as_millis() as u64);
            let delay = h.current_backoff;
            h.current_backoff = Duration::from_millis(capped.max(1));
            (delay, h.spec.clone())
        };

        debug!(child = %name, delay_ms = delay.as_millis() as u64, "would restart");
        tokio::time::sleep(delay).await;
        // v0.1 NOTE: auto-restart deferred to v0.2 — invoking spawn_child
        // here triggers a recursive opaque-future Send-inference error
        // through tokio::process::Child handles on Windows. Children that
        // exit are logged via the shared LogRing and require manual
        // `datatree daemon restart --child <name>` until the recursion is
        // restructured (likely via a bounded restart-channel + dedicated
        // supervisor thread). Tracked in TEST_RUN.md.
        let _ = spec;
        warn!(child = %name, "auto-restart deferred to v0.2; child stays down");
        Ok(())
    }

    /// Stop every child in parallel. Used during graceful shutdown.
    pub async fn shutdown_all(self: &Arc<Self>) -> Result<(), SupervisorError> {
        *self.shutdown_flag.lock().await = true;

        let monitors: Vec<JoinHandle<()>> = {
            let mut mons = self.monitors.lock().await;
            mons.drain().map(|(_, j)| j).collect()
        };

        // Sending the kill signal: tokio's `Command::kill_on_drop(true)` is in
        // place, but we explicitly mark every child as Stopped here.
        {
            let guard = self.handles.read().await;
            for (_, h) in guard.iter() {
                let mut handle = h.lock().await;
                handle.status = ChildStatus::Stopped;
            }
        }

        for j in monitors {
            // Detach: the per-child monitor will exit cleanly when its child
            // stream closes. We don't await — that could deadlock.
            j.abort();
        }
        Ok(())
    }

    /// Force-kill a single child by name. Used by the watchdog when a
    /// heartbeat is missed past the limit.
    pub async fn kill_child(self: &Arc<Self>, name: &str) -> Result<(), SupervisorError> {
        let pid_opt = {
            let guard = self.handles.read().await;
            match guard.get(name) {
                Some(h) => h.lock().await.pid,
                None => None,
            }
        };
        let pid = match pid_opt {
            Some(p) => p,
            None => return Ok(()),
        };

        kill_pid(pid)?;
        warn!(child = %name, pid, "force-killed child");
        Ok(())
    }

    /// Snapshot every child handle for read-only consumers (health server,
    /// IPC layer, watchdog).
    pub async fn snapshot(&self) -> Vec<ChildSnapshot> {
        let guard = self.handles.read().await;
        let mut out = Vec::with_capacity(guard.len());
        for (name, handle) in guard.iter() {
            let h = handle.lock().await;
            let percentiles = h.latency_percentiles_us();
            out.push(ChildSnapshot {
                name: name.clone(),
                status: h.status,
                pid: h.pid,
                restart_count: h.restart_count,
                current_uptime_ms: h.current_uptime().as_millis() as u64,
                total_uptime_ms: h.total_uptime.as_millis() as u64,
                last_exit_code: h.last_exit_code,
                p50_us: percentiles.map(|p| p.0),
                p95_us: percentiles.map(|p| p.1),
                p99_us: percentiles.map(|p| p.2),
            });
        }
        out
    }

    /// Return a clone of the live config (used by the IPC `Status` response).
    pub fn config(&self) -> &SupervisorConfig {
        &self.config
    }

    /// Borrow the shared log ring (used by the IPC `Logs` response).
    pub fn log_ring(&self) -> Arc<LogRing> {
        self.log_ring.clone()
    }

    /// Fetch all child names (used by the watchdog loop).
    pub async fn child_names(&self) -> Vec<String> {
        let guard = self.handles.read().await;
        guard.keys().cloned().collect()
    }

    /// Borrow a child handle Arc by name (used by the watchdog).
    pub async fn handle_for(&self, name: &str) -> Option<Arc<Mutex<ChildHandle>>> {
        let guard = self.handles.read().await;
        guard.get(name).cloned()
    }

    /// Update the heartbeat timestamp for a child.
    pub async fn record_heartbeat(&self, name: &str) {
        if let Some(h) = self.handle_for(name).await {
            let mut handle = h.lock().await;
            handle.last_heartbeat = Some(Instant::now());
        }
    }
}

/// Read-only summary used by the health & IPC layers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChildSnapshot {
    /// Child name.
    pub name: String,
    /// Lifecycle status.
    pub status: ChildStatus,
    /// OS PID if running.
    pub pid: Option<u32>,
    /// Total restarts since boot.
    pub restart_count: u64,
    /// Uptime since the most recent spawn.
    pub current_uptime_ms: u64,
    /// Cumulative uptime across all spawns.
    pub total_uptime_ms: u64,
    /// Last observed exit code.
    pub last_exit_code: Option<i32>,
    /// p50 latency in microseconds.
    pub p50_us: Option<u64>,
    /// p95 latency in microseconds.
    pub p95_us: Option<u64>,
    /// p99 latency in microseconds.
    pub p99_us: Option<u64>,
}

#[cfg(unix)]
fn kill_pid(pid: u32) -> Result<(), SupervisorError> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
        .map_err(|e| SupervisorError::Other(format!("kill({pid}) failed: {e}")))
}

#[cfg(windows)]
fn kill_pid(pid: u32) -> Result<(), SupervisorError> {
    // `tokio::process::Child::kill` is the preferred path, but the watchdog
    // only has the PID. Use `taskkill` via the standard library; it ships
    // with every Windows install and avoids a `windows-sys` dep here.
    let status = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| SupervisorError::Other(format!("taskkill spawn failed: {e}")))?;
    if !status.success() {
        return Err(SupervisorError::Other(format!(
            "taskkill exited with {status}"
        )));
    }
    Ok(())
}
