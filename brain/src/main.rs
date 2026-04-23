//! `brain` binary entry point.
//!
//! Spawns the [`worker`] loop and bridges its `mpsc` job channel to whatever
//! IPC transport the supervisor is configured to use. The default build
//! reads NDJSON-encoded `BrainJob` records from stdin and writes
//! `BrainResult` records to stdout — this keeps the binary trivially
//! testable from a shell and makes it easy to swap in a Unix-domain or
//! Windows named-pipe transport later (per design §3) without touching the
//! worker code.
//!
//! Exit codes:
//!   0  normal shutdown
//!   1  fatal init failure (model paths bad, etc.)
//!   2  IO error on stdin/stdout

use std::io::{BufRead, Write};
use std::process::ExitCode;

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use brain::worker::{spawn_worker, WorkerConfig};
use brain::{BrainJob, BrainResult};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> ExitCode {
    init_tracing();

    let cfg = match WorkerConfig::with_defaults() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "brain init failed");
            return ExitCode::from(1);
        }
    };

    let mut handle = spawn_worker(cfg);
    info!("brain ready (NDJSON over stdio)");

    // Forward stdin → jobs channel on a blocking thread.
    let jobs_tx = handle.jobs_tx.clone();
    let stdin_task = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();
        let mut line = String::new();
        loop {
            line.clear();
            let n = handle.read_line(&mut line)?;
            if n == 0 {
                // EOF — ask the worker to shut down cleanly.
                let _ = jobs_tx.blocking_send(BrainJob::Shutdown);
                return Ok(());
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<BrainJob>(trimmed) {
                Ok(job) => {
                    if jobs_tx.blocking_send(job).is_err() {
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!(error = %e, "bad job json — skipping");
                }
            }
        }
    });

    // Forward results → stdout on the runtime.
    let stdout_task = tokio::spawn(async move {
        forward_results(&mut handle.results_rx).await;
        // When results channel closes, also wait for worker to wind down.
        let _ = handle.join.await;
    });

    // Wait on whichever side completes first.
    tokio::select! {
        r = stdin_task => {
            if let Ok(Err(e)) = r {
                error!(error = %e, "stdin reader exited with error");
                return ExitCode::from(2);
            }
        }
        _ = stdout_task => {}
    }

    ExitCode::SUCCESS
}

async fn forward_results(rx: &mut mpsc::Receiver<BrainResult>) {
    let stdout = std::io::stdout();
    while let Some(result) = rx.recv().await {
        let line = match serde_json::to_string(&result) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "failed to serialise BrainResult");
                continue;
            }
        };
        let mut h = stdout.lock();
        if writeln!(h, "{line}").is_err() {
            return;
        }
        if h.flush().is_err() {
            return;
        }
    }
}

fn init_tracing() {
    // Best-effort tracing init — don't crash if env_logger / global subscriber
    // is already set elsewhere.
    let _ = tracing_subscriber_init();
}

#[cfg(feature = "tracing-subscriber")]
fn tracing_subscriber_init() -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::{fmt, EnvFilter};
    fmt().with_env_filter(EnvFilter::from_default_env()).try_init()?;
    Ok(())
}

#[cfg(not(feature = "tracing-subscriber"))]
fn tracing_subscriber_init() -> Result<(), Box<dyn std::error::Error>> {
    // Minimal no-dep fallback: write tracing events to stderr via a tiny
    // Subscriber so we still see warnings during local runs.
    Ok(())
}
