//! Child process manager.
//!
//! Owns every [`ChildHandle`], spawns workers via tokio's [`tokio::process`]
//! API, watches each one for exit, applies the exponential back-off restart
//! policy, and pipes stdout/stderr into the shared [`LogRing`].

use crate::child::{ChildHandle, ChildSpec, ChildStatus, RestartStrategy};
use crate::config::SupervisorConfig;
use crate::error::SupervisorError;
use crate::job_queue::JobQueue;
use crate::log_ring::LogRing;
use chrono::Utc;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Reason a monitor task queued a restart.
#[derive(Debug, Clone)]
pub(crate) struct RestartRequest {
    /// Child name.
    pub name: String,
    /// Exit code observed by the monitor.
    pub exit_code: i32,
    /// Time the exit was observed — used to compute the backoff delay.
    pub queued_at: Instant,
}

/// Owns all running children and their restart state.
pub struct ChildManager {
    config: SupervisorConfig,
    log_ring: Arc<LogRing>,
    handles: RwLock<HashMap<String, Arc<Mutex<ChildHandle>>>>,
    monitors: Mutex<HashMap<String, JoinHandle<()>>>,
    shutdown_flag: Mutex<bool>,
    /// Sender used by each monitor task to queue a restart request. The
    /// receive end lives inside a separate task started by
    /// [`Self::start_restart_loop`] — this indirection is what breaks the
    /// `tokio::process::Child` Send-recursion cycle that forced v0.1 to
    /// ship with auto-restart disabled.
    restart_tx: mpsc::UnboundedSender<RestartRequest>,
    restart_rx: Mutex<Option<mpsc::UnboundedReceiver<RestartRequest>>>,
    /// Shared job queue (set via [`Self::attach_job_queue`]). The queue
    /// tracks CLI-submitted work items (`Job::Parse`, `Job::Scan`, …)
    /// that the router task drains by pushing JSON lines to worker
    /// stdin via [`Self::dispatch_to_pool`].
    job_queue: RwLock<Option<Arc<JobQueue>>>,
}

impl ChildManager {
    /// Construct a manager from a fully-validated config.
    pub fn new(config: SupervisorConfig, log_ring: Arc<LogRing>) -> Self {
        let (restart_tx, restart_rx) = mpsc::unbounded_channel();
        Self {
            config,
            log_ring,
            handles: RwLock::new(HashMap::new()),
            monitors: Mutex::new(HashMap::new()),
            shutdown_flag: Mutex::new(false),
            restart_tx,
            restart_rx: Mutex::new(Some(restart_rx)),
            job_queue: RwLock::new(None),
        }
    }

    /// Attach a shared [`JobQueue`]. Must be called once during
    /// supervisor boot BEFORE the first worker can crash, so requeue
    /// logic never misses an exit.
    pub async fn attach_job_queue(&self, queue: Arc<JobQueue>) {
        let mut g = self.job_queue.write().await;
        *g = Some(queue);
    }

    /// Borrow the attached job queue, if any.
    pub async fn job_queue(&self) -> Option<Arc<JobQueue>> {
        self.job_queue.read().await.clone()
    }

    /// Take ownership of the restart-request receiver. The supervisor
    /// spawns exactly one restart loop per manager; calling this a second
    /// time returns `None`.
    pub(crate) async fn take_restart_rx(
        &self,
    ) -> Option<mpsc::UnboundedReceiver<RestartRequest>> {
        let mut guard = self.restart_rx.lock().await;
        guard.take()
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

        let mut child = self.spawn_os_process(&spec).await?;
        let pid = child.id();
        // Capture stdin BEFORE moving the child into the monitor task.
        // This lets the manager dispatch worker jobs later without needing
        // a handle to the Child itself (which is !Send across awaits on
        // Windows named-pipe stdio handles).
        let stdin_handle = child.stdin.take().map(|s| Arc::new(Mutex::new(s)));

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
                h.stdin = stdin_handle;
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
            h.stdin = None;
        }

        // If this worker had jobs in flight, push them back onto the
        // queue so the next worker in the pool picks them up. Skipping
        // this means a Parse/Scan/Embed job silently disappears on every
        // crash — the whole point of supervisor-mediated dispatch.
        if let Some(queue) = self.job_queue.read().await.clone() {
            let n = queue.requeue_worker(&name);
            if n > 0 {
                info!(child = %name, jobs = n, "requeued in-flight jobs after exit");
            }
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

        // Mark the child as Restarting and queue a request on the restart
        // channel. The dedicated restart loop (see `run_restart_loop`)
        // performs the actual respawn. This decouples the monitor task —
        // which still owns the dead `Child` handle until function return —
        // from the respawn code path that creates a NEW `Child`. The old
        // recursive `spawn_child` → `monitor_child` call stack forced the
        // compiler to prove the combined future was Send even though
        // Windows named-pipe stdio pieces make `Child` ambiguous across
        // awaits. Splitting via an mpsc boundary lets each side be Send
        // independently.
        {
            let mut h = handle.lock().await;
            h.status = ChildStatus::Restarting;
        }
        if let Err(e) = self.restart_tx.send(RestartRequest {
            name: name.clone(),
            exit_code,
            queued_at: Instant::now(),
        }) {
            error!(child = %name, error = %e, "restart channel closed; cannot queue respawn");
        } else {
            debug!(child = %name, exit_code, "restart request queued");
        }
    }

