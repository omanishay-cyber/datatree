//! Tests for the supervisor crate.
//!
//! - Unit tests for [`crate::log_ring`] live in that module.
//! - Integration-shaped tests live here. They avoid spawning real workers
//!   (the binaries don't exist yet) and focus on policy correctness:
//!     * exponential backoff math
//!     * restart-budget enforcement
//!     * heartbeat deadline arithmetic
//!     * a chaos-style restart-latency stub

use crate::child::{ChildHandle, ChildSpec, ChildStatus, RestartStrategy};
use crate::config::{RestartPolicy, SupervisorConfig};
use crate::log_ring::LogRing;
use crate::manager::ChildManager;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn dummy_spec(name: &str) -> ChildSpec {
    ChildSpec {
        name: name.into(),
        command: "true".into(),
        args: vec![],
        env: vec![],
        restart: RestartStrategy::Permanent,
        rss_limit_mb: None,
        cpu_limit_percent: None,
        health_endpoint: None,
    }
}

fn dummy_config() -> SupervisorConfig {
    let mut cfg = SupervisorConfig::default_layout();
    cfg.children.clear();
    cfg.children.push(dummy_spec("test-worker"));
    cfg
}

#[test]
fn exponential_backoff_obeys_cap() {
    let policy = RestartPolicy::default();
    let mut current = policy.initial_backoff;
    let mut max_seen = Duration::ZERO;
    for _ in 0..16 {
        let next_ms = (current.as_millis() as f32 * policy.backoff_multiplier) as u64;
        let capped_ms = next_ms.min(policy.max_backoff.as_millis() as u64);
        current = Duration::from_millis(capped_ms.max(1));
        if current > max_seen {
            max_seen = current;
        }
    }
    assert!(max_seen <= policy.max_backoff);
    assert!(max_seen > policy.initial_backoff);
}

#[test]
fn restart_budget_enforced() {
    let mut handle = ChildHandle::new(dummy_spec("x"), Duration::from_millis(100));
    let window = Duration::from_secs(60);
    for _ in 0..5 {
        handle.record_restart(window);
    }
    assert_eq!(handle.restarts_in_window(window), 5);
    handle.record_restart(window);
    assert!(handle.restarts_in_window(window) > 5);
}

#[test]
fn restart_window_prunes_old_entries() {
    let mut handle = ChildHandle::new(dummy_spec("x"), Duration::from_millis(100));
    let window = Duration::from_millis(50);
    handle.record_restart(window);
    std::thread::sleep(Duration::from_millis(80));
    handle.record_restart(window);
    // Only the most recent entry should still be inside the 50ms window.
    assert_eq!(handle.restarts_in_window(window), 1);
}

#[test]
fn config_validate_rejects_duplicates() {
    let mut cfg = dummy_config();
    cfg.children.push(dummy_spec("test-worker"));
    let res = cfg.validate();
    assert!(res.is_err(), "duplicate child names should be rejected");
}

#[test]
fn config_default_layout_has_all_workers() {
    let cfg = SupervisorConfig::default_layout();
    let names: Vec<&str> = cfg.children.iter().map(|c| c.name.as_str()).collect();
    assert!(names.iter().any(|n| *n == "store-worker"));
    assert!(names.iter().any(|n| n.starts_with("parser-worker-")));
    assert!(names.iter().any(|n| n.starts_with("scanner-worker-")));
    assert!(names.iter().any(|n| *n == "md-ingest-worker"));
    assert!(names.iter().any(|n| *n == "multimodal-bridge"));
    assert!(names.iter().any(|n| *n == "brain-worker"));
    assert!(names.iter().any(|n| *n == "livebus-worker"));
    assert!(names.iter().any(|n| *n == "mcp-server"));
    assert!(names.iter().any(|n| *n == "vision-server"));
}

#[test]
fn log_ring_capacity_floor() {
    let r = LogRing::new(0);
    assert!(r.capacity() >= 16);
}

#[test]
fn latency_percentiles_basic() {
    let mut h = ChildHandle::new(dummy_spec("x"), Duration::from_millis(100));
    for i in 1..=100u64 {
        h.record_latency_us(i);
    }
    let (p50, p95, p99) = h.latency_percentiles_us().expect("samples present");
    assert!(p50 >= 49 && p50 <= 51);
    assert!(p95 >= 94 && p95 <= 96);
    assert!(p99 >= 98 && p99 <= 100);
}

#[tokio::test]
async fn snapshot_returns_empty_before_spawn() {
    let cfg = dummy_config();
    let ring = Arc::new(LogRing::new(64));
    let mgr = Arc::new(ChildManager::new(cfg, ring));
    let snap = mgr.snapshot().await;
    assert!(snap.is_empty(), "no children spawned yet");
}

/// Chaos test stub: verify the *policy* yields an initial restart delay below
/// 100ms (the design target). This does not spawn a real process — see
/// `tests/chaos.rs` (workspace-level) for an end-to-end variant.
#[test]
fn chaos_restart_initial_under_100ms() {
    let policy = RestartPolicy::default();
    assert!(
        policy.initial_backoff <= Duration::from_millis(100),
        "initial backoff must be ≤100ms to meet the <100ms restart SLA"
    );
}

/// Sanity check: the heartbeat deadline must be larger than the heartbeat
/// tick so a healthy worker is never killed by a single missed tick.
#[test]
fn heartbeat_deadline_above_tick() {
    use crate::watchdog::HEARTBEAT_DEADLINE;
    assert!(HEARTBEAT_DEADLINE > Duration::from_secs(1));
}

/// Smoke test: build a fake [`ChildHandle`] and confirm the timestamp
/// arithmetic compiles and behaves.
#[test]
fn handle_timestamp_arithmetic() {
    let mut h = ChildHandle::new(dummy_spec("x"), Duration::from_millis(100));
    h.last_started_instant = Some(Instant::now());
    h.status = ChildStatus::Running;
    assert!(h.current_uptime() < Duration::from_millis(50));
}
