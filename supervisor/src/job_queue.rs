//! Supervisor-side job queue for CLI-dispatched work.
//!
//! Wiring:
//!   CLI `Build`/`Graphify` → IPC `DispatchJob(Job)` → [`JobQueue::submit`]
//!                                                         │
//!                                                         ▼
//!       router task drains pending jobs and calls
//!       `ChildManager::dispatch_to_pool(prefix, json_line)` to push the
//!       JSON-encoded job to a worker's stdin (existing v0.1 plumbing).
//!                                                         │
//!                                                         ▼
//!   worker processes the job → IPC `WorkerCompleteJob(JobId, Outcome)`
//!                                                         │
//!                                                         ▼
//!                         [`JobQueue::complete`] → CLI long-poll wakes
//!
//! Reliability (v0.3 MVP, documented in ARCHITECTURE.md):
//!   * Worker crash → the monitor sees Child::wait() exit and the restart
//!     loop respawns it. In-flight jobs assigned to that worker are
//!     re-queued via [`JobQueue::requeue_worker`] called from
//!     `ChildManager::monitor_child` on exit.
//!   * Supervisor crash → entire queue is lost. CLI times out and reports
//!     an error to the user. Durable queue is a future v0.4 item.
//!   * Backpressure → the queue is capped at `max_pending`; `submit`
//!     returns `SupervisorError::Other("queue full")` so the CLI can
//!     throttle.
//!
//! The router is kept deliberately simple: it picks the oldest pending
//! job and dispatches to any worker in the target pool. Sophisticated
//! scheduling (per-worker affinity, priority lanes, cancellation) is
//! out of scope for v0.3.

use crate::error::SupervisorError;
use common::jobs::{Job, JobId, JobOutcome};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot, Notify};
use tracing::{debug, warn};

/// A job that's been accepted by the supervisor but not yet reported as
/// complete.
#[derive(Debug)]
struct Tracked {
    job: Job,
    /// `Some(worker)` once the router has handed the job off.
    assigned_to: Option<String>,
    /// Instant the job entered the queue — drives the slow-job log line.
    enqueued_at: Instant,
    /// One-shot used to wake a CLI that's blocked on this job.
    waker: Option<oneshot::Sender<JobOutcome>>,
}

/// Thread-safe job queue.
///
/// Exposed as an `Arc<JobQueue>` from the [`ChildManager`] so the IPC
/// layer, the router task, and the monitor-on-exit path can all talk to
/// the same state.
pub struct JobQueue {
    inner: Mutex<Inner>,
    /// Notified whenever a new job is submitted so the router can wake
    /// without busy-polling.
    wake_router: Arc<Notify>,
    /// Notified whenever a job transitions to completed so anyone
    /// watching `snapshot()` can refresh.
    wake_watchers: Arc<Notify>,
    /// Bound on `pending.len() + in_flight.len()` — protects the
    /// supervisor from unbounded memory use if workers stall.
    max_pending: usize,
}

struct Inner {
    pending: VecDeque<JobId>,
    tracked: HashMap<JobId, Tracked>,
    in_flight: HashMap<JobId, String>, // worker name
    completed_count: u64,
    failed_count: u64,
    requeued_count: u64,
}

