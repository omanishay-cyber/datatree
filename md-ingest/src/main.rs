//! v0.1 stub: markdown-ingest worker placeholder.
//!
//! Real implementation in v0.2 will walk the project, hash every `.md`,
//! parse frontmatter, extract heading tree, link wikilinks, and feed
//! embeddings into `semantic.db`. For v0.1 we just stay alive so the
//! supervisor can check us off its "all children spawned" list and the
//! rest of the daemon keeps running.

use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("MNEME_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();
    tracing::info!("mneme-md-ingest v0.1 stub — idle");
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("mneme-md-ingest exiting");
}
