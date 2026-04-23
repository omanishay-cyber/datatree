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
            restart_window: VecDeque::new(),
            status: ChildStatus::Pending,
            last_exit_code: None,
            last_heartbeat: None,
            total_uptime: Duration::ZERO,
            current_backoff: initial_backoff,
            latency_samples_us: VecDeque::new(),
            stdin: None,
        }
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
