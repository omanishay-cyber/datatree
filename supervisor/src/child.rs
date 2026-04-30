//! Child process specifications and runtime handles.
//!
//! A [`ChildSpec`] is the *static* description of a worker (loaded from
//! config). A [`ChildHandle`] is the *runtime* state that a [`crate::ChildManager`]
//! mutates as the worker starts, crashes, and restarts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::ChildStdin;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Static description of one worker process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSpec {
    /// Stable identifier (used in logs, IPC, and the SLA dashboard).
    pub name: String,
    /// Path to the executable (or a name resolved via `PATH`, e.g. `bun`).
    pub command: String,
    /// CLI arguments passed to the child.
    pub args: Vec<String>,
    /// Extra environment variables (merged on top of the supervisor's env).
    pub env: Vec<(String, String)>,
    /// What to do when the child exits.
    pub restart: RestartStrategy,
    /// Max RSS in MB before the watchdog OOM-kills and restarts.
    pub rss_limit_mb: Option<u64>,
    /// Sustained CPU usage percent (per-core) before throttle-warn.
    pub cpu_limit_percent: Option<u32>,
    /// Optional health endpoint exposed by the child over its own IPC socket.
    pub health_endpoint: Option<String>,
    /// Optional per-child override for the watchdog's missed-heartbeat
    /// deadline. `None` means "use the supervisor default" (see
    /// [`crate::watchdog::HEARTBEAT_DEADLINE`]). Workers that legitimately
    /// stay idle for long stretches (e.g. an md-ingest sidecar that only
    /// runs on demand) should set this to a large value rather than have
    /// the watchdog kill them mid-sleep.
    #[serde(
        default,
        with = "opt_duration_secs",
        skip_serializing_if = "Option::is_none"
    )]
    pub heartbeat_deadline: Option<Duration>,
}

mod opt_duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Option<Duration>, ser: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => ser.serialize_some(&d.as_secs()),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Option<Duration>, D::Error> {
        let opt: Option<u64> = Option::deserialize(de)?;
        Ok(opt.map(Duration::from_secs))
    }
}

/// Restart semantics for a child.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestartStrategy {
    /// Always restart, regardless of exit code (default for workers).
    Permanent,
    /// Restart only on non-zero exit.
    Transient,
    /// Never restart (one-shot).
    Temporary,
}

/// Lifecycle state of a child.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChildStatus {
    /// Not yet started.
    Pending,
    /// Currently running.
    Running,
    /// In the back-off interval after a crash.
    Restarting,
    /// Crashed too many times within the budget window.
    Degraded,
    /// Cleanly stopped by the supervisor.
    Stopped,
}

