//! Mneme scan-worker binary entry point.
//!
//! Spawns a pool of [`ScanWorker`]s (default = `num_cpus * 2`), wires
//! them to a shared MPSC scan-job channel and a shared MPSC results
//! channel, attaches a [`StoreIpcBatcher`] that forwards findings to the
//! store-worker, and waits for SIGINT / Ctrl-C to drain.
//!
//! The actual cross-process plumbing (named pipe / Unix socket framing
//! between this binary and the store-worker) is owned by the supervisor
//! and store crates; this binary exposes the channels via stdin JSON
//! lines for now so the supervisor can pipe jobs in and receive batches
//! on stdout. That keeps this crate self-contained and IPC-transport
//! agnostic.

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use common::jobs::{JobId, JobOutcome};
use common::worker_ipc;
use mneme_scanners::{
    job::{ScanJob, ScanResult},
    registry::{RegistryConfig, ScannerRegistry},
    store_ipc::{BatcherConfig, FindingsBatch, StoreIpcBatcher},
    worker::ScanWorker,
};

/// Channel capacity for both jobs and results.
const CHANNEL_CAP: usize = 1024;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    init_tracing();

    let worker_count = std::env::var("MNEME_SCAN_WORKERS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| (num_cpus_or_default() * 2).max(2));

    tracing::info!(workers = worker_count, "scan-worker pool starting");

    let registry = Arc::new(ScannerRegistry::new(RegistryConfig::default()));
    let (jobs_tx, jobs_rx) = mpsc::channel::<ScanJob>(CHANNEL_CAP);
    let (results_tx, results_rx) = mpsc::channel::<ScanResult>(CHANNEL_CAP);
    let (batches_tx, mut batches_rx) = mpsc::channel::<FindingsBatch>(CHANNEL_CAP);

    // Fan-out tap — every ScanResult flowing to the batcher ALSO goes
    // here so we can fire `WorkerCompleteJob` IPC notifications without
    // forking the worker loop logic.
    let (ipc_tap_tx, mut ipc_tap_rx) = mpsc::channel::<(u64, u64, bool, usize, usize)>(CHANNEL_CAP);

    // Fan out workers, all sharing the same jobs receiver.
    let jobs_rx = Arc::new(tokio::sync::Mutex::new(jobs_rx));
    let mut worker_handles = Vec::with_capacity(worker_count);
    for id in 0..worker_count {
        let registry = registry.clone();
        let results = results_tx.clone();
        let jobs = jobs_rx.clone();
        let ipc_tap = ipc_tap_tx.clone();
        worker_handles.push(tokio::spawn(async move {
            let worker = ScanWorker::new(registry, id as u32);
            // Each worker pops jobs from a shared mutex-protected receiver
            // (single channel, multiple consumers).
            loop {
                let job = {
                    let mut guard = jobs.lock().await;
                    guard.recv().await
                };
                let Some(job) = job else { break };
                let res = worker.run_one(job).await;
                // Tap: best-effort IPC telemetry push. A full channel
                // just drops the tap — the primary batcher path still
                // completes.
                let ok = res.failed_scanners.is_empty();
                let _ = ipc_tap.try_send((
                    res.job_id,
                    res.scan_duration_ms,
                    ok,
                    res.findings.len(),
                    res.failed_scanners.len(),
                ));
                if results.send(res).await.is_err() {
                    break;
                }
            }
        }));
    }
    drop(results_tx);
    drop(ipc_tap_tx);

    // IPC tap consumer — emits one `WorkerCompleteJob` per scan
    // completion. Additive alongside the existing batched stdout path.
    let ipc_tap_handle = tokio::spawn(async move {
        while let Some((job_id, duration_ms, ok, findings_n, failed_n)) = ipc_tap_rx.recv().await {
            if job_id == 0 {
                continue;
            }
            let outcome = if ok {
                JobOutcome::Ok {
                    payload: None,
                    duration_ms,
                    stats: serde_json::json!({
                        "findings": findings_n,
                        "failed_scanners": failed_n,
                    }),
                }
            } else {
                JobOutcome::Err {
                    message: format!("{failed_n} scanner(s) failed"),
                    duration_ms,
                    stats: serde_json::json!({
                        "findings": findings_n,
                        "failed_scanners": failed_n,
                    }),
                }
            };
            if let Err(e) = worker_ipc::report_complete(JobId(job_id), outcome).await {
                tracing::debug!(error = %e, job_id, "scanner worker_complete_job ipc send skipped");
            }
        }
    });

    // Spawn the batcher.
    let batcher_handle = tokio::spawn(async move {
        let batcher = StoreIpcBatcher::new(BatcherConfig::default());
        if let Err(e) = batcher.run(results_rx, batches_tx).await {
            tracing::error!(error = %e, "batcher exited with error");
        }
    });

    // Forward batches to stdout as length-prefixed JSON.
    let stdout_handle = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(batch) = batches_rx.recv().await {
            let bytes = match serde_json::to_vec(&batch) {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(error = %e, "failed to serialize batch");
                    continue;
                }
            };
            let len = (bytes.len() as u32).to_be_bytes();
            if stdout.write_all(&len).await.is_err() {
                break;
            }
            if stdout.write_all(&bytes).await.is_err() {
                break;
            }
            let _ = stdout.flush().await;
        }
    });

    // Read jobs from stdin (one JSON object per line) and push into the
    // jobs channel.
    let stdin_handle = {
        let jobs_tx = jobs_tx.clone();
        tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let mut reader = BufReader::new(stdin).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<StdinJob>(&line) {
                    Ok(stdin_job) => {
                        let job = ScanJob {
                            file_path: stdin_job.file_path.into(),
                            content: Arc::new(stdin_job.content),
                            ast_id: stdin_job.ast_id,
                            scanner_filter: stdin_job.scanner_filter,
                            job_id: stdin_job.job_id,
                        };
                        if jobs_tx.send(job).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, line = %line, "bad scan job json");
                    }
                }
            }
        })
    };
    drop(jobs_tx);

    // Wait for shutdown signal.
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("ctrl-c received, draining workers");

    // Dropping all senders will close the chain.
    let _ = stdin_handle.await;
    for h in worker_handles {
        let _ = h.await;
    }
    let _ = batcher_handle.await;
    let _ = stdout_handle.await;
    let _ = ipc_tap_handle.await;

    tracing::info!("scan-worker pool exited cleanly");
    Ok(())
}

#[derive(serde::Deserialize)]
struct StdinJob {
    job_id: u64,
    file_path: String,
    content: String,
    #[serde(default)]
    ast_id: Option<u64>,
    #[serde(default)]
    scanner_filter: Vec<String>,
}

fn num_cpus_or_default() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
}
