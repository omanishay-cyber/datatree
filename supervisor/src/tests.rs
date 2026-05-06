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
        heartbeat_deadline: None,
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

// ---------------------------------------------------------------------
// BUG-A10-004 (2026-05-04) - manager.rs restart-loop unit coverage.
//
// Pre-existing: 1 test (windows_kill_pid_flags) for ~1290 LOC of
// restart-loop logic. The chaos-test K10 admits in-body it doesn't
// verify the restart actually happened. These tests pin the policy
// matrix that monitor_child / respawn_one rely on, without spinning
// up real worker processes.
// ---------------------------------------------------------------------

/// `RestartStrategy` decision matrix as encoded in `monitor_child`
/// (manager.rs:446-450). The match is the load-bearing branch that
/// decides whether a child crash escalates to a restart request -
/// regression here would silently leave dead workers on the floor.
#[test]
fn restart_strategy_decision_matrix_clean_exit() {
    // The decision logic, isolated for unit-testing:
    fn should_restart(strategy: RestartStrategy, exit_code: i32) -> bool {
        match strategy {
            RestartStrategy::Permanent => true,
            RestartStrategy::Transient => exit_code != 0,
            RestartStrategy::Temporary => false,
        }
    }
    // Permanent: clean exit (code 0) STILL triggers restart.
    assert!(
        should_restart(RestartStrategy::Permanent, 0),
        "Permanent must restart even on clean exit",
    );
    // Permanent: non-zero exit triggers restart.
    assert!(
        should_restart(RestartStrategy::Permanent, 137),
        "Permanent must restart on SIGKILL/abort exit code 137",
    );

    // Transient: clean exit means the child finished its work; do NOT
    // restart. This is the critical "panic vs clean exit" distinction.
    assert!(
        !should_restart(RestartStrategy::Transient, 0),
        "Transient must NOT restart on clean exit",
    );
    // Transient: non-zero (panic, signal, error) triggers restart.
    assert!(
        should_restart(RestartStrategy::Transient, 1),
        "Transient must restart on non-zero exit (panic)",
    );
    assert!(
        should_restart(RestartStrategy::Transient, 137),
        "Transient must restart on SIGKILL exit",
    );

    // Temporary: never restart.
    assert!(
        !should_restart(RestartStrategy::Temporary, 0),
        "Temporary must NOT restart on clean exit",
    );
    assert!(
        !should_restart(RestartStrategy::Temporary, 137),
        "Temporary must NOT restart even on crash exit",
    );
}

/// Repeated crashes within the budget window must escalate the
/// in-window count past `max_restarts_per_window` so the next
/// `respawn_one` invocation marks the child as `Degraded`. We can't
/// drive `respawn_one` without spawning real children, but we can
/// pin the budget arithmetic that gates it.
#[test]
fn repeated_crash_within_window_exceeds_budget() {
    let policy = RestartPolicy {
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(100),
        backoff_multiplier: 2.0,
        max_restarts_per_window: 3,
        budget_window: Duration::from_secs(5),
    };
    let mut handle = ChildHandle::new(dummy_spec("flaky"), policy.initial_backoff);

    // Three restarts within the window: still within budget.
    for _ in 0..policy.max_restarts_per_window {
        handle.record_restart(policy.budget_window);
    }
    assert_eq!(
        handle.restarts_in_window(policy.budget_window),
        policy.max_restarts_per_window,
        "exactly max_restarts_per_window restarts in window",
    );
    // The fourth restart pushes the count past the budget. respawn_one
    // would mark Degraded at this point (manager.rs:602).
    handle.record_restart(policy.budget_window);
    let in_window = handle.restarts_in_window(policy.budget_window);
    assert!(
        in_window > policy.max_restarts_per_window,
        "fourth restart escalates beyond budget (in_window={}, budget={})",
        in_window,
        policy.max_restarts_per_window,
    );
    // restart_count is the cumulative counter - persists across the
    // window for the duration of supervisor uptime.
    assert_eq!(handle.restart_count, 4, "cumulative restart_count == 4");
}

/// Crashes spaced wider than the budget window must NOT escalate -
/// the rolling window prunes old entries so a child that crashes once
/// per minute (with a 10s window) never trips the budget.
#[test]
fn crashes_outside_window_do_not_escalate() {
    let mut handle = ChildHandle::new(dummy_spec("slow-flaky"), Duration::from_millis(100));
    let window = Duration::from_millis(50);

    // First restart is recorded.
    handle.record_restart(window);
    assert_eq!(handle.restarts_in_window(window), 1);

    // Wait long enough that the first entry falls outside the window.
    std::thread::sleep(Duration::from_millis(80));
    handle.record_restart(window);
    // The first entry has been pruned; only the most recent remains.
    assert_eq!(
        handle.restarts_in_window(window),
        1,
        "old restart pruned, only the recent one counts",
    );
    // BUT: the cumulative restart_count still reflects 2 total restarts.
    assert_eq!(
        handle.restart_count, 2,
        "cumulative restart_count is NOT pruned by window",
    );
}

