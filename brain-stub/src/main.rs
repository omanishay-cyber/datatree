//! v0.1 stub: brain worker placeholder.
//!
//! The real `brain/` crate implements local ONNX embeddings + Leiden
//! clustering + Phi-3 concept extraction, but `ort = "=2.0.0-rc.4"` is
//! incompatible with rustc 1.95 (245 macro errors). Until ort is bumped
//! to rc.12+, this stub keeps the supervisor's "brain-worker" slot
//! healthy and idle so the rest of the daemon stays up.

use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("DATATREE_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();
    tracing::info!("datatree-brain v0.1 stub — embeddings disabled (ort version pending)");
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("datatree-brain exiting");
}