/// Runtime handle for a single child. Owned by the [`crate::ChildManager`].
#[derive(Debug)]
pub struct ChildHandle {
    /// Static spec we were spawned from.
    pub spec: ChildSpec,
    /// OS process id (None when not running).
    pub pid: Option<u32>,
    /// Wall-clock time of the last successful spawn.
    pub last_started_at: Option<DateTime<Utc>>,
    /// Monotonic time of the last successful spawn (used for uptime math).
    pub last_started_instant: Option<Instant>,
    /// Wall-clock time of the most recent restart.
    pub last_restart_at: Option<DateTime<Utc>>,
    /// Total restarts since supervisor boot.
    pub restart_count: u64,
    /// Bug L (postmortem §12.1 follow-up): total restart requests that
    /// were observed but could NOT be queued because the restart
    /// channel had been closed (receiver dropped — supervisor
    /// shutting down). With the unbounded channel introduced by Bug J
    /// this is the only remaining drop path, but it is still a
    /// debugging signal worth surfacing alongside `restart_count` so
    /// `mneme doctor` and Prometheus scrapers can see it.
    pub restart_dropped_count: u64,
    /// Rolling restart timestamps used for budget enforcement.
    pub restart_window: VecDeque<Instant>,
    /// Current lifecycle state.
    pub status: ChildStatus,
    /// Last observed exit code (if any).
    pub last_exit_code: Option<i32>,
    /// Last heartbeat received from the child.
    pub last_heartbeat: Option<Instant>,
    /// Total uptime accumulated across restarts.
    pub total_uptime: Duration,
    /// Current backoff interval used by the next restart.
    pub current_backoff: Duration,
    /// Most recent latency observations in microseconds (capped).
    pub latency_samples_us: VecDeque<u64>,
    /// Writable stdin handle for worker-bound job dispatch (framed JSON lines).
    /// None between spawn attempts or for workers that don't consume stdin.
    pub stdin: Option<Arc<Mutex<ChildStdin>>>,
    /// ID of the job most recently reported complete via `WorkerCompleteJob`.
    /// Keeps across restarts so the user sees "last job before the crash".
    pub last_job_id: Option<u64>,
    /// Wall-clock ms the worker spent on its most recent job.
    pub last_job_duration_ms: Option<u64>,
    /// Outcome of the most recent job (`"ok"` or `"error"`).
    pub last_job_status: Option<&'static str>,
    /// Rolling window of per-job durations in ms; used for `avg_job_ms`.
    pub job_durations_ms: VecDeque<u64>,
    /// Wall-clock time (UTC) the most recent `WorkerCompleteJob` arrived.
    pub last_job_completed_at: Option<DateTime<Utc>>,
    /// Cumulative jobs reported complete via IPC since boot.
    pub total_jobs_completed: u64,
    /// Cumulative failed jobs reported complete via IPC since boot.
    pub total_jobs_failed: u64,
    /// Cumulative jobs dispatched to this worker (router-side counter).
    /// Bumped from `ChildManager::record_job_dispatch` after a successful
    /// stdin write. Phase-A C5: gives `/health` a non-null counter even
    /// before the worker reports a `WorkerCompleteJob`, which previously
    /// left `total_jobs_completed=0` looking like the supervisor was idle.
    pub total_jobs_dispatched: u64,
    /// Most recent resident-set-size sample (in bytes) for this worker.
    /// Populated by the supervisor's periodic RSS refresher (sysinfo,
    /// see `lib.rs::run_rss_refresher`). `None` between samples or
    /// before the first refresh has run. Phase-A C1.
    pub rss_bytes: Option<u64>,
    /// JoinHandle for the spawned stdout-forwarder task. Kept so that on
    /// restart / shutdown we can `.abort()` the task instead of leaking a
    /// reader that would otherwise live until its pipe is closed by the
    /// OS — and on Windows that close-of-pipe path is unreliable. See
    /// I-5 / NEW-008.
    pub stdout_task: Option<JoinHandle<()>>,
    /// JoinHandle for the spawned stderr-forwarder task. Same rationale
    /// as `stdout_task`.
    pub stderr_task: Option<JoinHandle<()>>,
    /// Bug I defensive fix: tracks whether we have already emitted the
    /// "child recovered from crash loop after stable 60s uptime" log
    /// line for the current spawn lifetime.
    ///
    /// Flips to `true` exactly once when the manager's recovery checker
    /// observes:
    ///   1. `restart_count >= 3` (the child has been crash-looping), AND
    ///   2. `current_uptime() >= 60s` (the most recent spawn has been
    ///      stable long enough to count as recovered).
    ///
    /// `record_restart` resets the flag to `false` so a re-recovery
    /// after a future crash-loop emits a fresh log line. One-shot per
    /// worker per recovery cycle.
    pub crash_loop_recovery_logged: bool,
}

impl ChildHandle {
    /// Construct a fresh handle in the [`ChildStatus::Pending`] state.
    pub fn new(spec: ChildSpec, initial_backoff: Duration) -> Self {
        Self {
            spec,
            pid: None,
            last_started_at: None,
            last_started_instant: None,
            last_restart_at: None,
            restart_count: 0,
            restart_dropped_count: 0,
            restart_window: VecDeque::new(),
            status: ChildStatus::Pending,
            last_exit_code: None,
            last_heartbeat: None,
            total_uptime: Duration::ZERO,
            current_backoff: initial_backoff,
            latency_samples_us: VecDeque::new(),
            stdin: None,
            last_job_id: None,
            last_job_duration_ms: None,
            last_job_status: None,
            job_durations_ms: VecDeque::new(),
            last_job_completed_at: None,
            total_jobs_completed: 0,
            total_jobs_failed: 0,
            total_jobs_dispatched: 0,
            rss_bytes: None,
            stdout_task: None,
            stderr_task: None,
            crash_loop_recovery_logged: false,
        }
    }

    /// Abort any tracked stdout/stderr forwarder JoinHandles. Called from
    /// the manager when a child is being respawned (so the old forwarders
    /// don't outlive the dead pipe) and during graceful shutdown.
    pub fn abort_io_tasks(&mut self) {
        if let Some(h) = self.stdout_task.take() {
            h.abort();
        }
        if let Some(h) = self.stderr_task.take() {
            h.abort();
        }
    }

