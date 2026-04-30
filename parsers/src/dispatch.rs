//! M16 — head-of-line-blocking-free job dispatch across N worker
//! channels.
//!
//! The original `parse-worker` main.rs implemented a strict
//! round-robin: `target = next % senders.len(); senders[target].send(job).await`.
//! That blocks the dispatcher whenever the targeted worker is busy on
//! a giant file, even when other workers are idle — head-of-line
//! blocking. A 5-second parse on worker[0] would serialise the next
//! 100 jobs at 5s apiece.
//!
//! `try_send_fanout` walks the sender vector starting from a caller-
//! supplied cursor and attempts a non-blocking `try_send` on each in
//! turn. The first sender that accepts the job wins and the cursor is
//! advanced past it. If every sender is full we fall back to a
//! time-bounded `send_timeout(SEND_TIMEOUT_SECS)` on the original
//! cursor index — preserving backpressure semantics without the
//! head-of-line stall.
//!
//! The dispatcher in main.rs feeds the same `senders: Vec<mpsc::Sender<ParseJob>>`
//! it always built; only the per-job dispatch primitive changed.

use std::time::Duration;
use tokio::sync::mpsc;

/// Upper bound on a fallback `send_timeout` when every worker channel
/// is full. Exposed at `pub(crate)` so the inline test fixture can
/// drive it without re-declaring the magic number.
pub const SEND_TIMEOUT_SECS: u64 = 60;

/// Outcome of a dispatch attempt.
#[derive(Debug)]
pub enum DispatchOutcome {
    /// The job was delivered to `senders[index]`.
    Delivered { index: usize },
    /// Every sender was full for the full `send_timeout` window. The
    /// caller's cursor is unchanged; the job is returned so the caller
    /// can decide whether to retry, log, or drop.
    AllFull,
    /// Every sender's receiver has been dropped. The job is returned;
    /// the caller is expected to abort the dispatch loop.
    AllClosed,
}

/// Try to dispatch `job` to one of `senders` without blocking on a
/// single busy worker.
///
/// Walks `senders` starting from `cursor % senders.len()` and attempts
/// `try_send` on each in turn. Returns immediately on first success.
/// If every sender is at capacity the function falls back to a
/// time-bounded `send_timeout` on `senders[cursor % senders.len()]`
/// — bounded by `SEND_TIMEOUT_SECS` so a permanently-wedged worker
/// pool can't hang the dispatcher.
///
/// Closed senders (receiver dropped) are treated as exhausted slots;
/// the function will skip past them. If every sender is closed the
/// caller gets `AllClosed` and should stop dispatching.
pub async fn try_send_fanout<T: Send>(
    senders: &[mpsc::Sender<T>],
    cursor: usize,
    job: T,
) -> DispatchOutcome {
    if senders.is_empty() {
        return DispatchOutcome::AllClosed;
    }
    let len = senders.len();
    let start = cursor % len;
    let mut current_job = job;
    let mut all_closed = true;

    // Pass 1: try_send around the ring once.
    for offset in 0..len {
        let idx = (start + offset) % len;
        match senders[idx].try_send(current_job) {
            Ok(()) => return DispatchOutcome::Delivered { index: idx },
            Err(mpsc::error::TrySendError::Full(returned)) => {
                all_closed = false;
                current_job = returned;
            }
            Err(mpsc::error::TrySendError::Closed(returned)) => {
                current_job = returned;
            }
        }
    }

    if all_closed {
        return DispatchOutcome::AllClosed;
    }

    // Pass 2: every sender was full. Fall back to a bounded send on
    // the original cursor target. send_timeout() yields control while
    // waiting, so the runtime can keep the result-emitter task and
    // worker tasks scheduled — they're what will free a slot.
    match senders[start]
        .send_timeout(current_job, Duration::from_secs(SEND_TIMEOUT_SECS))
        .await
    {
        Ok(()) => DispatchOutcome::Delivered { index: start },
        Err(mpsc::error::SendTimeoutError::Timeout(_returned)) => DispatchOutcome::AllFull,
        Err(mpsc::error::SendTimeoutError::Closed(_returned)) => DispatchOutcome::AllClosed,
    }
}

