// env_lock std::sync::Mutex held across .await to serialize env mutation.
#![allow(
    clippy::await_holding_lock,
    clippy::doc_overindented_list_items,
    clippy::doc_lazy_continuation
)]

//! QA-6 (Wave 4 cleanup): end-to-end fixture for [`HookCtx`] writers.
//!
//! Each test:
//!   1. Carves out an isolated `~/.mneme` rooted at a `tempfile::tempdir()`
//!      (so we never touch the developer's real shard).
//!   2. Lays down a project marker (`.git/`) in a tempdir-nested
//!      `project/` so [`HookCtx::resolve`] succeeds and [`store::Store`]
//!      lazy-creates the per-project shard inside the isolated home.
//!   3. Calls one of the four `HookCtx::write_*` methods.
//!   4. Opens the resulting SQLite shard read-only with `rusqlite` and
//!      asserts the expected row landed.
//!
//! ## Why integration, not unit
//!
//! The four `write_*` methods in `cli::hook_writer` do **two** things:
//! they try the supervisor first, then on any IPC failure fall through to
//! a direct-DB write. A unit test that mocks `Store` proves nothing about
//! the SQL actually executing — the persistent-memory regression that
//! flagged 23/26 shards EMPTY in the v0.3.0 audit (Bucket B4) was
//! diagnosed by *exactly this kind of round-trip*: write, then read the
//! shard and look for the row. This fixture is the regression net for
//! that bucket.
//!
//! ## IPC isolation
//!
//! `HookCtx::write_*` always tries the supervisor IPC first. With a real
//! daemon running on the dev machine (`~/.mneme/supervisor.pipe` exists
//! and points at a live `\\.\pipe\mneme-supervisor-<pid>`), an unisolated
//! test would write rows into the developer's actual shard tree instead
//! of the tempdir. The env-var trio below — `USERPROFILE` / `HOME` /
//! `MNEME_HOME` / `MNEME_RUNTIME_DIR` — re-points every discovery the IPC
//! client performs at the same tempdir, so the supervisor probe fails
//! fast (no pipe file, no socket file, no listener) and the writer falls
//! through to the direct-DB path under test.
//!
//! ## Serial execution
//!
//! Cargo runs tests within one binary in parallel. Mutating process env
//! vars (`USERPROFILE` / `MNEME_HOME` / etc.) is process-global, so two
//! parallel tests would race and clobber each other's isolation. We
//! serialize via a static `Mutex` (the same `env_lock` pattern used in
//! `supervisor/src/tests.rs` — one fewer dependency than `serial_test`).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use mneme_cli::hook_writer::HookCtx;
use rusqlite::{Connection, OpenFlags};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Serial-env harness — the supervisor crate's pattern, reused.
// ---------------------------------------------------------------------------

/// Process-wide env-mutation lock. Tests in this binary that mutate
/// `MNEME_HOME` / `USERPROFILE` / `HOME` / `MNEME_RUNTIME_DIR` MUST hold
/// this guard for the full duration of the env override. Cargo's default
/// parallel-tests model would otherwise race two harnesses through
/// `unsafe set_var` and corrupt the isolation.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Snapshot of the env state we mutate. Held across the test body and
/// restored on drop so a failing test doesn't leak overrides into a
/// sibling.
struct EnvSnapshot {
    keys: &'static [&'static str],
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvSnapshot {
    fn capture(keys: &'static [&'static str]) -> Self {
        let saved = keys
            .iter()
            .map(|k| (*k, std::env::var_os(k)))
            .collect::<Vec<_>>();
        EnvSnapshot { keys, saved }
    }
}

impl Drop for EnvSnapshot {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            // Safety: env_lock() guarantees no parallel test is reading
            // or mutating these keys for the duration of the test body.
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        let _ = self.keys; // silence dead-code lint when keys list grows.
    }
}