impl JobQueue {
    /// Build a new empty queue.
    pub fn new(max_pending: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                pending: VecDeque::new(),
                tracked: HashMap::new(),
                in_flight: HashMap::new(),
                completed_count: 0,
                failed_count: 0,
                requeued_count: 0,
            }),
            wake_router: Arc::new(Notify::new()),
            wake_watchers: Arc::new(Notify::new()),
            max_pending,
        }
    }

    /// Submit a new job; returns its id. Caller may optionally register
    /// a `waker` oneshot that will fire with the outcome when the worker
    /// reports back.
    ///
    /// Fails with `SupervisorError::Other("queue full")` when capacity
    /// is reached.
    pub fn submit(
        &self,
        job: Job,
        waker: Option<oneshot::Sender<JobOutcome>>,
    ) -> Result<JobId, SupervisorError> {
        let id = JobId::next();
        let mut g = self.inner.lock();
        if g.pending.len() + g.in_flight.len() >= self.max_pending {
            return Err(SupervisorError::Other(format!(
                "job queue full ({}); refusing submit",
                self.max_pending
            )));
        }
        g.pending.push_back(id);
        g.tracked.insert(
            id,
            Tracked {
                job,
                assigned_to: None,
                enqueued_at: Instant::now(),
                waker,
            },
        );
        drop(g);
        self.wake_router.notify_one();
        Ok(id)
    }

    /// Pop the next pending job for the router. Returns `(id, job)` or
    /// `None` if the queue is empty. The caller is expected to dispatch
    /// it immediately; on success they MUST call [`Self::mark_assigned`].
    pub fn next_pending(&self) -> Option<(JobId, Job)> {
        let mut g = self.inner.lock();
        let id = g.pending.pop_front()?;
        let tracked = g.tracked.get(&id)?;
        Some((id, tracked.job.clone()))
    }

    /// Record that `worker` accepted the job. Moves it to in-flight.
    pub fn mark_assigned(&self, id: JobId, worker: String) {
        let mut g = self.inner.lock();
        if let Some(t) = g.tracked.get_mut(&id) {
            t.assigned_to = Some(worker.clone());
        }
        g.in_flight.insert(id, worker);
    }

    /// Put a job that failed to dispatch back on the front of the queue
    /// so the router retries it. Used when `dispatch_to_pool` errors
    /// out (e.g. no worker in the pool has an open stdin yet).
    pub fn return_pending(&self, id: JobId) {
        let mut g = self.inner.lock();
        g.in_flight.remove(&id);
        if g.tracked.contains_key(&id) {
            g.pending.push_front(id);
        }
        drop(g);
        self.wake_router.notify_one();
    }

    /// Worker reported completion. Fires the waker (if any) and cleans
    /// up tracking state.
    pub fn complete(&self, id: JobId, outcome: JobOutcome) {
        let mut g = self.inner.lock();
        g.in_flight.remove(&id);
        let Some(tracked) = g.tracked.remove(&id) else {
            debug!(%id, "complete for unknown job; ignoring (likely already requeued)");
            return;
        };
        if outcome.is_ok() {
            g.completed_count += 1;
        } else {
            g.failed_count += 1;
        }
        drop(g);
        if let Some(waker) = tracked.waker {
            let _ = waker.send(outcome);
        }
        self.wake_watchers.notify_waiters();
    }

    /// Re-queue every job a (now-dead) worker had in flight. Called
    /// from [`crate::manager::ChildManager::monitor_child`] on exit so
    /// no work silently evaporates when a worker crashes.
    pub fn requeue_worker(&self, worker: &str) -> usize {
        let mut g = self.inner.lock();
        let ids: Vec<JobId> = g
            .in_flight
            .iter()
            .filter(|(_, w)| w.as_str() == worker)
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            g.in_flight.remove(id);
            if let Some(t) = g.tracked.get_mut(id) {
                t.assigned_to = None;
            }
            g.pending.push_front(*id);
            g.requeued_count += 1;
        }
        let n = ids.len();
        drop(g);
        if n > 0 {
            warn!(worker, count = n, "re-queued in-flight jobs after worker exit");
            self.wake_router.notify_one();
        }
        n
    }

    /// Snapshot for the IPC Status response.
    pub fn snapshot(&self) -> JobQueueSnapshot {
        let g = self.inner.lock();
        JobQueueSnapshot {
            pending: g.pending.len(),
            in_flight: g.in_flight.len(),
            completed: g.completed_count,
            failed: g.failed_count,
            requeued: g.requeued_count,
        }
    }

    /// Handle the router task can `.notified().await` on.
    pub fn router_waker(&self) -> Arc<Notify> {
        self.wake_router.clone()
    }
}

/// Read-only queue telemetry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobQueueSnapshot {
    /// Jobs still waiting for a worker.
    pub pending: usize,
    /// Jobs currently running on a worker.
    pub in_flight: usize,
    /// Cumulative successful completions.
    pub completed: u64,
    /// Cumulative failed completions.
    pub failed: u64,
    /// Cumulative re-queues triggered by worker crashes.
    pub requeued: u64,
}

/// Signal pumped through an mpsc so the router can be cleanly shut
/// down without racing against `Notify::notify_waiters`.
#[derive(Debug)]
pub enum RouterSignal {
    /// Shut the router down (graceful).
    Shutdown,
}

/// Handles to bring the router task up in [`crate::lib::run`].
pub struct RouterHandle {
    /// Drop-side of the shutdown channel; router exits when this drops.
    pub shutdown_tx: mpsc::Sender<RouterSignal>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_parse() -> Job {
        Job::Parse {
            file_path: std::path::PathBuf::from("/tmp/x.rs"),
            shard_root: std::path::PathBuf::from("/tmp/shard"),
        }
    }

    #[test]
    fn submit_then_next_pending_returns_it() {
        let q = JobQueue::new(16);
        let id = q.submit(dummy_parse(), None).unwrap();
        let (id2, job) = q.next_pending().expect("has pending");
        assert_eq!(id, id2);
        match job {
            Job::Parse { .. } => {}
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn queue_full_rejects_submit() {
        let q = JobQueue::new(2);
        q.submit(dummy_parse(), None).unwrap();
        q.submit(dummy_parse(), None).unwrap();
        let err = q.submit(dummy_parse(), None).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("queue full"), "unexpected: {msg}");
    }

    #[test]
    fn requeue_worker_puts_jobs_back() {
        let q = JobQueue::new(16);
        let id = q.submit(dummy_parse(), None).unwrap();
        let (id2, _) = q.next_pending().unwrap();
        assert_eq!(id, id2);
        q.mark_assigned(id, "parser-worker-0".into());
        let n = q.requeue_worker("parser-worker-0");
        assert_eq!(n, 1);
        let snap = q.snapshot();
        assert_eq!(snap.pending, 1);
        assert_eq!(snap.in_flight, 0);
        assert_eq!(snap.requeued, 1);
    }

    #[tokio::test]
    async fn complete_fires_waker() {
        let q = JobQueue::new(16);
        let (tx, rx) = oneshot::channel();
        let id = q.submit(dummy_parse(), Some(tx)).unwrap();
        q.complete(
            id,
            JobOutcome::Ok {
                payload: Some(serde_json::json!({"nodes": 3})),
            },
        );
        let outcome = rx.await.unwrap();
        assert!(outcome.is_ok());
    }
}