    /// Record the outcome of a `WorkerCompleteJob` IPC message. Keeps a
    /// rolling window (last 256) of durations for `avg_job_ms`.
    ///
    /// Phase-A C5: also feeds the duration (converted to microseconds)
    /// into `latency_samples_us` so that `latency_percentiles_us`
    /// — which previously had no producer wired — actually reports
    /// p50/p95/p99 on `/health` once jobs start completing. Keeping the
    /// existing `record_latency_us` path additive means a worker that
    /// reports its own internal latency over a future IPC channel can
    /// still feed in higher-resolution samples without us double-counting.
    pub fn record_job_completion(&mut self, job_id: u64, status: &'static str, duration_ms: u64) {
        self.last_job_id = Some(job_id);
        self.last_job_duration_ms = Some(duration_ms);
        self.last_job_status = Some(status);
        self.last_job_completed_at = Some(Utc::now());
        self.job_durations_ms.push_back(duration_ms);
        while self.job_durations_ms.len() > 256 {
            self.job_durations_ms.pop_front();
        }
        // C5: keep p50/p95/p99 honest. Multiplying ms→us caps headroom
        // at u64::MAX/1000 ≈ 5.8 trillion ms which is comfortably above
        // any realistic single-job duration; saturating_mul guards the
        // pathological case anyway.
        let sample_us = (duration_ms as u128).saturating_mul(1000);
        self.record_latency_us(sample_us.min(u64::MAX as u128) as u64);
        if status == "ok" {
            self.total_jobs_completed = self.total_jobs_completed.saturating_add(1);
        } else {
            self.total_jobs_failed = self.total_jobs_failed.saturating_add(1);
        }
    }

    /// Record that the router has successfully dispatched a job to this
    /// worker. Phase-A C5: lets `/health` show `total_jobs_dispatched`
    /// rising even when the worker hasn't yet reported any
    /// `WorkerCompleteJob` back — useful when debugging "is anything
    /// flowing" vs "is the worker stuck".
    pub fn record_job_dispatch(&mut self) {
        self.total_jobs_dispatched = self.total_jobs_dispatched.saturating_add(1);
    }

    /// Update the most recent RSS sample for this child. Phase-A C1.
    /// Called by the supervisor's RSS refresher task — `None` resets the
    /// sample (used when the child has exited and we want `/health` to
    /// stop reporting stale memory readings).
    pub fn record_rss_bytes(&mut self, rss: Option<u64>) {
        self.rss_bytes = rss;
    }

    /// Mean wall-clock time across the rolling job-duration window.
    /// `None` when no jobs have completed yet.
    pub fn avg_job_ms(&self) -> Option<u64> {
        if self.job_durations_ms.is_empty() {
            return None;
        }
        let sum: u64 = self.job_durations_ms.iter().sum();
        Some(sum / self.job_durations_ms.len() as u64)
    }

    /// Push a fresh restart timestamp and prune entries older than `window`.
    pub fn record_restart(&mut self, window: Duration) {
        let now = Instant::now();
        self.restart_window.push_back(now);
        while let Some(front) = self.restart_window.front() {
            if now.duration_since(*front) > window {
                self.restart_window.pop_front();
            } else {
                break;
            }
        }
        self.restart_count = self.restart_count.saturating_add(1);
        self.last_restart_at = Some(Utc::now());
        // Bug I: a fresh restart means the previous "recovered" state
        // (if any) is invalidated. Clear the one-shot flag so that, if
        // the new spawn ALSO stabilises after 60s, the recovery log
        // line is emitted again — every recovery cycle gets its own
        // anchor.
        self.crash_loop_recovery_logged = false;
    }

    /// Number of restarts that have happened inside the rolling window.
    pub fn restarts_in_window(&self, window: Duration) -> u32 {
        let now = Instant::now();
        self.restart_window
            .iter()
            .filter(|t| now.duration_since(**t) <= window)
            .count() as u32
    }

    /// Current observed uptime since the last spawn (or zero if not running).
    pub fn current_uptime(&self) -> Duration {
        self.last_started_instant
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Record a latency sample (microseconds) and cap to the last 4096.
    pub fn record_latency_us(&mut self, sample: u64) {
        self.latency_samples_us.push_back(sample);
        while self.latency_samples_us.len() > 4096 {
            self.latency_samples_us.pop_front();
        }
    }

    /// Compute (p50, p95, p99) over the latency samples. `None` if empty.
    pub fn latency_percentiles_us(&self) -> Option<(u64, u64, u64)> {
        if self.latency_samples_us.is_empty() {
            return None;
        }
        let mut v: Vec<u64> = self.latency_samples_us.iter().copied().collect();
        v.sort_unstable();
        let pick = |p: f64| -> u64 {
            let idx = ((v.len() as f64) * p).floor() as usize;
            v[idx.min(v.len() - 1)]
        };
        Some((pick(0.50), pick(0.95), pick(0.99)))
    }
}
