//! Async scan-worker loop. Each worker drains [`ScanJob`]s from a shared
//! tokio MPSC, runs every applicable scanner against the job, and pushes
//! a [`ScanResult`] to the store-IPC channel.
//!
//! Failure isolation: if one scanner panics or returns an error, the
//! worker logs the failure on the [`ScanResult::failed_scanners`] list
//! and proceeds with the remaining scanners.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;

use crate::error::{Result, ScannerError};
use crate::findings_writer::FindingsWriter;
use crate::job::{ScanJob, ScanResult};
use crate::registry::ScannerRegistry;
use crate::scanner::Ast;

/// One scan worker. Cloneable; the underlying registry is shared via Arc.
#[derive(Clone)]
pub struct ScanWorker {
    /// The registry of available scanners.
    pub registry: Arc<ScannerRegistry>,
    /// Stable id used for telemetry (e.g. `worker-3`).
    pub id: u32,
}

impl ScanWorker {
    /// New worker.
    #[must_use]
    pub fn new(registry: Arc<ScannerRegistry>, id: u32) -> Self {
        Self { registry, id }
    }

    /// Drain `jobs` until the channel closes. Each [`ScanResult`] is
    /// forwarded to `results`. Returns when the receiver closes.
    pub async fn run(
        &self,
        mut jobs: mpsc::Receiver<ScanJob>,
        results: mpsc::Sender<ScanResult>,
    ) -> Result<()> {
        tracing::info!(worker_id = self.id, "scan worker starting");
        while let Some(job) = jobs.recv().await {
            let res = self.run_one(job).await;
            if results.send(res).await.is_err() {
                tracing::warn!(worker_id = self.id, "results channel closed; exiting");
                return Err(ScannerError::JobChannelClosed);
            }
        }
        tracing::info!(worker_id = self.id, "scan worker stopped");
        Ok(())
    }

    /// Run every applicable scanner on a single job. Never panics — each
    /// scanner is wrapped in [`std::panic::catch_unwind`] so a regex bug
    /// or stack overflow can't take the worker down.
    ///
    /// Async wrapper around [`Self::run_one_blocking`] for callers that
    /// need a `Future`. The body is purely synchronous; use
    /// `run_one_blocking` directly + `spawn_blocking` if you need real
    /// preemption (B-019 / B-027 — `tokio::time::timeout` cannot
    /// interrupt sync code, so the orchestrator wraps `run_one_blocking`
    /// in `tokio::task::spawn_blocking` for the per-file timeout).
    pub async fn run_one(&self, job: ScanJob) -> ScanResult {
        self.run_one_blocking(job)
    }

    /// Synchronous body of [`Self::run_one`]. Extracted so the per-file
    /// timeout in `scanners/src/main.rs::run_orchestrator_mode` can wrap
    /// it in `tokio::task::spawn_blocking` and have `tokio::time::timeout`
    /// actually preempt CPU-bound scanner code (B-027 / 2026-04-30 audit
    /// follow-up to B-019: tokio cannot interrupt sync futures, only
    /// blocking-pool futures).
    pub fn run_one_blocking(&self, job: ScanJob) -> ScanResult {
        let started = Instant::now();
        let mut findings = Vec::new();
        let mut failed_scanners = Vec::new();

        let applicable = self.registry.applicable_scanners(&job.file_path);
        let ast = job.ast_id.map(Ast::new);

        for s in applicable {
            if !job.allows_scanner(s.name()) {
                continue;
            }
            // Catch panics so one buggy scanner doesn't kill the worker.
            let name = s.name().to_string();
            let file_path = job.file_path.clone();
            let content = job.content.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                s.scan(&file_path, content.as_str(), ast)
            }));
            match result {
                Ok(mut v) => findings.append(&mut v),
                Err(panic) => {
                    let msg = panic_message(panic);
                    tracing::warn!(
                        worker_id = self.id,
                        scanner = %name,
                        file = %job.file_path.display(),
                        error = %msg,
                        "scanner panicked; isolating",
                    );
                    failed_scanners.push(name);
                }
            }
        }

        ScanResult {
            job_id: job.job_id,
            findings,
            scan_duration_ms: started.elapsed().as_millis() as u64,
            failed_scanners,
        }
    }

    /// Run a single job AND persist its findings to `findings_db`.
    ///
    /// Used by `mneme audit` and any other inline caller that does not
    /// want to go through the async batcher + store IPC. Returns the
    /// `ScanResult` (for telemetry) and the number of rows inserted.
    ///
    /// The findings.db connection is opened fresh per call and dropped on
    /// return, preserving the per-shard single-writer invariant.
    pub async fn scan_and_persist(
        &self,
        job: ScanJob,
        findings_db: &Path,
    ) -> Result<(ScanResult, usize)> {
        let result = self.run_one(job).await;
        let mut writer = FindingsWriter::open(findings_db)?;
        let n = writer.write_findings(&result.findings)?;
        Ok((result, n))
    }
}

/// Best-effort extraction of a panic payload as a printable string.
fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = panic.downcast_ref::<String>() {
        return s.clone();
    }
    "non-string panic payload".to_string()
}
