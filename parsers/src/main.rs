//! `parse-worker` binary — the long-lived process the mneme supervisor
//! spawns (one instance, internally hosting N parser workers).
//!
//! Behaviour:
//! 1. Build a [`ParserPool`] sized to `cpu_count * 4` (§21.3).
//! 2. Pre-compile every cached query (warm the cache).
//! 3. Spawn N worker tasks, each owning an MPSC receiver.
//! 4. Read JSON-encoded [`ParseJob`]s from stdin (one per line).
//! 5. Round-robin them to workers; emit JSON [`ParseResult`]s on stdout.
//!
//! In production the supervisor talks to this process over IPC framed as
//! length-prefixed bytes; the JSON-over-stdio path here is identical in
//! contract and is what the integration tests in `mneme/tests/` drive.

use mneme_parsers::{
    incremental::IncrementalParser, parser_pool::ParserPool, query_cache, worker::Worker,
    ParseJob, ParserError,
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tracing::{error, info};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let pool = Arc::new(ParserPool::with_default_size()?);
    info!(
        languages = pool.enabled_languages().len(),
        workers_per_language = pool.workers_per_language(),
        "parser pool ready"
    );

    // Warm the query cache so first-parse latency hits the <50ms target.
    if let Err(e) = query_cache::warm_up() {
        // Non-fatal: bad pattern for one grammar shouldn't kill the worker.
        error!(error = %e, "query warm-up reported issues");
    }

    let inc = Arc::new(IncrementalParser::new(pool.clone()));

    // N workers. We keep parity with the pool's per-language count: the
    // supervisor decides job dispatch by language, so worker count is the
    // upper bound on language-parallelism inside this process.
    let worker_count = (num_cpus::get() * 4).max(4);
    info!(workers = worker_count, "spawning parser workers");

    let (tx_results, mut rx_results) =
        mpsc::channel::<Result<mneme_parsers::ParseResult, ParserError>>(1024);
    let mut job_senders = Vec::with_capacity(worker_count);

    for id in 0..worker_count {
        let (tx_jobs, rx_jobs) = mpsc::channel::<ParseJob>(64);
        job_senders.push(tx_jobs);
        let worker = Worker::new(id, inc.clone(), rx_jobs, tx_results.clone());
        tokio::spawn(worker.run());
    }
    drop(tx_results); // workers each hold a clone; loop exits when all do

    // Result-emitter task: writes one JSON line per result to stdout.
    let result_task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(res) = rx_results.recv().await {
            let line = match res {
                Ok(r) => serde_json::to_string(&r)
                    .unwrap_or_else(|e| format!("{{\"error\":\"serialize: {e}\"}}")),
                Err(e) => format!("{{\"error\":\"{}\"}}", e),
            };
            if let Err(e) = stdout.write_all(line.as_bytes()).await {
                error!(error = %e, "stdout write failed");
                break;
            }
            let _ = stdout.write_all(b"\n").await;
            let _ = stdout.flush().await;
        }
    });

    // Stdin reader: parse JSON jobs and round-robin to workers.
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut next = 0usize;

    // Read jobs from stdin until EOF, then keep the process alive on a
    // SIGINT/SIGTERM watch so the supervisor's monitor doesn't reap us the
    // moment our launcher closes stdin. A worker running under the
    // supervisor with no incoming jobs is the expected steady state — not
    // an exit condition.
    tokio::spawn(async move {
        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let job: ParseJob = match serde_json::from_str::<JobWire>(&line) {
                Ok(j) => j.into_job(),
                Err(e) => {
                    error!(error = %e, raw = %line, "invalid job JSON");
                    continue;
                }
            };
            let target = next % job_senders.len();
            next = next.wrapping_add(1);
            if let Err(e) = job_senders[target].send(job).await {
                error!(error = %e, "worker queue closed; dropping job");
            }
        }
        tracing::info!("stdin closed; parse-worker entering idle mode (waiting for signals)");
    });

    // Block forever on ctrl-c. Supervisor kills us with taskkill / SIGKILL
    // during shutdown.
    let _ = tokio::signal::ctrl_c().await;
    info!("parse-worker shutting down cleanly");
    let _ = result_task.await;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_env("MNEME_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

// ---------------------------------------------------------------------------
// Wire format — keeps the `content` field as a String for ergonomics on the
// stdin side, then converts to the Arc<Vec<u8>> the worker expects.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct JobWire {
    file_path: std::path::PathBuf,
    language: mneme_parsers::Language,
    /// UTF-8 source. Binary content is rejected at the supervisor layer.
    content: String,
    #[serde(default)]
    prev_tree_id: Option<u64>,
    #[serde(default)]
    job_id: u64,
}

impl JobWire {
    fn into_job(self) -> ParseJob {
        ParseJob {
            file_path: self.file_path,
            language: self.language,
            content: Arc::new(self.content.into_bytes()),
            prev_tree_id: self.prev_tree_id,
            content_hash: None,
            job_id: self.job_id,
        }
    }
}
