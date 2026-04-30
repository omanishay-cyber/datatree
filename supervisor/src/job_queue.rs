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
//! Reliability (v0.3.0 — audit fix L5):
//!   * Worker crash → the monitor sees Child::wait() exit and the restart
//!     loop respawns it. In-flight jobs assigned to that worker are
//!     re-queued via [`JobQueue::requeue_worker`] called from
//!     `ChildManager::monitor_child` on exit.
//!   * Supervisor crash → durable. Every state transition is persisted
//!     to a single SQLite shard at `~/.mneme/run/jobs.db` via
//!     [`crate::job_queue_db::DurableJobQueue`]. Restart calls
//!     [`JobQueue::recover_from_disk`] to repopulate queued + in-flight
//!     jobs (the latter flipped back to queued so the new worker
//!     generation picks them up).
//!   * Backpressure → the queue is capped at `max_pending`; `submit`
//!     returns `SupervisorError::Other("queue full")` so the CLI can
//!     throttle.
//!
//! The router is kept deliberately simple: it picks the oldest pending
//! job and dispatches to any worker in the target pool. Sophisticated
//! scheduling (per-worker affinity, priority lanes, cancellation) is
//! out of scope for v0.3.

use crate::error::SupervisorError;
use crate::job_queue_db::DurableJobQueue;
use common::jobs::{Job, JobId, JobOutcome};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Notify};
use tracing::{debug, warn};

