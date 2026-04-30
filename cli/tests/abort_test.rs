//! Integration tests for `mneme abort`.
//!
//! These tests exercise the in-process `commands::abort` API directly
//! with a tempdir-isolated project layout (MNEME_HOME pointed at the
//! tempdir) and a real spawned child process we abort. We deliberately
//! do NOT depend on `cargo build` having materialised the `mneme`
//! binary first — these tests run as part of `cargo test -p mneme-cli`.
//!
//! Isolation pattern matches `cli/tests/install_writes_standalone_uninstaller.rs`:
//! an `env_lock` Mutex serialises within this binary so concurrent
//! tests don't stomp on each other's MNEME_HOME, and an `EnvSnapshot`
//! restores the original env on drop.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Serial-env harness
// ---------------------------------------------------------------------------

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvSnapshot {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvSnapshot {
    fn capture(keys: &[&'static str]) -> Self {
        let saved = keys
            .iter()
            .map(|k| (*k, std::env::var_os(k)))
            .collect::<Vec<_>>();
        EnvSnapshot { saved }
    }
}

impl Drop for EnvSnapshot {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            // Safety: env_lock() Mutex held by caller for the full body.
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
    }
}

const ENV_KEYS: &[&str] = &["MNEME_HOME"];

fn isolate_home(tempdir: &Path) -> EnvSnapshot {
    let snap = EnvSnapshot::capture(ENV_KEYS);
    // Safety: env_lock() held by caller.
    unsafe {
        std::env::set_var("MNEME_HOME", tempdir);
    }
    snap
}

/// Spawn a long-running subprocess we can later abort. We use the
/// platform's native sleep so the test does not depend on `cargo build`
/// having materialised any other binary first.
///
/// Returns the live `Child`. Its PID is the value we'll write into the
/// fixture's `.lock` file.
fn spawn_sleeper() -> std::process::Child {
    if cfg!(windows) {
        // `cmd /c timeout /t 60 /nobreak` blocks 60s. We redirect stdio
        // so the child has no console handles attached.
        Command::new("cmd")
            .args(["/c", "timeout", "/t", "60", "/nobreak"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cmd timeout sleeper")
    } else {
        Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep sleeper")
    }
}

/// Layout: `tempdir/projects/<id>/{.lock, graph.db, history.db}`.
/// Returns the absolute path to the project directory.
fn build_fake_project(temp: &TempDir, id: &str, pid: u32) -> PathBuf {
    let project_dir = temp.path().join("projects").join(id);
    fs::create_dir_all(&project_dir).unwrap();

    // Write the .lock in BuildLock format.
    let lock = project_dir.join(".lock");
    fs::write(
        &lock,
        format!("pid={pid} ts=1700000000 project={id}\n"),
    )
    .unwrap();

    // Drop a couple of populated SQLite DB files so checkpoint_shards
    // has something to run TRUNCATE on. Even one row in WAL mode forces
    // a non-trivial -wal sidecar that we can later assert was flushed.
    for shard_name in ["graph.db", "history.db"] {
        let db_path = project_dir.join(shard_name);
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch("CREATE TABLE t (k INTEGER PRIMARY KEY); INSERT INTO t (k) VALUES (1);")
            .unwrap();
        drop(conn);
    }

    project_dir
}

/// Try to wait for the child to exit, polling try_wait every 100ms up
/// to 3s. Returns true if the kernel reaped it.
fn wait_for_child_reap(child: &mut std::process::Child) -> bool {
    for _ in 0..30 {
        if let Ok(Some(_)) = child.try_wait() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn abort_kills_real_child_and_cleans_up_lock() {
    let lock = env_lock().lock().unwrap();

    let temp = TempDir::new().unwrap();
    let _snap = isolate_home(temp.path());

    let mut child = spawn_sleeper();
    let pid = child.id();

    let project_dir = build_fake_project(&temp, "fixture-real-child", pid);
    let lock_path = project_dir.join(".lock");
    assert!(lock_path.exists());

    use mneme_cli::commands::abort;
    let args = abort::AbortArgs {
        project: None,
        all: true,
        force: false,
        timeout_secs: 6,
    };
    let result = abort::run(args).await;
    result.expect("abort should succeed");

    let reaped = wait_for_child_reap(&mut child);
    if !reaped {
        // Failsafe — kill it ourselves so the test runner doesn't leak.
        let _ = child.kill();
        let _ = child.wait();
    }
    assert!(reaped, "child process should have been reaped after abort");

    assert!(
        !lock_path.exists(),
        "abort should have removed .lock at {}",
        lock_path.display()
    );

    drop(lock);
}

#[tokio::test]
async fn abort_with_stale_pid_only_cleans_up() {
    let lock = env_lock().lock().unwrap();

    let temp = TempDir::new().unwrap();
    let _snap = isolate_home(temp.path());

    let project_dir = build_fake_project(&temp, "fixture-stale-pid", u32::MAX);
    let lock_path = project_dir.join(".lock");
    assert!(lock_path.exists());

    use mneme_cli::commands::abort;
    let result = abort::run(abort::AbortArgs {
        project: None,
        all: true,
        force: false,
        timeout_secs: 1,
    })
    .await;
    result.expect("abort should succeed on stale lock");
    assert!(!lock_path.exists(), "stale lock should be removed");

    drop(lock);
}

#[tokio::test]
async fn abort_with_force_skips_grace_period() {
    let lock = env_lock().lock().unwrap();

    let temp = TempDir::new().unwrap();
    let _snap = isolate_home(temp.path());

    let mut child = spawn_sleeper();
    let pid = child.id();

    let project_dir = build_fake_project(&temp, "fixture-force", pid);
    let lock_path = project_dir.join(".lock");

    use mneme_cli::commands::abort;
    let started = std::time::Instant::now();
    let result = abort::run(abort::AbortArgs {
        project: None,
        all: true,
        force: true,
        timeout_secs: 30, // big budget — force should ignore it
    })
    .await;
    let elapsed = started.elapsed();
    result.expect("abort --force should succeed");

    let reaped = wait_for_child_reap(&mut child);
    if !reaped {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert!(reaped, "child should be reaped after --force abort");

    // Force should NOT wait full 30s. With ~500ms settle window plus
    // IO it tops out around ~3s on a healthy machine; certainly << 15s.
    assert!(
        elapsed < Duration::from_secs(15),
        "--force should not wait full timeout; elapsed {elapsed:?}"
    );

    assert!(
        !lock_path.exists(),
        "force abort should still remove .lock"
    );

    drop(lock);
}

#[tokio::test]
async fn abort_no_lock_file_is_no_op() {
    let lock = env_lock().lock().unwrap();

    let temp = TempDir::new().unwrap();
    let _snap = isolate_home(temp.path());

    // Project dir exists but no .lock inside.
    let project_dir = temp.path().join("projects").join("fixture-empty");
    fs::create_dir_all(&project_dir).unwrap();

    use mneme_cli::commands::abort;
    let result = abort::run(abort::AbortArgs {
        project: None,
        all: true,
        force: false,
        timeout_secs: 1,
    })
    .await;
    result.expect("abort over empty project should succeed silently");

    drop(lock);
}
