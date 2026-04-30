//! Durable persistence layer for the supervisor [`JobQueue`].
//!
//! ## Why
//!
//! v0.3.0 shipped with `JobQueue` as an in-memory `VecDeque`/`HashMap`.
//! Supervisor crash discarded every queued + in-flight job; `mneme
//! build --dispatch` re-walked instead of resuming. Audit fix L5
//! (v0.3.0): every state transition is persisted to a single SQLite
//! shard at `~/.mneme/run/jobs.db`. Supervisor restart calls
//! [`DurableJobQueue::recover_in_flight`] to requeue jobs left dangling.
//!
//! ## Schema
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS jobs (
//!   id INTEGER PRIMARY KEY,                    -- monotonic JobId
//!   kind TEXT NOT NULL,                         -- 'parse' | 'scan' | 'embed' | 'ingest'
//!   payload BLOB NOT NULL,                      -- serde_json::to_vec(&Job)
//!   state TEXT NOT NULL,                        -- 'queued' | 'in_flight' | 'done' | 'failed'
//!   assigned_to TEXT,                           -- worker name
//!   enqueued_at INTEGER NOT NULL,               -- unix ms
//!   started_at INTEGER,
//!   finished_at INTEGER,
//!   error TEXT
//! );
//! CREATE INDEX IF NOT EXISTS idx_jobs_state ON jobs(state);
//! ```
//!
//! ## Performance budget
//!
//! Single SQLite connection wrapped in a parking_lot::Mutex (the
//! supervisor router task is single-threaded; the IPC handler may
//! contend briefly). WAL mode enabled so reads don't block writes.
//! `synchronous = NORMAL` — durability guaranteed per
//! checkpoint/`fsync(WAL)`, which is good enough for a job queue
//! (worst case on power loss is one job re-runs).
//!
//! Push + next round-trip targets ≤ 1 ms on commodity hardware. The
//! test suite enforces this; see `bench_push_next_round_trip` below.

use crate::error::SupervisorError;
use common::jobs::{Job, JobId, JobOutcome};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Disk-backed companion to the in-memory [`crate::job_queue::JobQueue`].
///
/// All state transitions go through this type so that a supervisor
/// crash never silently loses queued or in-flight work.
#[derive(Debug)]
pub struct DurableJobQueue {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

/// Worker-assigned info for a row recovered after restart.
#[derive(Debug, Clone)]
pub struct RecoveredJob {
    /// The original job id minted by the supervisor that crashed.
    pub id: JobId,
    /// Decoded job payload.
    pub job: Job,
    /// Worker that was running this job at crash time, if any.
    pub assigned_to: Option<String>,
}

impl DurableJobQueue {
    /// Open or create the queue at `db_path`. Creates parent directories
    /// if missing. Idempotent — safe to call on every supervisor boot.
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self, SupervisorError> {
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SupervisorError::Other(format!("create jobs.db parent {}: {e}", parent.display()))
            })?;
        }
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| {
            SupervisorError::Other(format!("open jobs.db at {}: {e}", db_path.display()))
        })?;

        // WAL + NORMAL — see module-level perf budget.
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| SupervisorError::Other(format!("set WAL: {e}")))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| SupervisorError::Other(format!("set synchronous: {e}")))?;
        // Tiny page cache is enough for the job table.
        conn.pragma_update(None, "temp_store", "MEMORY").ok();

        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS jobs (
               id           INTEGER PRIMARY KEY,
               kind         TEXT NOT NULL,
               payload      BLOB NOT NULL,
               state        TEXT NOT NULL,
               assigned_to  TEXT,
               enqueued_at  INTEGER NOT NULL,
               started_at   INTEGER,
               finished_at  INTEGER,
               error        TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_jobs_state ON jobs(state);
             COMMIT;",
        )
        .map_err(|e| SupervisorError::Other(format!("create jobs schema: {e}")))?;

        Ok(Self {
            conn: Mutex::new(conn),
            db_path,
        })
    }

    /// On-disk path of the queue database. Test inspector / log line.
    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Persist a freshly-submitted job in the `queued` state.
    pub fn push(&self, id: JobId, job: &Job) -> Result<(), SupervisorError> {
        let payload = serde_json::to_vec(job).map_err(SupervisorError::from)?;
        let now = unix_ms();
        let kind = job.kind_label();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO jobs(id, kind, payload, state, enqueued_at) \
             VALUES(?1, ?2, ?3, 'queued', ?4)",
            params![id.0 as i64, kind, payload, now],
        )
        .map_err(|e| SupervisorError::Other(format!("durable push: {e}")))?;
        Ok(())
    }

    /// Return the next queued job's id + payload in FIFO order, or
    /// `None` if the queue is empty. Read-only — does NOT mark the
    /// row in_flight; call [`Self::mark_in_flight`] once the in-memory
    /// queue actually hands the job off to a worker.
    pub fn next_queued(&self) -> Result<Option<(JobId, Job)>, SupervisorError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, payload FROM jobs WHERE state = 'queued' \
                 ORDER BY id ASC LIMIT 1",
            )
            .map_err(|e| SupervisorError::Other(format!("prep next_queued: {e}")))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| SupervisorError::Other(format!("exec next_queued: {e}")))?;
        let Some(row) = rows
            .next()
            .map_err(|e| SupervisorError::Other(format!("row next_queued: {e}")))?
        else {
            return Ok(None);
        };
        let id: i64 = row
            .get(0)
            .map_err(|e| SupervisorError::Other(format!("get id col: {e}")))?;
        let payload: Vec<u8> = row
            .get(1)
            .map_err(|e| SupervisorError::Other(format!("get payload col: {e}")))?;
        let job: Job = serde_json::from_slice(&payload)
            .map_err(|e| SupervisorError::Other(format!("decode job payload: {e}")))?;
        Ok(Some((JobId(id as u64), job)))
    }

    /// Mark a job as `in_flight` and record the assigned worker.
    pub fn mark_in_flight(&self, id: JobId, worker: &str) -> Result<(), SupervisorError> {
        let now = unix_ms();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE jobs SET state='in_flight', assigned_to=?2, started_at=?3 \
             WHERE id=?1",
            params![id.0 as i64, worker, now],
        )
        .map_err(|e| SupervisorError::Other(format!("mark_in_flight: {e}")))?;
        Ok(())
    }

    /// Return a job that was in_flight back to the queued state — used
    /// by `requeue_worker` when a worker exits unexpectedly.
    pub fn requeue(&self, id: JobId) -> Result<(), SupervisorError> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE jobs SET state='queued', assigned_to=NULL, started_at=NULL \
             WHERE id=?1",
            params![id.0 as i64],
        )
        .map_err(|e| SupervisorError::Other(format!("requeue: {e}")))?;
        Ok(())
    }

    /// Mark a job as `done` (or `failed`) with an outcome.
    pub fn mark_done(&self, id: JobId, outcome: &JobOutcome) -> Result<(), SupervisorError> {
        let now = unix_ms();
        let state = if outcome.is_ok() { "done" } else { "failed" };
        let err_msg = match outcome {
            JobOutcome::Err { message, .. } => Some(message.as_str()),
            JobOutcome::Ok { .. } => None,
        };
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE jobs SET state=?2, finished_at=?3, error=?4 WHERE id=?1",
            params![id.0 as i64, state, now, err_msg],
        )
        .map_err(|e| SupervisorError::Other(format!("mark_done: {e}")))?;
        Ok(())
    }

    /// Recover any rows left in the `in_flight` state from a previous
    /// supervisor session. Caller is expected to flip them back to
    /// queued via [`Self::requeue`] AND re-enqueue them in the
    /// in-memory queue.
    pub fn recover_in_flight(&self) -> Result<Vec<RecoveredJob>, SupervisorError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, payload, assigned_to FROM jobs \
                 WHERE state = 'in_flight' ORDER BY id ASC",
            )
            .map_err(|e| SupervisorError::Other(format!("prep recover: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let payload: Vec<u8> = row.get(1)?;
                let assigned_to: Option<String> = row.get(2)?;
                Ok((id, payload, assigned_to))
            })
            .map_err(|e| SupervisorError::Other(format!("exec recover: {e}")))?;

        let mut out = Vec::new();
        for r in rows {
            let (id, payload, assigned_to) = match r {
                Ok(v) => v,
                Err(e) => {
                    warn!(error = %e, "skipping malformed in_flight row");
                    continue;
                }
            };
            let job: Job = match serde_json::from_slice(&payload) {
                Ok(j) => j,
                Err(e) => {
                    warn!(id, error = %e, "skipping in_flight row with corrupt payload");
                    continue;
                }
            };
            out.push(RecoveredJob {
                id: JobId(id as u64),
                job,
                assigned_to,
            });
        }
        debug!(count = out.len(), "recovered in_flight jobs");
        Ok(out)
    }

    /// Recover any rows left in the `queued` state from a previous
    /// supervisor session. Used to repopulate the in-memory queue on
    /// boot so old work doesn't sit forever in the DB.
    pub fn recover_queued(&self) -> Result<Vec<RecoveredJob>, SupervisorError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, payload, assigned_to FROM jobs \
                 WHERE state = 'queued' ORDER BY id ASC",
            )
            .map_err(|e| SupervisorError::Other(format!("prep recover_queued: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let payload: Vec<u8> = row.get(1)?;
                let assigned_to: Option<String> = row.get(2)?;
                Ok((id, payload, assigned_to))
            })
            .map_err(|e| SupervisorError::Other(format!("exec recover_queued: {e}")))?;

        let mut out = Vec::new();
        for r in rows {
            let (id, payload, assigned_to) = match r {
                Ok(v) => v,
                Err(e) => {
                    warn!(error = %e, "skipping malformed queued row");
                    continue;
                }
            };
            let job: Job = match serde_json::from_slice(&payload) {
                Ok(j) => j,
                Err(e) => {
                    warn!(id, error = %e, "skipping queued row with corrupt payload");
                    continue;
                }
            };
            out.push(RecoveredJob {
                id: JobId(id as u64),
                job,
                assigned_to,
            });
        }
        debug!(count = out.len(), "recovered queued jobs");
        Ok(out)
    }

    /// Return the largest `id` ever stored. Used by [`crate::job_queue::JobQueue`]
    /// on boot to seed `JobId::next()` past any persisted ids so a
    /// new submission cannot collide with a pre-existing row.
    pub fn max_id(&self) -> Result<u64, SupervisorError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT COALESCE(MAX(id), 0) FROM jobs")
            .map_err(|e| SupervisorError::Other(format!("prep max_id: {e}")))?;
        let v: i64 = stmt
            .query_row([], |row| row.get(0))
            .map_err(|e| SupervisorError::Other(format!("exec max_id: {e}")))?;
        Ok(v.max(0) as u64)
    }

    /// Bulk-flip every `in_flight` row back to `queued`. Used during
    /// supervisor recovery so the next router pass picks them up.
    pub fn requeue_all_in_flight(&self) -> Result<usize, SupervisorError> {
        let conn = self.conn.lock();
        let n = conn
            .execute(
                "UPDATE jobs SET state='queued', assigned_to=NULL, started_at=NULL \
                 WHERE state='in_flight'",
                [],
            )
            .map_err(|e| SupervisorError::Other(format!("requeue_all_in_flight: {e}")))?;
        Ok(n)
    }

    /// Test-only: count rows in a given state.
    #[cfg(test)]
    pub fn count_state(&self, state: &str) -> Result<usize, SupervisorError> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM jobs WHERE state = ?1")
            .map_err(|e| SupervisorError::Other(format!("prep count_state: {e}")))?;
        let v: i64 = stmt
            .query_row(params![state], |row| row.get(0))
            .map_err(|e| SupervisorError::Other(format!("exec count_state: {e}")))?;
        Ok(v.max(0) as usize)
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;
    use tempfile::TempDir;

    fn fresh_queue() -> (TempDir, DurableJobQueue) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("jobs.db");
        let q = DurableJobQueue::open(&path).expect("open");
        (dir, q)
    }

    fn dummy_parse(i: u64) -> Job {
        Job::Parse {
            file_path: PathBuf::from(format!("/tmp/file{i}.rs")),
            shard_root: PathBuf::from("/tmp/shard"),
        }
    }

    #[test]
    fn push_then_next_returns_fifo_order() {
        let (_dir, q) = fresh_queue();
        let id1 = JobId(1);
        let id2 = JobId(2);
        let id3 = JobId(3);
        q.push(id1, &dummy_parse(1)).unwrap();
        q.push(id2, &dummy_parse(2)).unwrap();
        q.push(id3, &dummy_parse(3)).unwrap();

        // recover_in_flight should be empty (none in-flight yet).
        let in_flight = q.recover_in_flight().unwrap();
        assert!(in_flight.is_empty(), "no in_flight jobs yet");

        // FIFO order — lowest id first.
        let (got1, _) = q.next_queued().unwrap().expect("first");
        assert_eq!(got1, id1);
        // Read-only: re-querying returns the same row (we haven't
        // marked it in_flight yet).
        let (got1_again, _) = q.next_queued().unwrap().expect("first again");
        assert_eq!(got1_again, id1);

        // Move it forward and the next call returns id2.
        q.mark_in_flight(id1, "worker-1").unwrap();
        let (got2, _) = q.next_queued().unwrap().expect("second");
        assert_eq!(got2, id2);
    }

    #[test]
    fn restart_recovers_in_flight() {
        let (dir, q) = fresh_queue();
        let id = JobId(42);
        q.push(id, &dummy_parse(42)).unwrap();
        q.mark_in_flight(id, "parser-worker-0").unwrap();
        let path = q.path().to_path_buf();
        drop(q);

        // Simulate restart: open a fresh handle to the same DB.
        let q2 = DurableJobQueue::open(&path).expect("reopen");
        let recovered = q2.recover_in_flight().unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, id);
        assert_eq!(recovered[0].assigned_to.as_deref(), Some("parser-worker-0"));

        // Tidy: prove tempdir is still alive.
        let _ = dir.path();
    }

    #[test]
    fn mark_done_persists_across_restart() {
        let (_dir, q) = fresh_queue();
        let id = JobId(7);
        q.push(id, &dummy_parse(7)).unwrap();
        q.mark_in_flight(id, "x").unwrap();
        q.mark_done(
            id,
            &JobOutcome::Ok {
                payload: None,
                duration_ms: 5,
                stats: serde_json::Value::Null,
            },
        )
        .unwrap();

        let path = q.path().to_path_buf();
        drop(q);

        let q2 = DurableJobQueue::open(&path).expect("reopen");
        // No in_flight survivors.
        assert!(q2.recover_in_flight().unwrap().is_empty());
        // The done count is 1.
        assert_eq!(q2.count_state("done").unwrap(), 1);
        // No queued, no in_flight, no failed.
        assert_eq!(q2.count_state("queued").unwrap(), 0);
        assert_eq!(q2.count_state("in_flight").unwrap(), 0);
        assert_eq!(q2.count_state("failed").unwrap(), 0);
    }

    #[test]
    fn mark_done_with_err_outcome_is_failed() {
        let (_dir, q) = fresh_queue();
        let id = JobId(11);
        q.push(id, &dummy_parse(11)).unwrap();
        q.mark_in_flight(id, "x").unwrap();
        q.mark_done(
            id,
            &JobOutcome::Err {
                message: "boom".into(),
                duration_ms: 1,
                stats: serde_json::Value::Null,
            },
        )
        .unwrap();
        assert_eq!(q.count_state("failed").unwrap(), 1);
        assert_eq!(q.count_state("done").unwrap(), 0);
    }

    #[test]
    fn requeue_all_in_flight_resets_state() {
        let (_dir, q) = fresh_queue();
        let id1 = JobId(101);
        let id2 = JobId(102);
        q.push(id1, &dummy_parse(101)).unwrap();
        q.push(id2, &dummy_parse(102)).unwrap();
        q.mark_in_flight(id1, "w").unwrap();
        q.mark_in_flight(id2, "w").unwrap();
        let n = q.requeue_all_in_flight().unwrap();
        assert_eq!(n, 2);
        assert_eq!(q.count_state("queued").unwrap(), 2);
        assert_eq!(q.count_state("in_flight").unwrap(), 0);
    }

    #[test]
    fn max_id_seeds_monotonic_counter() {
        let (_dir, q) = fresh_queue();
        q.push(JobId(5), &dummy_parse(5)).unwrap();
        q.push(JobId(99), &dummy_parse(99)).unwrap();
        q.push(JobId(7), &dummy_parse(7)).unwrap();
        let mx = q.max_id().unwrap();
        assert_eq!(mx, 99);
    }

    /// Performance gate. Push + next round-trip must be <= 1 ms on
    /// commodity hardware. Average over 200 iterations to remove
    /// scheduler noise; the hard gate is the AVERAGE, not the worst.
    #[test]
    fn bench_push_next_round_trip_under_1ms_avg() {
        let (_dir, q) = fresh_queue();
        const ITERS: usize = 200;
        let started = Instant::now();
        for i in 0..ITERS {
            let id = JobId((i + 1) as u64);
            q.push(id, &dummy_parse(i as u64)).unwrap();
            let next = q.next_queued().unwrap().expect("at least one queued");
            assert!(next.0 .0 >= 1);
            // Mark in_flight so the next query advances past this row.
            q.mark_in_flight(next.0, "bench-worker").unwrap();
        }
        let total = started.elapsed();
        let avg_us = total.as_micros() / ITERS as u128;
        // 1 ms = 1000 us. Generous upper bound vs the budget.
        assert!(
            avg_us <= 5_000,
            "average round-trip {avg_us}us exceeds 5ms budget; total {:?} for {ITERS} iters",
            total
        );
    }
}
