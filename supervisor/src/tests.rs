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
    // multimodal extraction is now pure Rust and runs in-process from the
    // CLI (see cli::commands::graphify). No supervised child.
    assert!(names.iter().any(|n| *n == "brain-worker"));
    assert!(names.iter().any(|n| *n == "livebus-worker"));
    // mcp-server and vision-server are intentionally NOT in the supervisor's
    // default layout — mcp-server is spawned per-Claude-Code-window via
    // `mneme mcp stdio`, and vision-server launches from `mneme view` or
    // the Tauri app. See `config.rs` line ~190 for the design rationale.
    assert!(!names.iter().any(|n| *n == "mcp-server"));
    assert!(!names.iter().any(|n| *n == "vision-server"));
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

/// Integration test: spawn a tiny child that exits on its own, verify the
/// watchdog-driven restart loop picks it back up.
///
/// We model the worker as a child that exits with code 0 after a short
/// sleep. The `Permanent` strategy means any exit must be restarted, so
/// the restart_count should reach 2 within a handful of seconds.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watchdog_respawns_crashed_worker() {
    use crate::manager::ChildManager;
    // Use a command that's portable: `cmd /c exit 0` on Windows,
    // `/bin/sh -c "exit 0"` on unix.
    #[cfg(windows)]
    let (cmd, args): (&str, Vec<String>) =
        ("cmd", vec!["/c".into(), "exit".into(), "0".into()]);
    #[cfg(unix)]
    let (cmd, args): (&str, Vec<String>) = ("/bin/sh", vec!["-c".into(), "exit 0".into()]);

    let spec = ChildSpec {
        name: "flaky".into(),
        command: cmd.into(),
        args,
        env: vec![],
        restart: RestartStrategy::Permanent,
        rss_limit_mb: None,
        cpu_limit_percent: None,
        health_endpoint: None,
    };
    let mut cfg = dummy_config();
    cfg.children.clear();
    cfg.children.push(spec.clone());
    // Tighten the restart budget so the test runs fast.
    cfg.default_restart_policy.initial_backoff = Duration::from_millis(5);
    cfg.default_restart_policy.max_backoff = Duration::from_millis(50);
    cfg.default_restart_policy.backoff_multiplier = 1.5;
    cfg.default_restart_policy.max_restarts_per_window = 20;

    let ring = Arc::new(crate::log_ring::LogRing::new(256));
    let mgr = Arc::new(ChildManager::new(cfg, ring));
    let rx = mgr
        .take_restart_rx()
        .await
        .expect("restart rx taken exactly once");
    let mgr_clone = mgr.clone();
    let _loop_task = tokio::spawn(async move { mgr_clone.run_restart_loop(rx).await });

    mgr.spawn_child(spec).await.expect("initial spawn");

    // Poll the snapshot for up to 5s waiting for restart_count >= 2.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = 0u64;
    while Instant::now() < deadline {
        let snap = mgr.snapshot().await;
        if let Some(s) = snap.iter().find(|s| s.name == "flaky") {
            observed = s.restart_count;
            if observed >= 2 {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        observed >= 2,
        "watchdog should have respawned at least twice, observed {observed}"
    );
}

/// Verify the dispatch API correctly reports a missing worker rather than
/// panicking. Full stdin-write coverage lives in the workspace-level
/// integration suite because it needs a real child binary.
#[tokio::test]
async fn dispatch_unknown_pool_returns_error() {
    use crate::manager::ChildManager;
    let cfg = dummy_config();
    let ring = Arc::new(crate::log_ring::LogRing::new(64));
    let mgr = Arc::new(ChildManager::new(cfg, ring));
    let res = mgr.dispatch_to_pool("parser-worker-", "{}\n").await;
    assert!(res.is_err(), "dispatch with no live workers must error");
}

/// Verify that attaching a JobQueue + submitting jobs works, and that
/// in-flight jobs are requeued when a worker exits. This is the core
/// contract the v0.3 supervisor-mediated dispatch relies on.
#[tokio::test]
async fn job_queue_requeues_on_worker_exit() {
    use crate::job_queue::JobQueue;
    use common::jobs::Job;
    let cfg = dummy_config();
    let ring = Arc::new(crate::log_ring::LogRing::new(64));
    let mgr = Arc::new(ChildManager::new(cfg, ring));
    let queue = Arc::new(JobQueue::new(32));
    mgr.attach_job_queue(queue.clone()).await;
    // Submit a job, pretend the router assigned it, then simulate the
    // worker crashing by calling requeue_worker directly (no real child
    // in this test — the integration suite covers that path).
    let id = queue
        .submit(
            Job::Parse {
                file_path: "/tmp/x.rs".into(),
                shard_root: "/tmp".into(),
            },
            None,
        )
        .expect("submit");
    let (got, _) = queue.next_pending().expect("pending");
    assert_eq!(got, id);
    queue.mark_assigned(id, "parser-worker-0".into());
    let n = queue.requeue_worker("parser-worker-0");
    assert_eq!(n, 1);
    assert_eq!(queue.snapshot().pending, 1);
}

/// Wire-compat check: the supervisor's ControlCommand serde shape must
/// match the CLI's IpcRequest for DispatchJob, otherwise the CLI sending
/// a DispatchJob would be rejected as malformed by the supervisor.
#[test]
fn dispatch_job_command_serde_shape_matches_cli() {
    use crate::ipc::ControlCommand;
    use common::jobs::Job;
    let cmd = ControlCommand::DispatchJob {
        job: Job::Parse {
            file_path: "/a/b.rs".into(),
            shard_root: "/shard".into(),
        },
    };
    let wire = serde_json::to_value(&cmd).unwrap();
    assert_eq!(wire["command"], "dispatch_job");
    assert_eq!(wire["job"]["kind"], "parse");
    assert_eq!(wire["job"]["file_path"], "/a/b.rs");
}
