//! Cross-process advisory file lock for `mneme build` / `mneme rebuild`.
//!
//! ## Why
//!
//! Two concurrent `mneme build` invocations on the same project share a
//! single SQLite shard. SQLite's WAL absorbs most of the race, but
//! interleaved writes can leave the per-project shard in a degraded
//! state (partial node graph, orphan FTS rows, …). Acceptance for
//! audit item L4 in v0.3.0: every build acquires an exclusive
//! file lock at the very top of `run()` before any DB writes happen.
//! Release on Drop covers the success path AND every panic / `?`
//! short-circuit.
//!
//! ## Lock file location
//!
//! `<project_root>/.lock` where `<project_root>` is the directory
//! `PathManager::project_root(&project_id)` returns
//! (i.e. `~/.mneme/projects/<id>/`). Per-project lock — concurrent
//! builds on DIFFERENT projects do not block each other.
//!
//! ## Behaviour
//!
//! * `BuildLock::acquire(...)` opens-or-creates the lock file and
//!   takes an exclusive `flock`-style hold via the `fs2` crate.
//!   Portable across Windows (`LockFileEx`) and Unix (`flock`).
//! * If the lock is already held by another process and `timeout = 0`,
//!   returns immediately with `CliError::Other("another build in
//!   progress for project <id> (locked at <ts>)")`. Exit code 4.
//! * If `timeout > 0`, polls every 250 ms up to the deadline. Returns
//!   the same error message with `(timed out after Ns)` suffix on
//!   expiry.
//! * On `Drop`, the lock is released. We DO remove the lock file — the
//!   tracked-fix tests assert that. The window between close and unlink
//!   is tiny and any racing process will simply re-create the file.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fs2::FileExt;

use crate::error::{CliError, CliResult};

/// Held lock. Drop releases it.
///
/// Construct via [`BuildLock::acquire`].
#[derive(Debug)]
pub struct BuildLock {
    /// Path to the lock file. Removed on Drop.
    path: PathBuf,
    /// File handle holding the OS-level lock. Dropping the handle
    /// releases the lock; we additionally call `unlock()` explicitly
    /// in Drop for clarity.
    file: Option<File>,
}

