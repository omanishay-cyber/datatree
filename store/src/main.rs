//! `mneme-store` binary — long-running store-worker process supervised
//! by `mneme-supervisor`. Listens on the supervisor IPC socket and
//! serves the 7-sub-layer Database Operations Layer for every other
//! mneme worker.

use std::sync::Arc;

use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use common::paths::PathManager;
use mneme_store::Store;

#[derive(Debug, Parser)]
#[command(name = "mneme-store", about = "Mneme storage daemon")]
struct Cli {
    /// Override the mneme home directory (default: ~/.mneme).
    #[arg(long, env = "MNEME_HOME")]
    home: Option<std::path::PathBuf>,

    /// Disable IPC and run as a one-shot health probe (exit 0 / 1).
    #[arg(long)]
    health_check: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("MNEME_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let cli = Cli::parse();

    let paths = match cli.home {
        Some(p) => PathManager::with_root(p),
        None => PathManager::default_root(),
    };
    info!(home = %paths.root().display(), "mneme-store booting");

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

    // Trap SIGINT/SIGTERM AND the IPC `Shutdown` request for graceful
    // shutdown (WIDE-010). The IPC handler triggers the bound oneshot
    // instead of calling `std::process::exit` mid-handler.
    let shutdown_signal = store.bind_shutdown();
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        res = mneme_store::ipc::run_listener(store.clone()) => {
            if let Err(e) = res {
                warn!(error = ?e, "ipc listener exited");
            }
        }
        _ = ctrl_c => {
            info!("ctrl_c received, shutting down");
        }
        _ = shutdown_signal => {
            info!("ipc Shutdown request received, shutting down");
        }
    }

    info!("mneme-store shutdown complete");
    Ok(())
}
