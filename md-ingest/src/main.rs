//! v0.3 md-ingest worker: reads JSON-encoded ingest jobs on stdin and
//! emits `WorkerCompleteJob` IPC messages back to the supervisor.
//!
//! Intentionally minimal until the full markdown frontmatter / heading /
//! wikilink extractor lands (see REMAINING_WORK.md tier-1 item 2). The
//! contract this crate must honour RIGHT NOW is:
//!
//!   * consume `{job_id, md_file}` NDJSON lines on stdin,
//!   * read the file (best-effort),
//!   * for each job, emit a typed `WorkerCompleteJob` via
//!     `common::worker_ipc::report_complete`,
//!   * keep stdout quiet (the supervisor tails it into the log ring;
//!     one info-level line per job suffices for traceability).
//!
//! When the real ingester ships it will populate the `stats` payload
//! with per-file heading/link counts; today we report a single
//! `{"bytes": N}` stat so dashboards have something to graph.

use std::path::PathBuf;
use std::time::Instant;

use common::jobs::{JobId, JobOutcome};
use common::worker_ipc;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
struct IngestJob {
    #[serde(default)]
    job_id: u64,
    md_file: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_tracing();
    info!("mneme-md-ingest v0.3 — consuming NDJSON ingest jobs on stdin");

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("mneme-md-ingest ctrl-c; exiting");
                break;
            }
            maybe_line = lines.next_line() => {
                let Ok(Some(line)) = maybe_line else {
                    // EOF or read error — stay alive waiting for a signal
                    // so the supervisor doesn't reap us the moment stdin
                    // closes. Matches parsers/main.rs behaviour.
                    info!("mneme-md-ingest stdin closed; waiting for signal");
                    let _ = tokio::signal::ctrl_c().await;
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                handle_line(&line).await;
            }
        }
    }
    info!("mneme-md-ingest exiting");
}

async fn handle_line(line: &str) {
    let job: IngestJob = match serde_json::from_str(line) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, raw = %line, "invalid md-ingest job JSON");
            return;
        }
    };
    let started = Instant::now();
    let (outcome, bytes) = match std::fs::metadata(&job.md_file) {
        Ok(m) => {
            let b = m.len();
            info!(file = %job.md_file.display(), bytes = b, "md-ingest (stub) processed");
            (
                JobOutcome::Ok {
                    payload: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                    stats: serde_json::json!({"bytes": b}),
                },
                b,
            )
        }
        Err(e) => (
            JobOutcome::Err {
                message: format!("metadata {}: {e}", job.md_file.display()),
                duration_ms: started.elapsed().as_millis() as u64,
                stats: serde_json::Value::Null,
            },
            0,
        ),
    };

    if job.job_id != 0 {
        if let Err(e) = worker_ipc::report_complete(JobId(job.job_id), outcome).await {
            debug!(
                error = %e,
                job_id = job.job_id,
                "md-ingest worker_complete_job ipc send skipped"
            );
        }
    } else {
        let _ = bytes;
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("MNEME_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();
}