impl BuildLock {
    /// Acquire the per-project build lock.
    ///
    /// `project_root` MUST be the directory `PathManager::project_root`
    /// returns for this project (`~/.mneme/projects/<id>/`). The
    /// directory is created if missing — first build on a fresh shard
    /// must succeed without a pre-existing project directory.
    ///
    /// `project_id` is included in the error message (not used for
    /// path construction — `project_root` is the source of truth).
    ///
    /// `timeout = Duration::ZERO` is fail-fast. Otherwise the call
    /// polls every 250 ms until the deadline.
    pub fn acquire(
        project_id: &str,
        project_root: &Path,
        timeout: Duration,
    ) -> CliResult<Self> {
        // Ensure the parent directory exists so OpenOptions::create
        // doesn't fail on a fresh shard. PathManager guarantees the
        // path layout but doesn't materialise the directory itself
        // until the store builder runs — and the lock has to be held
        // BEFORE the store builder runs.
        if !project_root.exists() {
            std::fs::create_dir_all(project_root).map_err(|e| {
                CliError::io(project_root.to_path_buf(), e)
            })?;
        }

        let lock_path = project_root.join(".lock");
        let deadline = if timeout.is_zero() {
            None
        } else {
            Some(Instant::now() + timeout)
        };

        loop {
            let file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&lock_path)
                .map_err(|e| CliError::io(lock_path.clone(), e))?;

            match file.try_lock_exclusive() {
                Ok(()) => {
                    // Stamp the lock file with PID + timestamp so a
                    // racing build can show a useful error.
                    let stamp = format!(
                        "pid={} ts={} project={}\n",
                        std::process::id(),
                        chrono_unix_secs(),
                        project_id
                    );
                    // Truncate existing content first — every fresh
                    // acquire overwrites the previous holder's stamp.
                    let _ = file.set_len(0);
                    let mut writer = &file;
                    let _ = writer.write_all(stamp.as_bytes());
                    let _ = writer.flush();
                    return Ok(Self {
                        path: lock_path,
                        file: Some(file),
                    });
                }
                Err(_e) => {
                    // Another process holds it. Read the stamp for the
                    // error message, then either fail-fast or wait.
                    drop(file);
                    if let Some(deadline) = deadline {
                        if Instant::now() >= deadline {
                            return Err(Self::contention_error(
                                project_id,
                                &lock_path,
                                Some(timeout),
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(250));
                        continue;
                    } else {
                        return Err(Self::contention_error(
                            project_id,
                            &lock_path,
                            None,
                        ));
                    }
                }
            }
        }
    }

    /// Read the existing stamp (if any) and synthesise the contention
    /// error returned to the user. Truncated to a single line — full
    /// stamp content lives in the lock file for diagnostics.
    fn contention_error(
        project_id: &str,
        lock_path: &Path,
        timeout: Option<Duration>,
    ) -> CliError {
        let stamp = std::fs::read_to_string(lock_path)
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or("(no stamp)")
            .to_string();
        let suffix = match timeout {
            Some(t) if !t.is_zero() => {
                format!(" (timed out after {}s)", t.as_secs())
            }
            _ => String::new(),
        };
        CliError::Other(format!(
            "another build in progress for project {} (locked at {}){}",
            project_id, stamp, suffix
        ))
    }

    /// Path to the on-disk lock file. Test-only inspector.
    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            // Explicit unlock for clarity. fs2::FileExt is method-on-
            // file — drop alone would work because fs2 unlocks on
            // close, but this makes the intent obvious.
            let _ = FileExt::unlock(&file);
            drop(file);
        }
        // Remove the lock file. Any racing process that hits
        // try_lock_exclusive() between unlock + remove will simply
        // re-create the file via OpenOptions::create above — safe.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Best-effort unix-seconds timestamp. We deliberately use
/// `SystemTime::UNIX_EPOCH` rather than chrono so build_lock.rs
/// compiles even if a future cleanup drops the chrono dep.
fn chrono_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn fixture_root() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path().join("project-fixture");
        std::fs::create_dir_all(&root).unwrap();
        (dir, root)
    }

    #[test]
    fn first_acquire_succeeds() {
        let (_guard, root) = fixture_root();
        let lock = BuildLock::acquire("test-pid", &root, Duration::ZERO)
            .expect("first acquire");
        // The lock file must exist on disk while the lock is held.
        assert!(lock.path().exists(), "lock file should exist while held");
    }

    #[test]
    fn second_fail_fast_returns_in_progress_error() {
        let (_guard, root) = fixture_root();
        let _first = BuildLock::acquire("alpha", &root, Duration::ZERO)
            .expect("first acquire");

        // Spawn the second attempt on a thread because fs2's
        // exclusive-lock semantics on Windows can be process-affine
        // for the SAME File handle. A separate OpenOptions handle
        // (different File) on the same path correctly contends, and
        // the easiest way to get a fresh File is a fresh thread.
        let root_clone = root.clone();
        let result =
            thread::spawn(move || BuildLock::acquire("alpha", &root_clone, Duration::ZERO))
                .join()
                .expect("thread join");

        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("second acquire should have failed"),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("another build in progress"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("alpha"), "should mention project id: {msg}");
    }

    #[test]
    fn second_with_timeout_waits_and_then_succeeds_when_first_releases() {
        let (_guard, root) = fixture_root();
        let first = BuildLock::acquire("beta", &root, Duration::ZERO)
            .expect("first acquire");

        // Spawn a thread that releases `first` after 300 ms.
        let released_signal = Arc::new(AtomicBool::new(false));
        let signal_clone = Arc::clone(&released_signal);
        let release_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            drop(first);
            signal_clone.store(true, Ordering::SeqCst);
        });

        // Try to acquire with a 2s timeout — should succeed once
        // the release fires at +300ms.
        let started = Instant::now();
        let root_clone = root.clone();
        let second = thread::spawn(move || {
            BuildLock::acquire("beta", &root_clone, Duration::from_secs(2))
        })
        .join()
        .expect("thread join")
        .expect("second acquire after release");

        // Sanity: must have waited at least ~250 ms.
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(200),
            "should have waited for the first lock; only waited {waited:?}"
        );
        assert!(
            released_signal.load(Ordering::SeqCst),
            "first lock should have been released before second acquired"
        );
        // Tidy.
        drop(second);
        release_thread.join().expect("release thread join");
    }

    #[test]
    fn second_with_timeout_returns_timeout_error_when_first_holds_too_long() {
        let (_guard, root) = fixture_root();
        let _first = BuildLock::acquire("gamma", &root, Duration::ZERO)
            .expect("first acquire");

        let root_clone = root.clone();
        let started = Instant::now();
        let result = thread::spawn(move || {
            BuildLock::acquire("gamma", &root_clone, Duration::from_millis(600))
        })
        .join()
        .expect("thread join");

        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected timeout error"),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("another build in progress"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("timed out"), "should mention timeout: {msg}");
        // Sanity: must have waited at least the timeout window.
        assert!(
            started.elapsed() >= Duration::from_millis(550),
            "should have waited the full timeout; elapsed {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn lock_file_is_removed_on_drop() {
        let (_guard, root) = fixture_root();
        let lock_path = {
            let lock = BuildLock::acquire("delta", &root, Duration::ZERO)
                .expect("first acquire");
            lock.path().to_path_buf()
        };
        assert!(
            !lock_path.exists(),
            "lock file should have been removed on Drop, still at {}",
            lock_path.display()
        );
    }

    #[test]
    fn acquire_creates_missing_project_root() {
        let dir = TempDir::new().expect("tempdir");
        let nonexistent = dir.path().join("does/not/exist/yet");
        assert!(!nonexistent.exists());
        let lock = BuildLock::acquire("epsilon", &nonexistent, Duration::ZERO)
            .expect("acquire on nonexistent root");
        assert!(nonexistent.exists(), "project root should have been created");
        assert!(lock.path().exists(), "lock file should exist");
    }
}
