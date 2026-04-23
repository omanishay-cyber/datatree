//! Thin client that forwards [`ScanResult`] batches to the store-worker
//! via length-prefixed JSON over a tokio MPSC. The actual cross-process
//! transport (Unix socket / Windows named pipe) is owned by the store
//! crate; this module simply serializes batches and emits them on a
//! shared channel that the supervisor wires to the IPC writer.
//!
//! Findings are batched to amortize IPC overhead. The default batch size
//! and flush interval are tuned for sub-millisecond latency on the
//! happy path while still bundling work under load.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::error::{Result, ScannerError};
use crate::job::ScanResult;
use crate::scanner::Finding;

/// One batch envelope sent to the store-worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingsBatch {
    /// Job ids included in this batch (1..N).
    pub job_ids: Vec<u64>,
    /// Findings in stable insertion order.
    pub findings: Vec<Finding>,
    /// Names of scanners that failed for any job in this batch.
    pub failed_scanners: Vec<String>,
    /// Batch creation timestamp (ms since epoch).
    pub created_ms: u64,
}

/// Configuration for the batcher.
#[derive(Debug, Clone)]
pub struct BatcherConfig {
    /// Maximum number of findings before forced flush.
    pub max_findings: usize,
    /// Maximum number of jobs before forced flush.
    pub max_jobs: usize,
    /// Maximum wait before forced flush.
    pub max_wait: Duration,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self {
            max_findings: 256,
            max_jobs: 64,
            max_wait: Duration::from_millis(100),
        }
    }
}

/// Drains [`ScanResult`]s, batches their findings, and forwards
/// [`FindingsBatch`]es to the store-worker IPC channel.
pub struct StoreIpcBatcher {
    cfg: BatcherConfig,
}

impl StoreIpcBatcher {
    /// New batcher with the given config.
    #[must_use]
    pub fn new(cfg: BatcherConfig) -> Self {
        Self { cfg }
    }

    /// Run the batcher loop. Exits cleanly when `results` closes.
    pub async fn run(
        &self,
        mut results: mpsc::Receiver<ScanResult>,
        out: mpsc::Sender<FindingsBatch>,
    ) -> Result<()> {
        let mut batch = FindingsBatch {
            job_ids: Vec::new(),
            findings: Vec::new(),
            failed_scanners: Vec::new(),
            created_ms: now_ms(),
        };
        let mut deadline = Instant::now() + self.cfg.max_wait;

        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());
            tokio::select! {
                maybe_res = results.recv() => {
                    match maybe_res {
                        Some(res) => {
                            batch.job_ids.push(res.job_id);
                            batch.findings.extend(res.findings);
                            batch.failed_scanners.extend(res.failed_scanners);
                            if batch.findings.len() >= self.cfg.max_findings
                                || batch.job_ids.len() >= self.cfg.max_jobs
                            {
                                self.flush(&mut batch, &out).await?;
                                deadline = Instant::now() + self.cfg.max_wait;
                            }
                        }
                        None => {
                            // Channel closed — flush remaining and exit.
                            if !batch.job_ids.is_empty() {
                                self.flush(&mut batch, &out).await?;
                            }
                            return Ok(());
                        }
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    if !batch.job_ids.is_empty() {
                        self.flush(&mut batch, &out).await?;
                    }
                    deadline = Instant::now() + self.cfg.max_wait;
                }
            }
        }
    }

    async fn flush(
        &self,
        batch: &mut FindingsBatch,
        out: &mpsc::Sender<FindingsBatch>,
    ) -> Result<()> {
        let mut filled = std::mem::replace(
            batch,
            FindingsBatch {
                job_ids: Vec::new(),
                findings: Vec::new(),
                failed_scanners: Vec::new(),
                created_ms: now_ms(),
            },
        );
        filled.created_ms = now_ms();
        out.send(filled)
            .await
            .map_err(|e| ScannerError::StoreIpc(e.to_string()))
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