/// Slow-job log threshold. A job that sits in the queue or in-flight
/// for longer than this triggers a warn-level telemetry line so
/// operators can see queue stalls (REG-020 wire-up of `enqueued_at`).
const SLOW_JOB_THRESHOLD: Duration = Duration::from_secs(5);

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
///
/// Optional durable backing via [`DurableJobQueue`] — every state
/// transition is mirrored to disk so a supervisor crash never loses
/// queued or in-flight work. Constructed via [`JobQueue::with_durable`].
/// Tests that don't care about durability use [`JobQueue::new`] which
/// retains the v0.3 in-memory semantics.
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
    /// Disk-backed durable queue. Wrapped in an `Arc` so we can share
    /// it with the recovery routine and the IPC layer if it ever needs
    /// direct access. `None` for tests / legacy callers using
    /// [`JobQueue::new`].
    durable: Option<Arc<DurableJobQueue>>,
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
    /// Build a new empty queue with no durable backing.
    ///
    /// Retains the v0.3 in-memory semantics. Suitable for unit tests
    /// and for callers that explicitly do NOT want crash recovery.
    /// Production supervisors should use [`Self::with_durable`].
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
            durable: None,
        }
    }

    /// Build a new queue with a durable on-disk companion.
    ///
    /// On construction we ALSO seed [`JobId::next`] past any pre-existing
    /// row so freshly-submitted jobs cannot collide with persisted ids.
    pub fn with_durable(
        max_pending: usize,
        durable: Arc<DurableJobQueue>,
    ) -> Result<Self, SupervisorError> {
        let max_id = durable.max_id()?;
        if max_id > 0 {
            JobId::seed_to(max_id + 1);
        }
        Ok(Self {
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
            durable: Some(durable),
        })
    }

    /// Borrow the durable backing, if attached.
    pub fn durable(&self) -> Option<Arc<DurableJobQueue>> {
        self.durable.clone()
    }

    /// Recover queued + in-flight jobs persisted by a previous
    /// supervisor session. Caller is the boot path in `crate::run`.
    ///
    /// Order:
    ///   1. flip every `in_flight` row back to `queued` on disk (a
    ///      worker that was running them is gone — the next router
    ///      pass will re-dispatch);
    ///   2. read every `queued` row in id order and push it onto the
    ///      in-memory pending queue;
    ///   3. `JobId::next` was already seeded past max(id) by
    ///      `with_durable`, so new submissions stay monotonic.
    ///
    /// Returns the count of jobs that were recovered.
    pub fn recover_from_disk(&self) -> Result<usize, SupervisorError> {
        let Some(d) = self.durable.as_ref() else {
            return Ok(0);
        };
        let flipped = d.requeue_all_in_flight()?;
        if flipped > 0 {
            warn!(
                count = flipped,
                "recovered in-flight jobs from previous supervisor session; \
                 flipped back to queued"
            );
        }
        let queued = d.recover_queued()?;
        let n = queued.len();
        if n > 0 {
            let mut g = self.inner.lock();
            for r in queued {
                g.pending.push_back(r.id);
                g.tracked.insert(
                    r.id,
                    Tracked {
                        job: r.job,
                        assigned_to: None,
                        enqueued_at: Instant::now(),
                        waker: None,
                    },
                );
            }
            drop(g);
            self.wake_router.notify_one();
        }
        Ok(n)
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
        // Persist BEFORE returning the id so a crash between submit
        // and the first state read can recover the job.
        if let Some(d) = self.durable.as_ref() {
            if let Err(e) = d.push(id, &job) {
                warn!(%id, error = %e, "durable submit persist failed; in-memory only");
            }
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
        // NEW-011: invariant — every id in `pending` MUST have a
        // matching entry in `tracked`. If not, something inserted into
        // `pending` without going through `submit`, or a racing remove
        // missed the pending side. Surface the popped + tracked counts
        // so the bug is obvious in logs, debug_assert in dev, and
        // return None in release so we never hand out a phantom job.
        let tracked_count = g.tracked.len();
        let Some(tracked) = g.tracked.get(&id) else {
            warn!(
                popped = %id,
                tracked = tracked_count,
                "job_queue invariant violated: popped={}, tracked={}",
                id,
                tracked_count
            );
            debug_assert!(
                false,
                "job_queue invariant violated: popped={id} but {tracked_count} tracked"
            );
            return None;
        };
        Some((id, tracked.job.clone()))
    }

    /// Record that `worker` accepted the job. Moves it to in-flight.
    pub fn mark_assigned(&self, id: JobId, worker: String) {
        let mut g = self.inner.lock();
        if let Some(t) = g.tracked.get_mut(&id) {
            t.assigned_to = Some(worker.clone());
        }
        g.in_flight.insert(id, worker.clone());
        drop(g);
        if let Some(d) = self.durable.as_ref() {
            if let Err(e) = d.mark_in_flight(id, &worker) {
                warn!(%id, worker, error = %e, "durable mark_in_flight failed");
            }
        }
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
        if let Some(d) = self.durable.as_ref() {
            if let Err(e) = d.requeue(id) {
                warn!(%id, error = %e, "durable return_pending failed");
            }
        }
        self.wake_router.notify_one();
    }

    /// Worker reported completion. Fires the waker (if any) and cleans
    /// up tracking state.
    ///
    /// Returns the worker name the job was assigned to (if any) so the
    /// caller can update per-worker telemetry. Returns `None` when the
    /// job id is unknown (already requeued or never existed).
    pub fn complete(&self, id: JobId, outcome: JobOutcome) -> Option<String> {
        let mut g = self.inner.lock();
        g.in_flight.remove(&id);
        let Some(tracked) = g.tracked.remove(&id) else {
            debug!(%id, "complete for unknown job; ignoring (likely already requeued)");
            return None;
        };
        if outcome.is_ok() {
            g.completed_count += 1;
        } else {
            g.failed_count += 1;
        }
        let worker = tracked.assigned_to.clone();
        drop(g);
        if let Some(d) = self.durable.as_ref() {
            if let Err(e) = d.mark_done(id, &outcome) {
                warn!(%id, error = %e, "durable mark_done failed");
            }
        }
        // REG-020: log queue latency for slow jobs. `enqueued_at` was
        // previously read-only telemetry; this wires it into actual
        // operator-visible output without changing the queue's hot path
        // beyond a single Instant subtraction.
        let queue_latency = tracked.enqueued_at.elapsed();
        if queue_latency > SLOW_JOB_THRESHOLD {
            warn!(
                %id,
                queue_latency_ms = queue_latency.as_millis() as u64,
                "slow job: total time from submit to complete exceeded threshold"
            );
        }
        if let Some(waker) = tracked.waker {
            let _ = waker.send(outcome);
        }
        self.wake_watchers.notify_waiters();
        worker
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
            warn!(
                worker,
                count = n,
                "re-queued in-flight jobs after worker exit"
            );
            if let Some(d) = self.durable.as_ref() {
                for id in &ids {
                    if let Err(e) = d.requeue(*id) {
                        warn!(%id, worker, error = %e, "durable requeue failed");
                    }
                }
            }
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

// NEW-017: `RouterHandle` and `RouterSignal` were dead code — the
// router shuts down via the shared `Notify` in `lib::run`, no mpsc
// channel is involved. They have been removed; if structured router
// shutdown is needed in v0.4 it should be reintroduced together with
// the consumer that actually uses it.

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
                duration_ms: 7,
                stats: serde_json::json!({"nodes": 3}),
            },
        );
        let outcome = rx.await.unwrap();
        assert!(outcome.is_ok());
    }

    #[test]
    fn complete_returns_assigned_worker() {
        let q = JobQueue::new(16);
        let id = q.submit(dummy_parse(), None).unwrap();
        let (id2, _) = q.next_pending().unwrap();
        assert_eq!(id, id2);
        q.mark_assigned(id, "parser-worker-1".into());
        let worker = q.complete(
            id,
            JobOutcome::Ok {
                payload: None,
                duration_ms: 13,
                stats: serde_json::Value::Null,
            },
        );
        assert_eq!(worker.as_deref(), Some("parser-worker-1"));
    }

    #[test]
    fn complete_returns_none_for_unknown_job() {
        let q = JobQueue::new(16);
        let bogus = JobId::next();
        let w = q.complete(
            bogus,
            JobOutcome::Ok {
                payload: None,
                duration_ms: 0,
                stats: serde_json::Value::Null,
            },
        );
        assert!(w.is_none());
    }
}