/// Mutated env keys this fixture owns:
///
/// - `MNEME_HOME`         — `PathManager::default_root()` reads this first.
/// - `MNEME_RUNTIME_DIR`  — `mneme_cli::runtime_dir()` reads this for the
///                          IPC socket fallback path.
/// - `USERPROFILE`        — Windows `dirs::home_dir()` source.
/// - `HOME`               — Unix `dirs::home_dir()` source. The IPC client's
///                          `default_path()` consults `home_dir().join(".mneme/
///                          supervisor.pipe")` BEFORE the runtime_dir
///                          fallback — without overriding home, an unisolated
///                          test would discover and write to the live
///                          supervisor pipe.
const ENV_KEYS: &[&str] = &["MNEME_HOME", "MNEME_RUNTIME_DIR", "USERPROFILE", "HOME"];

/// Apply the four-key isolation override. The `tempdir` becomes the
/// virtual home; `tempdir/.mneme/` becomes `MNEME_HOME`; `tempdir/.mneme/
/// run/` becomes `MNEME_RUNTIME_DIR`. Caller is responsible for dropping
/// the returned [`EnvSnapshot`] last (Rust drops in reverse declaration
/// order, so declare `_snap` after `_guard` in the test body).
fn isolate_env(tempdir: &Path) -> EnvSnapshot {
    let snap = EnvSnapshot::capture(ENV_KEYS);
    let mneme_home = tempdir.join(".mneme");
    let runtime_dir = mneme_home.join("run");
    // Safety: env_lock() Mutex held by caller.
    unsafe {
        std::env::set_var("USERPROFILE", tempdir);
        std::env::set_var("HOME", tempdir);
        std::env::set_var("MNEME_HOME", &mneme_home);
        std::env::set_var("MNEME_RUNTIME_DIR", &runtime_dir);
    }
    snap
}

/// Carve out an isolated home + a project root with a `.git/` marker.
/// Returns `(tempdir, project_root)`. `tempdir` MUST stay alive for the
/// test body — dropping it removes the shard and the assertions race
/// against fs cleanup.
fn fresh_project() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().join("project");
    std::fs::create_dir_all(project_root.join(".git"))
        .expect("create project_root with .git marker");
    (dir, project_root)
}

/// Compute the shard file path the way `PathManager::shard_db` would.
/// We re-derive from `MNEME_HOME` + `ProjectId::from_path` instead of
/// reaching back into `HookCtx.store.paths` to keep the assertion
/// independent of the writer's plumbing — if the helper drifts out of
/// sync with the production path layout, this test fails first.
fn shard_path(mneme_home: &Path, project_root: &Path, db: &str) -> PathBuf {
    let pid = common::ids::ProjectId::from_path(project_root)
        .expect("hash project path for shard lookup");
    mneme_home.join("projects").join(pid.as_str()).join(db)
}

