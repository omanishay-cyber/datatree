//! SLA dashboard HTTP server (localhost:7777 by default).
//!
//! Exposes:
//!   - `GET /health`      — full SLA snapshot as JSON
//!   - `GET /health/live` — liveness probe (always 200 if process is up)
//!   - `GET /metrics`     — minimal Prometheus-like text format
//!
//! Bound exclusively to `127.0.0.1`. No authentication: this is a local-only
//! daemon (see §22 of the design doc).

use crate::error::SupervisorError;
use crate::manager::{ChildManager, ChildSnapshot};
use crate::watcher::WatcherStatsHandle;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Disks, System};
use tokio::sync::Notify;
use tracing::{error, info};

/// Full snapshot returned by `GET /health`.
#[derive(Debug, Clone, Serialize)]
pub struct SlaSnapshot {
    /// Wall-clock at the time of the snapshot.
    pub timestamp: DateTime<Utc>,
    /// Supervisor uptime in seconds.
    pub supervisor_uptime_s: u64,
    /// Per-child snapshots.
    pub children: Vec<ChildSnapshot>,
    /// Aggregate uptime % over the lifetime of the supervisor.
    pub overall_uptime_percent: f64,
    /// Cache hit rate (placeholder until store-worker reports back).
    pub cache_hit_rate: f64,
    /// Disk usage, free bytes, total bytes for the mneme root.
    pub disk: DiskUsage,
    /// File watcher percentiles (save-to-graph latency, ms).
    pub watcher: WatcherMetrics,
}

/// Watcher metrics surfaced on `/health` and `/metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WatcherMetrics {
    /// Total files reindexed since boot.
    pub total_reindexed: u64,
    /// Total delete events processed.
    pub total_deletes: u64,
    /// Files dropped because they matched the ignore list.
    pub total_ignored: u64,
    /// p50 latency in milliseconds.
    pub p50_ms: u64,
    /// p95 latency in milliseconds (SLO = 500ms).
    pub p95_ms: u64,
    /// p99 latency in milliseconds.
    pub p99_ms: u64,
}

/// Disk usage summary used by [`SlaSnapshot`].
#[derive(Debug, Clone, Serialize)]
pub struct DiskUsage {
    /// Total bytes of the volume holding `~/.mneme`.
    pub total_bytes: u64,
    /// Free bytes.
    pub free_bytes: u64,
    /// Used percentage (0.0 – 100.0).
    pub used_percent: f64,
}

#[derive(Clone)]
struct AppState {
    manager: Arc<ChildManager>,
    started: Instant,
    watcher_stats: WatcherStatsHandle,
}

/// HTTP server hosting the SLA dashboard.
pub struct HealthServer {
    manager: Arc<ChildManager>,
    port: u16,
    watcher_stats: WatcherStatsHandle,
}

impl HealthServer {
    /// Construct a new health server.
    pub fn new(manager: Arc<ChildManager>, port: u16) -> Self {
        Self {
            manager,
            port,
            watcher_stats: WatcherStatsHandle::new(),
        }
    }

    /// Construct a new health server with a shared watcher-stats handle.
    /// Call this flavor when the supervisor embeds a watcher so the SLA
    /// dashboard can surface save-to-graph latency.
    pub fn with_watcher_stats(
        manager: Arc<ChildManager>,
        port: u16,
        watcher_stats: WatcherStatsHandle,
    ) -> Self {
        Self {
            manager,
            port,
            watcher_stats,
        }
    }

    /// Run the server until `shutdown.notified()`.
    pub async fn serve(self, shutdown: Arc<Notify>) {
        let state = AppState {
            manager: self.manager,
            started: Instant::now(),
            watcher_stats: self.watcher_stats,
        };

        let app = Router::new()
            .route("/health", get(health_full))
            .route("/health/live", get(health_live))
            .route("/metrics", get(metrics))
            .with_state(state);

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port);
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!(addr = %addr, error = %e, "health server bind failed");
                return;
            }
        };
        info!(addr = %addr, "health server listening");

        let shutdown_signal = async move {
            shutdown.notified().await;
        };

        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            error!(error = %e, "health server error");
        }
    }
}