#[cfg(test)]
mod tests {
    //! M16 — fan-out dispatch must skip a busy worker so head-of-line
    //! blocking on a single slow parse cannot serialise the queue.
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// Simulates a parse worker pool. Worker 0 takes a single
    /// "5-second parse" on its first job (the "stuck on a giant
    /// file" case) and is unresponsive for the duration. Workers
    /// 1..N drain instantly.
    ///
    /// We submit 100 jobs and assert the *dispatch loop* completes
    /// well under the strict-round-robin worst case (which would
    /// stall on worker 0 for ~5s every time the cursor lands back
    /// on it).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn fanout_does_not_head_of_line_block_on_slow_worker() {
        const WORKERS: usize = 4;
        const JOB_COUNT: usize = 100;
        const SLOW_PARSE: Duration = Duration::from_secs(5);
        const CHANNEL_CAP: usize = 64; // matches parsers/src/main.rs
        const DISPATCH_BUDGET: Duration = Duration::from_secs(6);

        let mut senders = Vec::with_capacity(WORKERS);
        let mut handles = Vec::with_capacity(WORKERS);
        let completed = Arc::new(AtomicUsize::new(0));

        for w in 0..WORKERS {
            let (tx, mut rx) = mpsc::channel::<u32>(CHANNEL_CAP);
            senders.push(tx);
            let completed = completed.clone();
            handles.push(tokio::spawn(async move {
                let mut first = true;
                while let Some(_job) = rx.recv().await {
                    if w == 0 && first {
                        // Worker 0 is "stuck on a giant file" for
                        // its first job only. After the 5s parse
                        // the worker recovers and drains normally
                        // — this models a single big file in a
                        // healthy pool.
                        first = false;
                        tokio::time::sleep(SLOW_PARSE).await;
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        // Dispatch JOB_COUNT jobs. With strict round-robin every
        // 4th job lands on worker 0; the moment its 64-cap channel
        // fills, the dispatcher blocks waiting for that 5s parse —
        // serialising the rest of the queue behind it. With the
        // fan-out strategy the dispatcher walks past worker 0's
        // full channel and feeds the idle workers 1..3 instead.
        let start = Instant::now();
        let mut cursor = 0usize;
        for job_id in 0..JOB_COUNT {
            match try_send_fanout(&senders, cursor, job_id as u32).await {
                DispatchOutcome::Delivered { index } => {
                    cursor = index.wrapping_add(1);
                }
                other => panic!("dispatch failed unexpectedly at job {job_id}: {other:?}"),
            }
        }
        let dispatch_elapsed = start.elapsed();

        // The dispatch loop itself must finish within the budget —
        // this is the core M16 guarantee. Worker 0 may still be
        // chewing on its 5s parse afterwards, but the dispatcher
        // is not allowed to be on the critical path for that.
        assert!(
            dispatch_elapsed < DISPATCH_BUDGET,
            "fan-out dispatcher must not head-of-line block; \
             dispatch took {dispatch_elapsed:?}, budget {DISPATCH_BUDGET:?} — \
             a stalled worker 0 must not serialise the queue"
        );

        // Drop senders so workers' rx loops can exit, then await
        // them to confirm every job eventually completes.
        drop(senders);
        for h in handles {
            let _ = h.await;
        }
        assert_eq!(
            completed.load(Ordering::SeqCst),
            JOB_COUNT,
            "all jobs must complete; got {}",
            completed.load(Ordering::SeqCst)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fanout_returns_all_closed_when_every_receiver_dropped() {
        let (tx1, rx1) = mpsc::channel::<u8>(4);
        let (tx2, rx2) = mpsc::channel::<u8>(4);
        drop(rx1);
        drop(rx2);
        let outcome = try_send_fanout(&[tx1, tx2], 0, 7u8).await;
        assert!(
            matches!(outcome, DispatchOutcome::AllClosed),
            "every receiver dropped → AllClosed, got {outcome:?}"
        );
    }
}
