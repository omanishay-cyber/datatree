//! mneme-livebus — local SSE/WebSocket fan-out daemon.
//!
//! Binds the HTTP listener to **127.0.0.1 only**. The bind address is
//! validated by [`mneme_livebus::bind_addr`] which refuses anything other
//! than a loopback interface, so a misconfigured `--host 0.0.0.0` is a hard
//! startup failure rather than a silent security regression.

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{any, get};
use axum::Router;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use mneme_livebus::bus::EventBus;
use mneme_livebus::health::{health_handler, HealthCtx, RateSampler};
use mneme_livebus::ipc_input::{default_ipc_path, run_ipc_listener};
use mneme_livebus::sse::{sse_firehose_handler, sse_handler};
use mneme_livebus::subscriber::SubscriberManager;
use mneme_livebus::ws::ws_upgrade;
use mneme_livebus::{bind_addr, DEFAULT_HOST, DEFAULT_PORT};

#[derive(Parser, Debug)]
#[command(name = "mneme-livebus", version, about)]
struct Args {
    /// Loopback host to bind. Refuses non-loopback addresses.
    #[arg(long, env = "LIVEBUS_HOST", default_value_t = DEFAULT_HOST.to_string())]
    host: String,

    /// TCP port for SSE/WebSocket/health.
    #[arg(long, env = "LIVEBUS_PORT", default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Local IPC ingest path (Unix socket or Windows named pipe).
    #[arg(long, env = "LIVEBUS_IPC_PATH")]
    ipc_path: Option<PathBuf>,

    /// Disable the IPC ingest listener (HTTP-only mode for testing).
    #[arg(long, env = "LIVEBUS_NO_IPC", default_value_t = false)]
    no_ipc: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    // Hard-fail if someone tries to bind to a routable address.
    let addr = bind_addr(&args.host, args.port)?;
    info!(%addr, "livebus binding to loopback only");

    let bus = EventBus::new();
    let mgr = SubscriberManager::new(bus.clone());
    let sampler = Arc::new(RateSampler::new());
    let health_state = HealthCtx {
        mgr: mgr.clone(),
        sampler,
    };

    // ---- HTTP router ----
    let app = Router::new()
        .route("/events", get(sse_firehose_handler))
        .route("/events/:topic", get(sse_handler))
        .route("/ws", any(ws_upgrade))
        .with_state(mgr.clone())
        .merge(
            Router::new()
                .route("/health", get(health_handler))
                .with_state(health_state),
        );

    // ---- IPC ingest task ----
    if !args.no_ipc {
        let ipc_path = args.ipc_path.unwrap_or_else(default_ipc_path);
        let mgr_ipc = mgr.clone();
        tokio::spawn(async move {
            if let Err(e) = run_ipc_listener(ipc_path.clone(), mgr_ipc).await {
                warn!(?ipc_path, error = %e, "ipc listener exited");
            }
        });
    }

    // ---- HTTP server ----
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "livebus listening (SSE /events/:topic, WS /ws, GET /health)");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("livebus shutdown complete");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env("LIVEBUS_LOG")
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init()
        .ok();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut s) = signal(SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => info!("ctrl-c received; shutting down"),
        _ = terminate => info!("SIGTERM received; shutting down"),
    }
}