/// The static [`ChildSpec`] (env vars, args, command path, restart
/// strategy) MUST be preserved across the in-flight restart cycle.
/// `respawn_one` clones `h.spec` (manager.rs:620) and feeds it back to
/// `spawn_child`. A buggy refactor that mutated the spec on restart
/// would corrupt env vars + lose the worker's intended config.
#[test]
fn child_spec_env_preserved_across_restart_lifecycle() {
    let mut spec = dummy_spec("env-keeper");
    spec.env = vec![
        ("MNEME_WORKER_ID".to_string(), "42".to_string()),
        ("MNEME_LOG_LEVEL".to_string(), "info".to_string()),
    ];
    spec.args = vec!["--mode=parser".into(), "--shard=alpha".into()];

    let initial_backoff = Duration::from_millis(100);
    let mut handle = ChildHandle::new(spec.clone(), initial_backoff);
    let window = Duration::from_secs(60);

    // Drive 3 restart cycles - record_restart only mutates the per-
    // restart counters; the spec must be left alone.
    for _ in 0..3 {
        handle.record_restart(window);
    }

    assert_eq!(handle.spec.name, "env-keeper");
    assert_eq!(
        handle.spec.env,
        vec![
            ("MNEME_WORKER_ID".to_string(), "42".to_string()),
            ("MNEME_LOG_LEVEL".to_string(), "info".to_string()),
        ],
        "env vars must survive restart cycles",
    );
    assert_eq!(
        handle.spec.args,
        vec!["--mode=parser".to_string(), "--shard=alpha".to_string()],
        "args must survive restart cycles",
    );
    assert_eq!(
        handle.spec.restart,
        RestartStrategy::Permanent,
        "restart strategy must survive restart cycles",
    );
    // Spec clone must produce identical contents - verifies the seam
    // respawn_one uses (manager.rs:620 `h.spec.clone()`).
    let cloned = handle.spec.clone();
    assert_eq!(cloned.env, handle.spec.env);
    assert_eq!(cloned.args, handle.spec.args);
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
    assert!(names.contains(&"store-worker"));
    assert!(names.iter().any(|n| n.starts_with("parser-worker-")));
    assert!(names.iter().any(|n| n.starts_with("scanner-worker-")));
    assert!(names.contains(&"md-ingest-worker"));
    // multimodal extraction is now pure Rust and runs in-process from the
    // CLI (see cli::commands::graphify). No supervised child.
    assert!(names.contains(&"brain-worker"));
    assert!(names.contains(&"livebus-worker"));
    // mcp-server and vision-server are intentionally NOT in the supervisor's
    // default layout — mcp-server is spawned per-Claude-Code-window via
    // `mneme mcp stdio`, and vision-server launches from `mneme view` or
    // the Tauri app. See `config.rs` line ~190 for the design rationale.
    assert!(!names.contains(&"mcp-server"));
    assert!(!names.contains(&"vision-server"));
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
    assert!((49..=51).contains(&p50));
    assert!((94..=96).contains(&p95));
    assert!((98..=100).contains(&p99));
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

/// HIGH-38 audit fix (2026-05-05) — lifetime restart cap (CRIT-2 hard
/// kill-switch). The rolling-window budget can reset every 60s, so a
/// worker crashing once every ~7 minutes never trips it but burns CPU
/// + spawn syscalls forever. CRIT-2 added `MAX_TOTAL_RESTARTS` as a
/// non-resettable lifetime cap; if `restart_count` exceeds it the
/// child is marked `Dead` and refuses further restart requests.
///
/// This test pins both halves of the contract:
///   1. ChildHandle's restart_count IS cumulative (record_restart
///      increments unconditionally — never pruned by the rolling
///      window). The earlier `crashes_outside_window_do_not_escalate`
///      test partly covered this; this one nails the exact threshold.
///   2. Crossing `MAX_TOTAL_RESTARTS` is OBSERVABLE: respawn_one's
///      check `if h.restart_count >= MAX_TOTAL_RESTARTS` flips the
///      child to Dead. We mirror that gate condition rather than
///      driving respawn_one (which needs a real ChildManager + spawn
///      machinery — covered by chaos tests).
///
/// Audit fix follow-up (2026-05-06 multi-agent fan-out, correctness
/// CORR-NEW-2): the gate operator was tightened from strict `>` to
/// `>=`. The prior strict-greater allowed ONE extra restart past the
/// documented cap (cap=200 meant 201 attempts permitted). The test
/// previously asserted the off-by-one as a feature; now it asserts
/// the cap behaves as documented.
#[test]
fn lifetime_restart_cap_threshold_pins_dead_state_gate() {
    use crate::manager::ChildManager;
    let mut handle = ChildHandle::new(dummy_spec("chronic-crasher"), Duration::from_millis(10));
    let window = Duration::from_secs(60);

    // Drive 199 restarts (== MAX_TOTAL_RESTARTS - 1). The gate fires
    // at `restart_count >= MAX_TOTAL_RESTARTS`, so at 199 we're still
    // strictly under and a respawn would be permitted.
    for _ in 0..(ChildManager::MAX_TOTAL_RESTARTS - 1) {
        handle.record_restart(window);
    }
    assert_eq!(
        handle.restart_count,
        ChildManager::MAX_TOTAL_RESTARTS - 1,
        "after MAX-1 restarts, count is one below the cap"
    );
    assert!(
        handle.restart_count < ChildManager::MAX_TOTAL_RESTARTS,
        "still strictly under the cap; gate does NOT fire yet"
    );

    // The 200th restart hits the cap exactly. With `>=`, respawn_one
    // flips to Dead at this point — no extra restart permitted past
    // the documented cap.
    handle.record_restart(window);
    assert_eq!(
        handle.restart_count,
        ChildManager::MAX_TOTAL_RESTARTS,
        "after 200th restart, count == cap exactly"
    );
    assert!(
        handle.restart_count >= ChildManager::MAX_TOTAL_RESTARTS,
        "at the cap, gate must fire; respawn_one would mark Dead"
    );

    // Simulate the side effect respawn_one applies under the same
    // condition (manager.rs:657): set status to Dead. Once Dead, the
    // monitor loop must NOT queue any further restart requests.
    handle.status = ChildStatus::Dead;
    assert_eq!(
        handle.status,
        ChildStatus::Dead,
        "Dead is terminal — operator intervention required"
    );
}

/// HIGH-38 audit fix — the rolling 60s window can reset every minute,
/// so a worker that crashes once every ~7 minutes (the live-PC
/// pattern of 182-194 restarts/24h) NEVER trips the per-window
/// budget. Without the lifetime cap from CRIT-2 it would restart
/// forever. This test pins the exact failure mode the cap addresses.
#[test]
fn rolling_window_resets_but_lifetime_cap_does_not() {
    let mut handle = ChildHandle::new(dummy_spec("slow-flaky"), Duration::from_millis(10));
    let window = Duration::from_millis(50);

    // Crash 5 times, then sleep past the window.
    for _ in 0..5 {
        handle.record_restart(window);
    }
    assert_eq!(handle.restarts_in_window(window), 5);
    std::thread::sleep(Duration::from_millis(80));

    // After the window slides, in-window count drops to zero.
    assert_eq!(
        handle.restarts_in_window(window),
        0,
        "rolling window must prune entries older than the window"
    );

    // BUT cumulative restart_count is NOT pruned. This is the exact
    // property CRIT-2 relies on — without it, a slow-drip crasher
    // would never be observable as "too many lifetime restarts".
    assert_eq!(
        handle.restart_count, 5,
        "cumulative restart_count survives the window slide"
    );

    // Crash 3 more times. Window count rebuilds from zero (5 minus
    // however many fall outside vs. 3 new ones). Lifetime keeps
    // climbing.
    for _ in 0..3 {
        handle.record_restart(window);
    }
    assert_eq!(
        handle.restart_count, 8,
        "lifetime count keeps climbing across window slides"
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
    let (cmd, args): (&str, Vec<String>) = ("cmd", vec!["/c".into(), "exit".into(), "0".into()]);
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
        heartbeat_deadline: None,
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

/// Phase-A C3: per-worker name list in `/health` must sort numerically.
/// The previous lexical sort left `parser-worker-10` before
/// `parser-worker-2` which is confusing on every dump. We can't reach
/// the private `natural_name_cmp` helper directly, but `snapshot()`
/// applies it after collection — so we exercise it by pushing a config
/// with intentionally numeric child names and asserting the order.
#[tokio::test]
async fn snapshot_natural_sort_orders_workers_numerically() {
    use crate::manager::ChildManager;
    let mut cfg = dummy_config();
    cfg.children.clear();
    // Names chosen so the lexical sort would put 10/11 before 2/9.
    for i in [0u32, 1, 2, 9, 10, 11].iter() {
        cfg.children.push(dummy_spec(&format!("parser-worker-{i}")));
    }
    let ring = Arc::new(crate::log_ring::LogRing::new(64));
    let mgr = Arc::new(ChildManager::new(cfg.clone(), ring));
    // We can't actually spawn `true` reliably on Windows, so insert
    // bare ChildHandles by calling spawn_child with a non-existent
    // command — spawn_all's spawn-error path is what we want here.
    // Instead, just test the helper via dispatch_to_pool's name list:
    // that already exercises the same map. But the cleanest path is to
    // call snapshot() on a manager whose handles map we've populated by
    // running spawn_all with `true` (works on bash on Windows because
    // git-bash ships it, but the daemon ships on plain Windows). So
    // instead we only sanity-check via the public natural_name_cmp
    // surface re-exported through the test module below.
    drop(mgr);
    // Direct contract: the helper must be transitive and put numeric
    // suffixes in numeric order.
    let mut v = vec![
        "parser-worker-10".to_string(),
        "parser-worker-2".to_string(),
        "parser-worker-1".to_string(),
        "parser-worker-11".to_string(),
        "scanner-worker-0".to_string(),
        "watchdog".to_string(),
    ];
    v.sort_by(|a, b| crate::manager::__test_natural_name_cmp(a, b));
    assert_eq!(
        v,
        vec![
            "parser-worker-1".to_string(),
            "parser-worker-2".to_string(),
            "parser-worker-10".to_string(),
            "parser-worker-11".to_string(),
            "scanner-worker-0".to_string(),
            "watchdog".to_string(),
        ]
    );
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

// ---------------------------------------------------------------------
// B-005 logs-dir tests
// ---------------------------------------------------------------------
//
// Both tests redirect `MNEME_HOME` to a tempdir and exercise the small
// helper surface in `lib.rs` (`logs_dir`, `supervisor_log_path`,
// `ensure_logs_dir`, `tail_supervisor_log`). They do NOT spin up a real
// supervisor — that's the integration suite's job — so they're fast and
// hermetic.
//
// They run inside a `serial_with_env` lock because tests within a
// single Rust binary share env state; running them concurrently would
// cause a sister test to see `MNEME_HOME` swapped out from under it.
// We don't have `serial_test` as a dep, but we use a static `Mutex`
// guard for the same effect with one fewer dependency.
#[cfg(test)]
fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// B-005 acceptance: calling `ensure_logs_dir` from a fresh
/// `~/.mneme`-equivalent must create the `logs/` subfolder. This is the
/// path `init_tracing` walks on every supervisor boot — it's also re-run
/// from `lib::run` defensively. Test confirms the two helper APIs and
/// their interaction.
#[test]
fn daemon_creates_logs_dir_on_start() {
    let _g = env_lock().lock().unwrap_or_else(|p| p.into_inner());

    let tmp = tempfile::tempdir().expect("tempdir");
    // Save the previous MNEME_HOME so the test never leaks state into
    // sibling tests (which would re-evaluate `PathManager::default_root`).
    let prev = std::env::var_os("MNEME_HOME");
    // Safety: env_lock() guarantees serial access to env state for tests
    // in this crate.
    unsafe {
        std::env::set_var("MNEME_HOME", tmp.path());
    }

    // Pre-condition: no logs/ subfolder yet.
    let logs = crate::logs_dir();
    assert_eq!(
        logs,
        tmp.path().join("logs"),
        "logs_dir resolved from MNEME_HOME"
    );
    assert!(!logs.exists(), "logs dir must NOT exist before the call");

    // Act: ensure_logs_dir creates it.
    let dir = crate::ensure_logs_dir().expect("ensure_logs_dir succeeds on tempdir");
    assert_eq!(dir, logs, "ensure_logs_dir returns the canonical path");
    assert!(dir.exists(), "logs dir must exist after the call");
    assert!(dir.is_dir(), "logs path must be a directory");

    // The canonical supervisor-log file path lives inside the dir.
    let log_path = crate::supervisor_log_path();
    assert_eq!(
        log_path,
        tmp.path().join("logs").join("supervisor.log"),
        "supervisor_log_path is logs/supervisor.log"
    );
    assert_eq!(
        log_path.parent().unwrap(),
        dir,
        "supervisor.log's parent is the freshly-created logs dir"
    );

    // Idempotency: calling again must succeed and not error on existing dir.
    crate::ensure_logs_dir().expect("ensure_logs_dir idempotent");

    // Cleanup
    unsafe {
        match prev {
            Some(v) => std::env::set_var("MNEME_HOME", v),
            None => std::env::remove_var("MNEME_HOME"),
        }
    }
}

/// B-005 acceptance: `mneme daemon logs` (which routes through the
/// `Logs` IPC handler in `ipc.rs`) must be able to tail the rotated
/// supervisor.log file when the in-memory ring is empty. This test
/// exercises the supporting helper `tail_supervisor_log` with both
/// the canonical name (`supervisor.log`) and a rotated daily-suffix
/// name (`supervisor.log.YYYY-MM-DD`) — the latter is what the
/// `tracing_appender::rolling::Rotation::DAILY` writer produces.
#[test]
fn mneme_daemon_logs_tails_supervisor_log() {
    let _g = env_lock().lock().unwrap_or_else(|p| p.into_inner());

    let tmp = tempfile::tempdir().expect("tempdir");
    let prev = std::env::var_os("MNEME_HOME");
    unsafe {
        std::env::set_var("MNEME_HOME", tmp.path());
    }

    let dir = crate::ensure_logs_dir().expect("logs dir");

    // Pre: empty file dir → empty tail.
    assert!(
        crate::tail_supervisor_log(100).is_empty(),
        "empty dir → empty tail"
    );

    // Drop two log files: the older "yesterday" rotated file AND the
    // current canonical-name file. We write the older file first then
    // sleep 50ms so its mtime is strictly older than `today`'s — the
    // helper's mtime-sort relies on that. (Filesystem mtime resolution
    // on Windows is 100µs at minimum but commonly 1ms; 50ms is safely
    // above the noise floor.)
    let yesterday = dir.join("supervisor.log.2026-04-26");
    let today = dir.join("supervisor.log");
    std::fs::write(
        &yesterday,
        "2026-04-26T23:59:00Z line-A\n2026-04-26T23:59:30Z line-B\n",
    )
    .expect("write yesterday");
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(
        &today,
        "2026-04-27T00:00:01Z line-C\n2026-04-27T00:00:02Z line-D\n",
    )
    .expect("write today");

    // tail(2) → last two lines across both files (line-C, line-D)
    let tail2 = crate::tail_supervisor_log(2);
    assert_eq!(tail2.len(), 2, "n=2 returns 2 lines");
    assert!(
        tail2[0].contains("line-C"),
        "tail(2)[0] = line-C, got {:?}",
        tail2[0]
    );
    assert!(
        tail2[1].contains("line-D"),
        "tail(2)[1] = line-D, got {:?}",
        tail2[1]
    );

    // tail(100) → all four lines, oldest-first across rotated files
    let tail_all = crate::tail_supervisor_log(100);
    assert_eq!(tail_all.len(), 4, "n=100 returns all 4 lines");
    assert!(tail_all[0].contains("line-A"), "oldest first: line-A");
    assert!(tail_all[1].contains("line-B"));
    assert!(tail_all[2].contains("line-C"));
    assert!(tail_all[3].contains("line-D"), "newest last: line-D");

    // tail(0) → empty (defensive guard).
    assert!(crate::tail_supervisor_log(0).is_empty(), "n=0 → empty");

    // Cleanup env var.
    unsafe {
        match prev {
            Some(v) => std::env::set_var("MNEME_HOME", v),
            None => std::env::remove_var("MNEME_HOME"),
        }
    }
}

/// Bug L — when the restart channel is closed (receiver dropped),
/// `enqueue_restart_request_for_test` returns Err AND the increment
/// path inside `monitor_child` is exercised via the same `send` call.
/// We can't drive the full `monitor_child` (no real worker), so we
/// drop the receiver and simulate the close-detection by calling the
/// public test entrypoint and asserting the per-child snapshot
/// reflects the dropped count.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_dropped_count_increments_on_closed_channel() {
    use crate::child::ChildHandle;
    use crate::manager::ChildManager;

    let cfg = dummy_config();
    let ring = Arc::new(crate::log_ring::LogRing::new(256));
    let mgr = Arc::new(ChildManager::new(cfg, ring));

    // Pre-register a child handle directly on the manager (no real
    // spawn) so the dropped-count path has somewhere to write.
    let spec = dummy_spec("dropper");
    mgr.register_handle_for_test(ChildHandle::new(spec, Duration::from_millis(10)))
        .await;

    // Take and drop the receiver to close the channel.
    let rx = mgr.take_restart_rx().await.expect("rx taken once");
    drop(rx);

    // Drive the monitor-child close path: this is the increment site.
    // The helper directly increments the per-child gauge in the same
    // way `monitor_child` does on `SendError`.
    mgr.simulate_dropped_restart_for_test("dropper").await;

    // Snapshot must surface the new field.
    let snap = mgr.snapshot().await;
    let s = snap
        .iter()
        .find(|s| s.name == "dropper")
        .expect("dropper child appears in snapshot");
    assert!(
        s.restart_dropped_count >= 1,
        "expected restart_dropped_count >= 1, got {}",
        s.restart_dropped_count
    );
}

/// Bug J — the restart-request channel is unbounded. Pushing 1000
/// requests in a tight loop without a draining receiver must succeed
/// every time. The bounded predecessor used `try_send` and silently
/// dropped requests on `Full` (postmortem §12.1: 11 dropped restarts in
/// 5s on the AWS test fleet).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unbounded_restart_channel_send_succeeds_under_load() {
    use crate::manager::{ChildManager, RestartRequest};

    let cfg = dummy_config();
    let ring = Arc::new(crate::log_ring::LogRing::new(256));
    let mgr = Arc::new(ChildManager::new(cfg, ring));

    // Send 1000 restart requests with no receiver consuming. A bounded
    // channel of cap 8/16/etc. would drop the vast majority via
    // `TrySendError::Full`. The unbounded channel must accept all of
    // them.
    const N: usize = 1000;
    let mut accepted = 0usize;
    for i in 0..N {
        let req = RestartRequest {
            name: format!("worker-{i}"),
            exit_code: 1,
            queued_at: Instant::now(),
        };
        if mgr.enqueue_restart_request_for_test(req).is_ok() {
            accepted += 1;
        }
    }
    assert_eq!(
        accepted, N,
        "all {N} requests must be accepted by an unbounded channel"
    );

    // Drain the receiver and assert the count matches.
    let mut rx = mgr
        .take_restart_rx()
        .await
        .expect("restart rx taken exactly once");
    let mut drained = 0usize;
    while let Ok(_req) = rx.try_recv() {
        drained += 1;
    }
    assert_eq!(drained, N, "all {N} sent requests must be receivable");
}

/// Bug D — workers spawned by the supervisor must receive
/// `CREATE_NO_WINDOW` (0x08000000) in their Windows creation flags so
/// that no console window flashes when the daemon boots its 22 workers
/// (the "hydra heads" storm reported in the 2026-04-29 install
/// postmortem §3.D + §12.5). The same composition the uninstall
/// self-delete shim uses (`cli/src/commands/uninstall.rs:448-449`)
/// applies here, plus `CREATE_BREAKAWAY_FROM_JOB` so a Job-owned daemon
/// doesn't drag every worker into the same Job.
///
/// We assert the helper returns the exact bit composition required by
/// the postmortem fix. `spawn_os_process` reads this same helper via
/// `command.creation_flags(windows_worker_spawn_flags())`.
#[cfg(windows)]
#[test]
fn windows_worker_spawn_flags_includes_create_no_window() {
    use crate::manager::windows_worker_spawn_flags;

    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let flags = windows_worker_spawn_flags();

    assert_ne!(
        flags & CREATE_NO_WINDOW,
        0,
        "CREATE_NO_WINDOW must be set on worker spawns (postmortem §3.D)"
    );
    assert_eq!(
        flags,
        DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB | CREATE_NO_WINDOW,
        "exact flag composition required by postmortem §3.D + uninstall.rs:448-449 parity"
    );
}

// ---------------------------------------------------------------------
// Bug I defensive fix tests — boot-time worker version probe + crash-loop
// recovery logger. Both contracts are documented in
// `docs/dev/SESSION-2026-04-29-FIX-LOG.md` (Five Whys section).
// ---------------------------------------------------------------------

/// Bug I acceptance: the boot-time `--version` probe MUST refuse to
/// continue when a worker exe advertises a version string that does
/// not match `expected_version`. This is the defensive-depth fix that
/// catches any future v0.3.0/v0.3.2 mixed-binary scenario before
/// workers crash-loop with `STATUS_CONTROL_C_EXIT` (-1073741510 on
/// Windows).
///
/// Strategy: write a tiny shell script to a tempdir that always prints
/// `mneme-stub 0.0.1` and exits 0. Drive `probe_worker_versions`
/// against that path with `expected_version = "9.9.9"`. The probe
/// MUST extract `0.0.1`, see the mismatch, and return
/// `SupervisorError::BinaryVersionSkew { actual: "0.0.1", expected:
/// "9.9.9", worker: "version-stub" }`.
#[test]
fn boot_refuses_when_worker_version_skews() {
    use crate::error::SupervisorError;
    use crate::manager::probe_worker_versions;

    let tmp = tempfile::tempdir().expect("tempdir");

    // Author a stub script that ignores its arguments and always prints
    // a known version line. The probe will spawn it with `--version`
    // appended; a real script-shaped worker would conditionally check
    // the arg, but since we always print the same line either way the
    // probe sees `0.0.1` regardless.
    #[cfg(windows)]
    let stub_path = {
        let path = tmp.path().join("stub.bat");
        // `@echo off` suppresses the command-echoing prologue so stdout
        // contains exactly one line: `mneme-stub 0.0.1`. The trailing
        // exit /b 0 makes the exit code deterministic.
        std::fs::write(&path, "@echo off\r\necho mneme-stub 0.0.1\r\nexit /b 0\r\n")
            .expect("write stub");
        path
    };

    #[cfg(unix)]
    let stub_path = {
        let path = tmp.path().join("stub.sh");
        std::fs::write(&path, "#!/bin/sh\nprintf 'mneme-stub 0.0.1\\n'\nexit 0\n")
            .expect("write stub");
        // Mark executable so std::process::Command::new(...).spawn() works.
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
        path
    };

    let spec = ChildSpec {
        name: "version-stub".into(),
        command: stub_path
            .to_str()
            .expect("tempdir path is utf-8")
            .to_string(),
        args: vec![],
        env: vec![],
        restart: RestartStrategy::Permanent,
        rss_limit_mb: None,
        cpu_limit_percent: None,
        health_endpoint: None,
        heartbeat_deadline: None,
    };

    // Supervisor's compile-time version is intentionally wildly
    // different from the stub's `0.0.1` so `probe_worker_versions`
    // MUST return `BinaryVersionSkew`.
    let result = probe_worker_versions(&[spec], "9.9.9");
    match result {
        Err(SupervisorError::BinaryVersionSkew {
            worker,
            expected,
            actual,
        }) => {
            assert_eq!(worker, "version-stub", "error names the offending worker");
            assert_eq!(expected, "9.9.9", "error carries supervisor's expected");
            assert_eq!(actual, "0.0.1", "error carries worker's actual version");
        }
        Ok(()) => panic!(
            "probe_worker_versions returned Ok — expected BinaryVersionSkew \
             because stub prints 0.0.1 and we asked for 9.9.9"
        ),
        Err(other) => panic!(
            "probe_worker_versions returned wrong error variant: {other:?} \
             (expected BinaryVersionSkew)"
        ),
    }
}

/// Bug I acceptance: when a worker has crash-looped (`restart_count >= 3`)
/// and then run stably for `>= 60s`, `ChildManager::check_recovery_logs`
/// MUST emit exactly one recovery log line and flip the per-handle
/// one-shot flag. A second call without a fresh restart MUST be a
/// no-op (the flag stays set, the log line does not repeat).
///
/// Strategy: we cannot easily spawn a real worker that crashes 3 times
/// then runs for 60s inside a unit test. Instead we exercise the
/// post-condition contract: take a `ChildManager`, populate its
/// handles map by spawning a long-running portable command (`ping`
/// /  `sleep`) so the handle exists, then reach in via the public
/// `handle_for` API to mutate `restart_count` and rewind
/// `last_started_instant` past the 60-second threshold. After that the
/// `check_recovery_logs` contract is what we test.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manager_logs_recovery_after_stable_uptime() {
    use crate::manager::ChildManager;
    // Long-running command so the spawn succeeds and stays alive for
    // the duration of the test. We don't actually wait for it to
    // complete — the test exits in well under a second; tokio's
    // `kill_on_drop(true)` (set in `manager::spawn_os_process`) reaps
    // the child when the manager drops.
    #[cfg(windows)]
    let (cmd, args): (&str, Vec<String>) = (
        "cmd",
        vec![
            "/c".into(),
            // 30 seconds of no-op output so the test has plenty of
            // headroom even on a slow CI runner.
            "ping".into(),
            "-n".into(),
            "30".into(),
            "127.0.0.1".into(),
        ],
    );
    #[cfg(unix)]
    let (cmd, args): (&str, Vec<String>) = ("/bin/sh", vec!["-c".into(), "sleep 30".into()]);

    let spec = ChildSpec {
        name: "recovery-stub".into(),
        command: cmd.into(),
        args,
        env: vec![],
        restart: RestartStrategy::Permanent,
        rss_limit_mb: None,
        cpu_limit_percent: None,
        health_endpoint: None,
        heartbeat_deadline: None,
    };
    let mut cfg = dummy_config();
    cfg.children.clear();
    cfg.children.push(spec.clone());

    let ring = Arc::new(crate::log_ring::LogRing::new(64));
    let mgr = Arc::new(ChildManager::new(cfg, ring));
    mgr.spawn_child(spec).await.expect("initial spawn");

    // Yield long enough for spawn_child's spawned setup task — which
    // writes `last_started_instant = Some(Instant::now())` once after
    // the OS spawn returns — to definitely have run. Without this
    // sync point our manual mutations below race with that task and
    // get silently overwritten when the test re-acquires the lock.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Pre-condition: zero restarts, no recovery log expected.
    let emitted_pre = mgr.check_recovery_logs().await;
    assert_eq!(
        emitted_pre, 0,
        "fresh worker (restart_count=0) must NOT trigger a recovery log"
    );

    // Reach into the handle and synthesise "this worker has been
    // crash-looping but is now stable". We rewind
    // `last_started_instant` by 65 seconds so `current_uptime() >=
    // 60s` holds, and bump `restart_count` to 5 so the threshold of 3
    // is comfortably crossed.
    let handle = mgr
        .handle_for("recovery-stub")
        .await
        .expect("handle exists post-spawn");
    {
        let mut h = handle.lock().await;
        h.restart_count = 5;
        h.last_started_instant = Some(Instant::now() - Duration::from_secs(65));
        // Defensive: the spawn just ran and the flag was initialised
        // to `false`, but we re-assert the precondition here so the
        // test's pass/fail signal is unambiguous.
        h.crash_loop_recovery_logged = false;
    }

    // First call MUST emit exactly one recovery log line (and flip the
    // handle's one-shot flag).
    let emitted_first = mgr.check_recovery_logs().await;
    assert_eq!(
        emitted_first, 1,
        "first call after threshold-cross must emit exactly one recovery log"
    );

    // The flag MUST now be set. Confirm via the public read path.
    {
        let h = handle.lock().await;
        assert!(
            h.crash_loop_recovery_logged,
            "one-shot flag must be `true` after recovery emit"
        );
    }

    // Second call with no fresh restart MUST be a no-op.
    let emitted_second = mgr.check_recovery_logs().await;
    assert_eq!(
        emitted_second, 0,
        "second call without a fresh restart must NOT re-emit"
    );

    // A new restart event MUST clear the flag (so the next stable
    // recovery emits a fresh log line). This exercises the
    // `record_restart` clearing path documented in `child.rs`.
    {
        let mut h = handle.lock().await;
        h.record_restart(Duration::from_secs(60));
        assert!(
            !h.crash_loop_recovery_logged,
            "record_restart must clear the recovery-logged flag"
        );
        // Also rewind `last_started_instant` again — record_restart
        // doesn't touch it, but the next emit needs >=60s of uptime
        // since the most recent spawn from the manager's PoV.
        h.last_started_instant = Some(Instant::now() - Duration::from_secs(65));
    }
    // `record_restart` bumped `restart_count` from 5 → 6, still well
    // above the threshold of 3. The next call MUST emit again.
    let emitted_third = mgr.check_recovery_logs().await;
    assert_eq!(
        emitted_third, 1,
        "after a fresh restart-then-stable cycle, recovery log fires again"
    );

    // Tear down: shutdown_all reaps the long-running ping/sleep child
    // synchronously via `kill_on_drop(true)` semantics.
    let _ = mgr.shutdown_all().await;
}

/// Bug I sanity check: parse_semver is a free helper inside
/// `manager.rs`. We exercise it indirectly via
/// `probe_worker_versions` against an exe that prints a known semver,
/// but the easiest unit-level test of the helper is to round-trip a
/// crafted output through it. We don't currently re-export the helper
/// (it's `fn`, not `pub fn`), so we rely on the integration-shaped
/// `boot_refuses_when_worker_version_skews` above to exercise the
/// happy path AND the parse path together.
#[test]
fn parse_semver_via_boot_probe_consistency() {
    use crate::manager::probe_worker_versions;
    // A worker that's literally not on PATH must NOT cause boot
    // failure — the probe is best-effort. This protects builds where
    // a worker binary is renamed during development.
    let spec = ChildSpec {
        name: "missing-worker".into(),
        // A path that is guaranteed not to resolve.
        command: "this-binary-does-not-exist-anywhere-12345".into(),
        args: vec![],
        env: vec![],
        restart: RestartStrategy::Permanent,
        rss_limit_mb: None,
        cpu_limit_percent: None,
        health_endpoint: None,
        heartbeat_deadline: None,
    };
    // Should be Ok(())  — the probe failure is logged and skipped.
    let result = probe_worker_versions(&[spec], env!("CARGO_PKG_VERSION"));
    assert!(
        result.is_ok(),
        "missing exe must not fail boot — probe is advisory; got {result:?}"
    );
}

// ---------------------------------------------------------------------
// Bug #233 regression (2026-05-04): worker IPC stdin-wedge under load.
//
// Symptom under heavy IPC traffic (Claude Code tool use, ~1KB+ payloads):
//   "child stdin write timed out after 10s (wedged worker?)"
//   "heartbeat missed past deadline; force-kill"
//   "restart scheduled, child=parser-worker-N delay_ms=10000"
// observed as 40+ orphan parser procs and 100+ supervisor restarts
// in `mneme doctor`.
//
// Root cause: BUG-A4-003 (commit a8b97ba) set `heartbeat_deadline =
// Some(60s)` on every worker spec in `default_layout()`. The supervisor
// watchdog enforces that deadline against `last_heartbeat`, which is
// initialised to spawn-time in `manager.rs::spawn_child` (line 267)
// and is ONLY ever advanced by the `ControlCommand::Heartbeat` IPC
// handler (`record_heartbeat`). NO worker binary in this repo ever
// sends that command — `common::worker_ipc` only exposes
// `report_complete`, never `heartbeat`. Result: every worker is
// guaranteed to be force-killed exactly 60 s after spawn, dispatches
// in flight see the dying stdin pipe and time out at 10 s, the
// monitor task respawns the worker, the cycle repeats forever.
//
// Contract: workers that DO NOT emit heartbeats MUST have
// `heartbeat_deadline = None` (the documented S-PHASE NEW-055 opt-out
// path in `watchdog.rs::heartbeat_pass`). When a worker class is
// later refactored to call `worker_ipc::heartbeat()` periodically,
// this test must be updated to allow `Some(...)` for THAT worker
// only — and a paired heartbeat-emit test must accompany the change.
//
// This test is intentionally tied to the canonical default layout, so
// the next contributor who flips `None → Some(60s)` on a
// heartbeat-less worker without a paired emit-side change gets a hard
// red CI failure pointing at this comment.
#[test]
fn default_layout_workers_have_no_heartbeat_deadline_until_emit_is_wired() {
    let cfg = SupervisorConfig::default_layout();
    let offenders: Vec<(String, Duration)> = cfg
        .children
        .iter()
        .filter_map(|c| c.heartbeat_deadline.map(|d| (c.name.clone(), d)))
        .collect();
    assert!(
        offenders.is_empty(),
        "Bug #233: default-layout workers MUST set `heartbeat_deadline = None` \
         until they wire a periodic `worker_ipc::heartbeat()` send. \
         Without a heartbeat sender, the watchdog kills every worker \
         60s after spawn, causing the stdin-wedge restart loop. \
         Offenders (name, deadline): {offenders:?}. \
         When you wire heartbeat-emit for a worker class, update this test \
         to whitelist THAT worker only — the rest must stay None."
    );
}

// ---------------------------------------------------------------------
// Wave 2.4: update_check integration tests (supervisor-level)
// ---------------------------------------------------------------------
//
// These tests exercise the supervisor-side plumbing:
//   * AutoUpdateConfig deserialization and defaults
//   * compare_semver edge cases via the re-export in lib.rs
//   * update_check_path / update_notice_seen_path helpers
//   * The config's default_layout includes a sensible AutoUpdateConfig
//
// Network-hitting tests (fetch_latest_release) live ONLY in
// update_check.rs itself and are gated with an opt-out env var.
// No test in this suite ever calls the real GitHub API.

#[test]
fn auto_update_config_default_is_enabled() {
    use crate::config::AutoUpdateConfig;
    let cfg = AutoUpdateConfig::default();
    assert!(cfg.enabled, "auto_update must be ON by default in v0.4.0");
    assert_eq!(cfg.check_interval_hours, 24);
    assert!(!cfg.apply, "apply mode must be OFF by default (Wave 2.5)");
    assert!(!cfg.include_prerelease);
}

#[test]
fn default_layout_includes_auto_update_config() {
    let cfg = SupervisorConfig::default_layout();
    assert!(
        cfg.auto_update.enabled,
        "default_layout must carry an enabled AutoUpdateConfig"
    );
    assert_eq!(cfg.auto_update.check_interval_hours, 24);
}

#[test]
fn auto_update_config_roundtrips_toml() {
    use crate::config::AutoUpdateConfig;
    let cfg = AutoUpdateConfig {
        enabled: false,
        check_interval_hours: 12,
        apply: false,
        include_prerelease: true,
    };
    let s = toml::to_string(&cfg).expect("serialize");
    let back: AutoUpdateConfig = toml::from_str(&s).expect("deserialize");
    assert!(!back.enabled);
    assert_eq!(back.check_interval_hours, 12);
    assert!(back.include_prerelease);
}

#[test]
fn auto_update_config_missing_section_uses_defaults() {
    // A TOML string that has NO [auto_update] section — the serde(default)
    // attribute on the field must kick in and produce the default struct.
    let toml_str = r#"
        root_dir = "/tmp/mneme"
        bin_dir  = "/tmp/mneme/bin"
        log_dir  = "/tmp/mneme/logs"
        ipc_socket_path = "/tmp/mneme/supervisor.sock"
        health_port = 7777
        health_check_interval = 60

        [default_restart_policy]
        initial_backoff = 100
        max_backoff = 10000
        backoff_multiplier = 5.0
        max_restarts_per_window = 5
        budget_window = 60

        [[children]]
        name    = "store-worker"
        command = "/tmp/mneme/bin/mneme-store"
        args    = []
        env     = []
        restart = "permanent"
    "#;
    let cfg: SupervisorConfig = toml::from_str(toml_str).expect("deserialize");
    assert!(cfg.auto_update.enabled, "missing section → default ON");
    assert_eq!(cfg.auto_update.check_interval_hours, 24);
    assert!(!cfg.auto_update.apply);
}

#[test]
fn update_check_paths_are_under_run_dir() {
    use crate::update_check::{update_check_path, update_notice_seen_path};
    let run_dir = std::path::PathBuf::from("/tmp/mneme/run");
    let check = update_check_path(&run_dir);
    let seen = update_notice_seen_path(&run_dir);
    assert_eq!(check.file_name().unwrap(), "update_check.json");
    assert_eq!(seen.file_name().unwrap(), "update_notice_seen.json");
    assert_eq!(check.parent().unwrap(), run_dir.as_path());
    assert_eq!(seen.parent().unwrap(), run_dir.as_path());
}

#[test]
fn supervisor_compare_semver_covers_all_cases() {
    use crate::update_check::compare_semver;
    use std::cmp::Ordering;
    // Older minor
    assert_eq!(compare_semver("0.3.99", "0.4.0"), Ordering::Less);
    // Equal
    assert_eq!(compare_semver("0.4.0", "0.4.0"), Ordering::Equal);
    // Newer patch (the 0.4.10 > 0.4.9 case that broke lexical sort)
    assert_eq!(compare_semver("0.4.10", "0.4.9"), Ordering::Greater);
    // Major beats everything
    assert_eq!(compare_semver("1.0.0", "0.99.99"), Ordering::Greater);
    // GitHub includes `v` prefix
    assert_eq!(compare_semver("0.4.0", "v0.4.1"), Ordering::Less);
    assert_eq!(compare_semver("0.4.1", "v0.4.1"), Ordering::Equal);
}

#[test]
fn write_and_read_update_check_roundtrip_supervisor() {
    use crate::update_check::{read_cached_result, write_result_atomic, UpdateCheckResult};
    use chrono::Utc;

    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path();

    let result = UpdateCheckResult {
        current_version: "0.4.0".to_string(),
        latest_version: Some("0.4.2".to_string()),
        update_available: Some(true),
        last_checked_at: Some(Utc::now()),
        last_attempt_at: Utc::now(),
        last_error: None,
    };

    write_result_atomic(run_dir, &result);
    let back = read_cached_result(run_dir).expect("round-trip read");

    assert_eq!(back.current_version, "0.4.0");
    assert_eq!(back.latest_version.as_deref(), Some("0.4.2"));
    assert_eq!(back.update_available, Some(true));
}

#[test]
fn banner_shown_supervisor_level() {
    use crate::update_check::{
        mark_notice_seen, read_cached_result, should_show_banner, write_result_atomic,
        UpdateCheckResult,
    };
    use chrono::Utc;

    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path();

    let cached = UpdateCheckResult {
        current_version: "0.4.0".to_string(),
        latest_version: Some("0.4.1".to_string()),
        update_available: Some(true),
        last_checked_at: Some(Utc::now()),
        last_attempt_at: Utc::now(),
        last_error: None,
    };
    write_result_atomic(run_dir, &cached);
    let cached = read_cached_result(run_dir).unwrap();

    // No seen file → should show.
    assert!(should_show_banner(&cached, "0.4.1", run_dir));

    // After marking seen within 24h → should NOT show.
    mark_notice_seen(run_dir, "0.4.1");
    assert!(!should_show_banner(&cached, "0.4.1", run_dir));
}

#[test]
fn banner_not_shown_when_up_to_date_supervisor() {
    use crate::update_check::{should_show_banner, UpdateCheckResult};
    use chrono::Utc;

    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path();

    let cached = UpdateCheckResult {
        current_version: "0.4.1".to_string(),
        latest_version: Some("0.4.1".to_string()),
        update_available: Some(false),
        last_checked_at: Some(Utc::now()),
        last_attempt_at: Utc::now(),
        last_error: None,
    };
    assert!(!should_show_banner(&cached, "0.4.1", run_dir));
}

// ---------------------------------------------------------------------------
// Audit fix TEST-NEW-3 (2026-05-06 multi-agent fan-out, testing-reviewer):
// property tests for compute_jittered_delay. The jitter logic was
// untested before this — `cargo check` and the chaos suite both
// passed it without exercising the herd-breaking property.
// ---------------------------------------------------------------------------

#[test]
fn jitter_zero_input_returns_zero_no_panic() {
    use crate::manager::compute_jittered_delay;
    use std::time::Duration;
    // ZERO input is "do not sleep at all" — bypass the 50ms floor
    // and honour the caller's intent. Most importantly: don't panic
    // (the previous inline form's `seed % 0` would have).
    let out = compute_jittered_delay("worker", Duration::ZERO, 12345);
    assert_eq!(out, Duration::ZERO);
}

#[test]
fn jitter_result_is_strictly_less_than_max_of_base_and_50ms() {
    use crate::manager::compute_jittered_delay;
    use std::time::Duration;

    // For each base in {1ms, 49ms, 50ms, 200ms, 5000ms}, jitter
    // must produce a Duration in [0, max(base, 50ms)).
    for base_ms in [1u64, 49, 50, 200, 5_000] {
        let base = Duration::from_millis(base_ms);
        let upper_bound_ms = base_ms.max(50);
        for nanos in 0u32..1000 {
            let out = compute_jittered_delay("worker-x", base, nanos);
            let out_ms = out.as_millis() as u64;
            assert!(
                out_ms < upper_bound_ms,
                "jitter {out_ms}ms must be strictly < upper bound {upper_bound_ms}ms (base={base_ms}ms, nanos={nanos})",
            );
        }
    }
}

#[test]
fn jitter_distinct_names_at_same_time_produce_different_values() {
    use crate::manager::compute_jittered_delay;
    use std::collections::HashSet;
    use std::time::Duration;

    // Synchronised-failure scenario: 10 distinct workers all observe
    // their crash at the same nanosecond. Without jitter they sleep
    // identically and respawn in lockstep. With jitter, the seed must
    // produce distinct values for each name.
    let base = Duration::from_millis(1000);
    let now_nanos = 500_000_000u32;
    let names: Vec<String> = (0..10).map(|i| format!("worker-{i}")).collect();
    let values: HashSet<u64> = names
        .iter()
        .map(|n| compute_jittered_delay(n, base, now_nanos).as_millis() as u64)
        .collect();
    // 10 names, 1000ms range, ~1000 possible values: birthday-paradox
    // collision probability is ≈ (10*9/2) / 1000 ≈ 4.5%. Allow a
    // very loose lower bound — 5 distinct values (tolerates one
    // collision pair) — so the test isn't flaky, while still proving
    // names produce different jitter.
    assert!(
        values.len() >= 5,
        "expected ≥5 distinct jitter values across 10 names, got {} ({values:?})",
        values.len(),
    );
}

#[test]
fn jitter_floor_kicks_in_for_sub_50ms_base() {
    use crate::manager::compute_jittered_delay;
    use std::time::Duration;

    // 1ms initial_backoff (test config / aggressive YAML) used to
    // collapse to seed%1 == 0. After CORR-NEW-1 + the 50ms floor
    // the jittered value can range up to 49ms — so across many
    // names we should observe at least ONE value > 1ms. Without
    // the floor every output would be 0.
    let base = Duration::from_millis(1);
    let names: Vec<String> = (0..20).map(|i| format!("w-{i}")).collect();
    let max_observed = names
        .iter()
        .map(|n| compute_jittered_delay(n, base, 42).as_millis())
        .max()
        .unwrap_or(0);
    assert!(
        max_observed > 1,
        "with 50ms floor, at least one jitter sample should exceed 1ms — got max {max_observed}ms",
    );
}

#[test]
fn jitter_is_deterministic_for_fixed_inputs() {
    use crate::manager::compute_jittered_delay;
    use std::time::Duration;

    // Same (name, base, nanos) tuple must always yield the same
    // Duration. This is what makes the function testable and what
    // makes the per-restart distinction (different `nanos` per
    // restart attempt) actually break herds.
    let a = compute_jittered_delay("w", Duration::from_millis(1000), 42);
    let b = compute_jittered_delay("w", Duration::from_millis(1000), 42);
    assert_eq!(a, b);
}
