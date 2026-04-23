//! `datatree-store` binary — long-running store-worker process supervised
//! by `datatree-supervisor`. Listens on the supervisor IPC socket and
//! serves the 7-sub-layer Database Operations Layer for every other
//! datatree worker.

use std::sync::Arc;

use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use datatree_common::paths::PathManager;
use datatree_store::Store;

#[derive(Debug, Parser)]
#[command(name = "datatree-store", about = "Datatree storage daemon")]
struct Cli {
    /// Override the datatree home directory (default: ~/.datatree).
    #[arg(long, env = "DATATREE_HOME")]
    home: Option<std::path::PathBuf>,

    /// Disable IPC and run as a one-shot health probe (exit 0 / 1).
    #[arg(long)]
    health_check: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("DATATREE_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let cli = Cli::parse();

    let paths = match cli.home {
        Some(p) => PathManager::with_root(p),
        None => PathManager::default_root(),
    };
    info!(home = %paths.root().display(), "datatree-store booting");

    if cli.health_check {
        let exists = paths.meta_db().exists();
        if exists {
            println!("ok");
            std::process::exit(0);
        } else {
            eprintln!("meta.db missing at {}", paths.meta_db().display());
            std::process::exit(1);
        }
    }

    let store = Arc::new(Store::new(paths));

    // Trap SIGINT/SIGTERM for graceful shutdown.
    let shutdown = tokio::signal::ctrl_c();

    tokio::select! {
        res = datatree_store::ipc::run_listener(store.clone()) => {
            if let Err(e) = res {
                warn!(error = ?e, "ipc listener exited");
            }
        }
        _ = shutdown => {
            info!("ctrl_c received, shutting down");
        }
    }

    Ok(())
}