async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn health_full(State(state): State<AppState>) -> impl IntoResponse {
    match build_snapshot(&state).await {
        Ok(s) => (StatusCode::OK, Json(s)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let snap = match build_snapshot(&state).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let mut body = String::new();
    body.push_str(&format!(
        "# TYPE datatree_supervisor_uptime_seconds countermneme_supervisor_uptime_seconds {}\n",
        snap.supervisor_uptime_s
    ));
    body.push_str(&format!(
        "# TYPE datatree_overall_uptime_percent gaugemneme_overall_uptime_percent {}\n",
        snap.overall_uptime_percent
    ));
    body.push_str(&format!(
        "# TYPE datatree_cache_hit_rate gaugemneme_cache_hit_rate {}\n",
        snap.cache_hit_rate
    ));
    for c in &snap.children {
        body.push_str(&format!(
            "datatree_child_restart_count{{child=\"{}\"}} {}\n",
            c.name, c.restart_count
        ));
        if let Some(p50) = c.p50_us {
            body.push_str(&format!(
                "datatree_child_latency_us{{child=\"{}\",quantile=\"0.5\"}} {}\n",
                c.name, p50
            ));
        }
        if let Some(p95) = c.p95_us {
            body.push_str(&format!(
                "datatree_child_latency_us{{child=\"{}\",quantile=\"0.95\"}} {}\n",
                c.name, p95
            ));
        }
        if let Some(p99) = c.p99_us {
            body.push_str(&format!(
                "datatree_child_latency_us{{child=\"{}\",quantile=\"0.99\"}} {}\n",
                c.name, p99
            ));
        }
    }
    (StatusCode::OK, body).into_response()
}

async fn build_snapshot(state: &AppState) -> Result<SlaSnapshot, SupervisorError> {
    let children = state.manager.snapshot().await;
    let supervisor_uptime_s = state.started.elapsed().as_secs();

    // Aggregate uptime % = sum(child total_uptime) / (n_children * supervisor_uptime).
    let denom = (children.len() as u64).saturating_mul(supervisor_uptime_s.max(1));
    let numer: u64 = children.iter().map(|c| c.total_uptime_ms / 1000).sum();
    let overall_uptime_percent = if denom == 0 {
        100.0
    } else {
        ((numer as f64) / (denom as f64)) * 100.0
    };

    let disk = compute_disk_usage(state.manager.config().root_dir.as_path());

    let ws = state.watcher_stats.snapshot();
    let watcher = WatcherMetrics {
        total_reindexed: ws.total_reindexed,
        total_deletes: ws.total_deletes,
        total_ignored: ws.total_ignored,
        p50_ms: ws.p50_ms,
        p95_ms: ws.p95_ms,
        p99_ms: ws.p99_ms,
    };

    Ok(SlaSnapshot {
        timestamp: Utc::now(),
        supervisor_uptime_s,
        children,
        overall_uptime_percent: overall_uptime_percent.min(100.0),
        cache_hit_rate: 0.0, // populated when the store-worker reports back
        disk,
        watcher,
    })
}

fn compute_disk_usage(_root: &std::path::Path) -> DiskUsage {
    let _sys = System::new();
    let disks = Disks::new_with_refreshed_list();
    let mut total: u64 = 0;
    let mut free: u64 = 0;
    for d in disks.list() {
        total = total.saturating_add(d.total_space());
        free = free.saturating_add(d.available_space());
    }
    let used_percent = if total == 0 {
        0.0
    } else {
        ((total - free) as f64 / total as f64) * 100.0
    };
    DiskUsage {
        total_bytes: total,
        free_bytes: free,
        used_percent,
    }
}