/// Open a per-project shard READ-ONLY. The writer task may still hold a
/// connection internally; opening read-only avoids any chance of a
/// `database is locked` race against the writer's idempotent
/// build_or_migrate path.
fn open_ro(path: &Path) -> Connection {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap_or_else(|e| panic!("open_ro({}): {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Test 1: write_turn → history.db::turns
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hook_writer_writes_turn_to_history_db() {
    let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (tmp, project_root) = fresh_project();
    let _snap = isolate_env(tmp.path());

    // build_or_migrate runs under HookCtx::resolve.
    let ctx = HookCtx::resolve(&project_root)
        .await
        .expect("HookCtx::resolve succeeds inside isolated MNEME_HOME");

    // Sanity: the resolver landed inside our tempdir.
    let mneme_home = tmp.path().join(".mneme");
    let history = shard_path(&mneme_home, &project_root, "history.db");
    assert!(
        history.exists(),
        "history.db must be created by build_or_migrate at {}",
        history.display()
    );

    // Act.
    ctx.write_turn("sess-qa6-turn", "user", "hello world from qa6")
        .await
        .expect("write_turn falls through to direct-DB and succeeds");

    // Assert: row landed in turns table.
    let conn = open_ro(&history);
    let (session_id, role, content): (String, String, String) = conn
        .query_row(
            "SELECT session_id, role, content FROM turns WHERE session_id = ?1",
            ["sess-qa6-turn"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("turns row not found");
    assert_eq!(session_id, "sess-qa6-turn");
    assert_eq!(role, "user");
    assert_eq!(content, "hello world from qa6");
}

// ---------------------------------------------------------------------------
// Test 2: write_ledger_entry → tasks.db::ledger_entries
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hook_writer_writes_ledger_entry_to_tasks_db() {
    let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (tmp, project_root) = fresh_project();
    let _snap = isolate_env(tmp.path());

    let ctx = HookCtx::resolve(&project_root)
        .await
        .expect("HookCtx::resolve succeeds inside isolated MNEME_HOME");

    let mneme_home = tmp.path().join(".mneme");
    let tasks = shard_path(&mneme_home, &project_root, "tasks.db");
    assert!(tasks.exists(), "tasks.db must exist after build_or_migrate");

    // Act.
    ctx.write_ledger_entry(
        "sess-qa6-ledger",
        "decision",
        "qa6 ledger smoke summary",
        Some("qa6 rationale"),
    )
    .await
    .expect("write_ledger_entry falls through to direct-DB and succeeds");

    // Assert: row landed.
    let conn = open_ro(&tasks);
    let (session_id, kind, summary, rationale): (String, String, String, String) = conn
        .query_row(
            "SELECT session_id, kind, summary, rationale FROM ledger_entries \
             WHERE session_id = ?1",
            ["sess-qa6-ledger"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("ledger_entries row not found");
    assert_eq!(session_id, "sess-qa6-ledger");
    assert_eq!(kind, "decision");
    assert_eq!(summary, "qa6 ledger smoke summary");
    assert_eq!(rationale, "qa6 rationale");
}

// ---------------------------------------------------------------------------
// Test 3: write_tool_call → tool_cache.db::tool_calls
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hook_writer_writes_tool_call_to_tool_cache_db() {
    let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (tmp, project_root) = fresh_project();
    let _snap = isolate_env(tmp.path());

    let ctx = HookCtx::resolve(&project_root)
        .await
        .expect("HookCtx::resolve succeeds inside isolated MNEME_HOME");

    let mneme_home = tmp.path().join(".mneme");
    let tool_cache = shard_path(&mneme_home, &project_root, "tool_cache.db");
    assert!(
        tool_cache.exists(),
        "tool_cache.db must exist after build_or_migrate"
    );

    // Act.
    let params_json = r#"{"file":"/tmp/x.txt"}"#;
    let result_json = r#"{"ok":true}"#;
    ctx.write_tool_call("sess-qa6-tool", "Read", params_json, result_json)
        .await
        .expect("write_tool_call falls through to direct-DB and succeeds");

    // Assert: row landed with the params we asked for.
    let conn = open_ro(&tool_cache);
    let (tool, session_id, params, result): (String, String, String, String) = conn
        .query_row(
            "SELECT tool, session_id, params, result FROM tool_calls \
             WHERE session_id = ?1",
            ["sess-qa6-tool"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("tool_calls row not found");
    assert_eq!(tool, "Read");
    assert_eq!(session_id, "sess-qa6-tool");
    assert_eq!(params, params_json);
    assert_eq!(result, result_json);
}

// ---------------------------------------------------------------------------
// Test 4: write_file_event → livestate.db::file_events
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hook_writer_writes_file_event_to_livestate_db() {
    let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (tmp, project_root) = fresh_project();
    let _snap = isolate_env(tmp.path());

    let ctx = HookCtx::resolve(&project_root)
        .await
        .expect("HookCtx::resolve succeeds inside isolated MNEME_HOME");

    let mneme_home = tmp.path().join(".mneme");
    let livestate = shard_path(&mneme_home, &project_root, "livestate.db");
    assert!(
        livestate.exists(),
        "livestate.db must exist after build_or_migrate"
    );

    // Act.
    ctx.write_file_event("/tmp/qa6/touched.txt", "edit", "claude")
        .await
        .expect("write_file_event falls through to direct-DB and succeeds");

    // Assert: row landed.
    let conn = open_ro(&livestate);
    let (file_path, event_type, actor): (String, String, String) = conn
        .query_row(
            "SELECT file_path, event_type, actor FROM file_events \
             WHERE file_path = ?1",
            ["/tmp/qa6/touched.txt"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("file_events row not found");
    assert_eq!(file_path, "/tmp/qa6/touched.txt");
    assert_eq!(event_type, "edit");
    assert_eq!(actor, "claude");
}
