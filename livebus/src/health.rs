//! `/health` endpoint.
//!
//! Returns a JSON snapshot suitable for both human eyeballs and the
//! supervisor's heartbeat probe. Format:
//!
//! ```json
//! {
//!   "state": "ok",
//!   "active_subscribers": 3,
//!   "evicted_subscribers": 0,
//!   "events_per_sec": 482.3,
//!   "published_events": 17422,
//!   "dropped_events_count": 0,
//!   "uptime_seconds": 3612
//! }
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::subscriber::SubscriberManager;

/// Coarse-grained health state.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    Ok,
    Degraded,
}

/// Snapshot returned by `GET /health`.
#[derive(Debug, Clone, Serialize)]
pub struct HealthSnapshot {
    pub state: HealthState,
    pub active_subscribers: usize,
    pub evicted_subscribers: u64,
    pub events_per_sec: f64,
    pub published_events: u64,
    pub dropped_events_count: u64,
    pub uptime_seconds: u64,
}

/// Lightweight rate sampler used to compute `events_per_sec`. The supervisor
/// polls `/health` every few seconds; we keep a single previous-sample point
/// and divide.
#[derive(Debug)]
pub struct RateSampler {
    last_count: AtomicU64,
    last_at: std::sync::Mutex<Instant>,
}

impl RateSampler {
    pub fn new() -> Self {
        Self {
            last_count: AtomicU64::new(0),
            last_at: std::sync::Mutex::new(Instant::now()),
        }
    }

    /// Compute and return the rate since the last call.
    pub fn sample(&self, current_count: u64) -> f64 {
        let mut guard = self.last_at.lock().expect("rate sampler poisoned");
        let now = Instant::now();
        let elapsed = now.duration_since(*guard).as_secs_f64().max(0.001);
        let prev = self.last_count.swap(current_count, Ordering::Relaxed);
        *guard = now;
        let delta = current_count.saturating_sub(prev);
        delta as f64 / elapsed
    }
}

impl Default for RateSampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared state injected into the `/health` handler.
#[derive(Clone)]
pub struct HealthCtx {
    pub mgr: SubscriberManager,
    pub sampler: Arc<RateSampler>,
}

impl std::fmt::Debug for HealthCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthState").finish()
    }
}

/// Axum handler for `GET /health`.
pub async fn health_handler(State(state): State<HealthCtx>) -> impl IntoResponse {
    let bus = state.mgr.bus();
    let stats = state.mgr.stats();
    let published = bus.published_count();
    let dropped = bus.dropped_count();
    let rate = state.sampler.sample(published);

    let coarse = if dropped > 0 || stats.evicted_subscribers > 0 {
        HealthState::Degraded
    } else {
        HealthState::Ok
    };

    let snap = HealthSnapshot {
        state: coarse,
        active_subscribers: stats.active_subscribers,
        evicted_subscribers: stats.evicted_subscribers,
        events_per_sec: round2(rate),
        published_events: published,
        dropped_events_count: dropped,
        uptime_seconds: bus.uptime_seconds(),
    };
    Json(snap)
}

fn round2(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}
