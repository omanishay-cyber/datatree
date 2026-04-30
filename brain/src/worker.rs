//! Async worker that owns the embedder, store, extractor and runners and
//! dispatches incoming [`BrainJob`]s onto them.
//!
//! Architecture:
//! ```text
//!   caller --(BrainJob via mpsc)--> worker --(BrainResult via mpsc)--> caller
//! ```
//!
//! The worker is single-threaded with respect to the underlying ONNX session
//! (which is itself not `Sync`), but it spawns blocking work onto Tokio's
//! blocking pool so the runtime stays responsive.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::cluster_runner::{ClusterRunner, ClusterRunnerConfig};
use crate::concept::{ConceptExtractor, ExtractInput};
use crate::embed_store::EmbedStore;
use crate::embeddings::Embedder;
use crate::error::BrainResult;
use crate::job::{BrainJob, BrainResult as JobResult};
use crate::summarize::Summarizer;

/// Public worker handle returned from [`spawn_worker`].
#[derive(Debug)]
pub struct WorkerHandle {
    pub jobs_tx: mpsc::Sender<BrainJob>,
    pub results_rx: mpsc::Receiver<JobResult>,
    pub join: JoinHandle<()>,
}

/// Construction-time options.
#[derive(Clone)]
pub struct WorkerConfig {
    pub embedder: Embedder,
    pub store: EmbedStore,
    pub extractor: ConceptExtractor,
    pub summarizer: Summarizer,
    pub cluster: ClusterRunner,
    pub channel_capacity: usize,
}

impl std::fmt::Debug for WorkerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerConfig")
            .field("channel_capacity", &self.channel_capacity)
            .finish()
    }
}

impl WorkerConfig {
    /// Build with all-default subsystems. Fails only if `EmbedStore` cannot
    /// open its cache directory.
    pub fn with_defaults() -> BrainResult<Self> {
        Ok(Self {
            embedder: Embedder::from_default_path()?,
            store: EmbedStore::open_default()?,
            extractor: ConceptExtractor::new(),
            summarizer: Summarizer::new(),
            cluster: ClusterRunner::new(ClusterRunnerConfig::default()),
            channel_capacity: 256,
        })
    }
}

/// Spawn the worker. Returns sender/receiver pair plus the join handle.
pub fn spawn_worker(cfg: WorkerConfig) -> WorkerHandle {
    let (jobs_tx, mut jobs_rx) = mpsc::channel::<BrainJob>(cfg.channel_capacity);
    let (results_tx, results_rx) = mpsc::channel::<JobResult>(cfg.channel_capacity);

    let embedder = cfg.embedder.clone();
    let store = cfg.store.clone();
    let extractor = cfg.extractor.clone();
    let summarizer = cfg.summarizer.clone();
    let cluster = cfg.cluster.clone();

    let join = tokio::spawn(async move {
        info!("brain worker started");
        while let Some(job) = jobs_rx.recv().await {
            if matches!(job, BrainJob::Shutdown) {
                info!("brain worker shutting down");
                break;
            }
            let result = handle(
                job,
                embedder.clone(),
                store.clone(),
                extractor.clone(),
                summarizer.clone(),
                cluster.clone(),
            )
            .await;
            if results_tx.send(result).await.is_err() {
                warn!("brain result receiver dropped — exiting");
                break;
            }
        }
        // Best-effort flush of the embed store on shutdown.
        if let Err(e) = store.flush() {
            warn!(error = %e, "embed store flush on shutdown failed");
        }
    });

    WorkerHandle {
        jobs_tx,
        results_rx,
        join,
    }
}

async fn handle(
    job: BrainJob,
    embedder: Embedder,
    store: EmbedStore,
    extractor: ConceptExtractor,
    summarizer: Summarizer,
    cluster: ClusterRunner,
) -> JobResult {
    let id = job.id();
    match job {
        BrainJob::Embed { id, node, text } => {
            let res = run_blocking(move || {
                let v = embedder.embed(&text)?;
                if let Some(n) = node {
                    store.upsert(n, &v)?;
                }
                Ok::<_, crate::error::BrainError>(v)
            })
            .await;
            match res {
                Ok(vector) => JobResult::Embedding { id, node, vector },
                Err(e) => JobResult::Error {
                    id,
                    message: e.to_string(),
                },
            }
        }
        BrainJob::EmbedBatch { id, items } => {
            let res = run_blocking(move || {
                let texts: Vec<&str> = items.iter().map(|(_, t)| t.as_str()).collect();
                let vectors = embedder.embed_batch(&texts)?;
                let mut out = Vec::with_capacity(items.len());
                let mut to_store: Vec<(crate::NodeId, Vec<f32>)> = Vec::new();
                for ((node, _), v) in items.iter().zip(vectors.into_iter()) {
                    if let Some(n) = node {
                        to_store.push((*n, v.clone()));
                    }
                    out.push((*node, v));
                }
                if !to_store.is_empty() {
                    store.upsert_many(&to_store)?;
                }
                Ok::<_, crate::error::BrainError>(out)
            })
            .await;
            match res {
                Ok(vectors) => JobResult::EmbeddingBatch { id, vectors },
                Err(e) => JobResult::Error {
                    id,
                    message: e.to_string(),
                },
            }
        }
        BrainJob::Cluster { id, edges, seed } => {
            let mut local_cluster = cluster.clone();
            // Override seed if caller supplied one.
            if let Some(s) = seed {
                let mut cfg = ClusterRunnerConfig::default();
                cfg.leiden.seed = s;
                local_cluster = ClusterRunner::new(cfg);
            }
            let res = run_blocking(move || local_cluster.run(&edges)).await;
            match res {
                Ok(communities) => JobResult::Clusters { id, communities },
                Err(e) => JobResult::Error {
                    id,
                    message: e.to_string(),
                },
            }
        }
        BrainJob::ExtractConcepts {
            id,
            node,
            kind,
            text,
        } => {
            let res = run_blocking(move || {
                extractor.extract(ExtractInput {
                    kind: &kind,
                    text: &text,
                })
            })
            .await;
            match res {
                Ok(concepts) => JobResult::Concepts { id, node, concepts },
                Err(e) => JobResult::Error {
                    id,
                    message: e.to_string(),
                },
            }
        }
        BrainJob::Summarize {
            id,
            node,
            signature,
            body,
        } => {
            let res = run_blocking(move || summarizer.summarize_function(&signature, &body)).await;
            match res {
                Ok(summary) => JobResult::Summary { id, node, summary },
                Err(e) => JobResult::Error {
                    id,
                    message: e.to_string(),
                },
            }
        }
        BrainJob::Shutdown => JobResult::Error {
            id,
            message: "shutdown is not a job".into(),
        },
    }
}

/// Convenience: run CPU-bound work on the blocking pool, propagating panics
/// as errors instead of crashing the worker.
async fn run_blocking<F, T>(f: F) -> Result<T, crate::error::BrainError>
where
    F: FnOnce() -> Result<T, crate::error::BrainError> + Send + 'static,
    T: Send + 'static,
{
    let _ = Arc::new(()); // anchor for clippy: fn signature stable across builds
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "blocking task panicked");
            Err(crate::error::BrainError::Other(anyhow::anyhow!(
                "blocking task panicked: {e}"
            )))
        }
    }
}