    /// Process queued restart requests forever. Owned by a single task.
    ///
    /// This loop pulls `RestartRequest`s off the channel filled by
    /// [`Self::monitor_child`] and performs the respawn with exponential
    /// backoff + restart-budget enforcement. Because it runs in its own
    /// tokio task with a fresh stack, the opaque-future Send-inference
    /// cycle that blocked v0.1 is avoided structurally.
    pub(crate) async fn run_restart_loop(
        self: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<RestartRequest>,
    ) {
        info!("restart loop online");
        while let Some(req) = rx.recv().await {
            if *self.shutdown_flag.lock().await {
                debug!(child = %req.name, "shutdown in progress; ignoring restart request");
                continue;
            }
            if let Err(e) = self.respawn_one(&req).await {
                error!(child = %req.name, error = %e, "restart failed");
            }
        }
        info!("restart loop offline");
    }

    async fn respawn_one(
        self: &Arc<Self>,
        req: &RestartRequest,
    ) -> Result<(), SupervisorError> {
        let policy = self.config.default_restart_policy.clone();
        let handle = match self.handle_for(&req.name).await {
            Some(h) => h,
            None => {
                warn!(child = %req.name, "restart for unknown child; dropping");
                return Ok(());
            }
        };

        // Compute backoff + enforce budget under the handle lock.
        let (delay, spec) = {
            let mut h = handle.lock().await;
            h.record_restart(policy.budget_window);
            let in_window = h.restarts_in_window(policy.budget_window);
            if in_window > policy.max_restarts_per_window {
                h.status = ChildStatus::Degraded;
                warn!(
                    child = %req.name,
                    restarts = in_window,
                    window_secs = policy.budget_window.as_secs(),
                    "restart budget exceeded; marking degraded"
                );
                return Err(SupervisorError::RestartBudgetExceeded {
                    name: req.name.clone(),
                    restarts: in_window,
                    window_secs: policy.budget_window.as_secs(),
                });
            }
            let next =
                (h.current_backoff.as_millis() as f32 * policy.backoff_multiplier) as u64;
            let capped = next.min(policy.max_backoff.as_millis() as u64);
            let delay = h.current_backoff;
            h.current_backoff = Duration::from_millis(capped.max(1));
            (delay, h.spec.clone())
        };

        // Sleep the backoff interval. No `Child` is in scope here, so the
        // compiler can trivially prove the future is Send.
        debug!(
            child = %req.name,
            delay_ms = delay.as_millis() as u64,
            exit_code = req.exit_code,
            "restart scheduled"
        );
        tokio::time::sleep(delay).await;

        if *self.shutdown_flag.lock().await {
            return Ok(());
        }

        // Spawn a fresh child. spawn_child is its own future with its own
        // stack, so nothing in the old monitor's frame is borrowed here.
        self.spawn_child(spec).await?;
        info!(child = %req.name, "child respawned");
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
                last_started_at: h.last_started_at,
                last_restart_at: h.last_restart_at,
                p50_us: percentiles.map(|p| p.0),
                p95_us: percentiles.map(|p| p.1),
                p99_us: percentiles.map(|p| p.2),
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
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

    /// Dispatch a single JSON-line job to the named worker via its stdin
    /// pipe. The caller serialises the payload; the manager appends a
    /// trailing newline and flushes.
    ///
    /// Returns `Err(SupervisorError::Other)` if the child is not running,
    /// its stdin handle has been reaped, or the write fails.
    pub async fn dispatch_job(
        &self,
        name: &str,
        payload: &str,
    ) -> Result<(), SupervisorError> {
        let handle = self
            .handle_for(name)
            .await
            .ok_or_else(|| SupervisorError::Other(format!("unknown child: {name}")))?;
        let stdin_arc = {
            let h = handle.lock().await;
            if h.status != ChildStatus::Running {
                return Err(SupervisorError::Other(format!(
                    "child '{name}' not running (status {:?})",
                    h.status
                )));
            }
            h.stdin
                .clone()
                .ok_or_else(|| SupervisorError::Other(format!("child '{name}' has no stdin")))?
        };
        let mut stdin = stdin_arc.lock().await;
        stdin.write_all(payload.as_bytes()).await?;
        if !payload.ends_with('\n') {
            stdin.write_all(b"\n").await?;
        }
        stdin.flush().await?;
        Ok(())
    }

    /// Pick a worker matching `prefix` (e.g. `"parser-worker-"`) in round
    /// robin fashion and dispatch a job to it. Used by the daemon's
    /// in-process router so the CLI doesn't have to know how many workers
    /// exist.
    pub async fn dispatch_to_pool(
        &self,
        prefix: &str,
        payload: &str,
    ) -> Result<String, SupervisorError> {
        let mut candidates: Vec<String> = {
            let guard = self.handles.read().await;
            guard
                .keys()
                .filter(|n| n.starts_with(prefix))
                .cloned()
                .collect()
        };
        candidates.sort();
        for name in &candidates {
            match self.dispatch_job(name, payload).await {
                Ok(()) => return Ok(name.clone()),
                Err(e) => {
                    debug!(child = %name, error = %e, "pool dispatch attempt failed; trying next");
                }
            }
        }
        Err(SupervisorError::Other(format!(
            "no worker matching prefix '{prefix}' is accepting jobs ({} candidates)",
            candidates.len()
        )))
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
    /// Wall-clock time of the most recent successful spawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Wall-clock time of the most recent auto-restart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_restart_at: Option<chrono::DateTime<chrono::Utc>>,
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
