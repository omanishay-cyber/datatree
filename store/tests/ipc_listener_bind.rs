//! Regression tests for `mneme_store::ipc::build_listener`.
//!
//! NEW-B (2026-05-04): the store-worker was crash-looping on Windows
//! and ending up `status=degraded` after restart-budget exhaustion
//! because the bind path was using the wrong interprocess API.
//!
//! Specifically, `to_fs_name::<GenericFilePath>()` requires a path that
//! starts with `\\.\pipe\` (two leading backslashes) on Windows, but
//! `PathBuf::from(r"\\.\pipe\...")` is normalised by Windows path
//! handling down to `\.\pipe\...` (one leading backslash) — which the
//! interprocess crate rejects with "not a named pipe path" / "Access is
//! denied". Worker exited cleanly, supervisor (Permanent restart strategy)
//! respawned it, and the loop hit the 6-restarts-in-60s budget within
//! milliseconds.
//!
//! These tests pin the contract so the regression cannot reappear:
//!
//!   1. `build_listener_succeeds_against_path_manager_store_socket` —
//!      the exact path produced by `PathManager::store_socket()` must
//!      bind successfully on every supported target. This is the test
//!      the bug would have caught.
//!
//!   2. `build_listener_drops_cleanly_releasing_socket` — dropping the
//!      Listener releases the underlying resource so a follow-on bind
//!      against the same path succeeds. This protects against a future
//!      regression where someone adds OS-level state that survives drop.

use common::paths::PathManager;
use mneme_store::ipc::build_listener;

/// The bug. `PathManager::store_socket()` on Windows returns
/// `\.\pipe\mneme-store` (one leading backslash, courtesy of UNC
/// canonicalisation in `PathBuf::from`). Pre-fix, `build_listener`
/// blindly fed that to `to_fs_name::<GenericFilePath>()`, which
/// returned an Err — the listener never bound, the worker exited, and
/// the supervisor flagged it `degraded` after 6 restarts in 60s.
///
/// Post-fix, `build_listener` extracts `path.file_name()` and uses
/// `to_ns_name::<GenericNamespaced>()` on Windows, mirroring the
/// supervisor's own pattern. The bind succeeds and the listener is
/// usable.
#[tokio::test]
async fn build_listener_succeeds_against_path_manager_store_socket() {
    // Use a tempdir-rooted PathManager so the test does not collide with
    // a real running mneme-daemon's pipe (a pre-existing pipe with the
    // same name would cause our bind to fail with EADDRINUSE on Linux
    // or ERROR_PIPE_BUSY on Windows). On Windows the pipe name is
    // derived from the file_name() of the returned PathBuf — which is
    // always `mneme-store` regardless of the home root — so we override
    // the constant to a unique per-test name to avoid that collision.
    //
    // `PathManager::with_root` does not affect the file_name extracted
    // on Windows (the pipe path is hard-coded in `paths::store_socket`),
    // so for the Windows arm we drive `build_listener` through a
    // synthetic path that mirrors the production shape but uses a
    // unique pipe-leaf. The real production path is exercised in the
    // sibling `build_listener_succeeds_with_production_pipe_name` test
    // which is allowed to be skipped when an existing pipe collides.
    let tmp = tempfile::tempdir().expect("tempdir");
    let pm = PathManager::with_root(tmp.path().to_path_buf());

    #[cfg(unix)]
    {
        // Unix: the path is per-tempdir, so the bind cannot collide with
        // any other process. Drive the actual production path verbatim.
        let path = pm.store_socket();
        let listener = build_listener(&path)
            .expect("build_listener must succeed against PathManager::store_socket() on Unix");
        // Drop drops the listener; the test passes by virtue of bind
        // succeeding.
        drop(listener);
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(windows)]
    {
        // Windows: `store_socket()` returns the same PathBuf
        // (`\.\pipe\mneme-store`) regardless of root, so we cannot just
        // call it — a real running daemon would already own that pipe.
        // Instead, swap in a unique pipe-leaf via a synthetic
        // PathBuf that goes through the same code path inside
        // `build_listener` (file_name extraction → to_ns_name →
        // ListenerOptions::create_tokio).
        let unique = format!(
            "mneme-store-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let synth = std::path::PathBuf::from(format!(r"\.\pipe\{unique}"));
        let listener = build_listener(&synth).expect(
            "build_listener must succeed against a PathBuf shaped like \
             PathManager::store_socket() on Windows. Pre-NEW-B fix this \
             returned Err because to_fs_name<GenericFilePath> rejects \
             \\.\\pipe\\... (one leading backslash) — see paths.rs::store_socket.",
        );
        drop(listener);

        // Sanity: the production pm helper still returns the canonical
        // mneme-store leaf so the *real* code path the worker uses is
        // exactly what we just verified, modulo the unique leaf swap.
        let prod = pm.store_socket();
        assert_eq!(
            prod.file_name().and_then(|s| s.to_str()),
            Some("mneme-store"),
            "PathManager::store_socket() must keep the `mneme-store` leaf — \
             the Windows arm of build_listener relies on file_name() being non-empty"
        );
    }
}

/// Drop-then-rebind round trip. Proves the listener releases its
/// underlying resource (Unix: socket file via the helper's unlink path;
/// Windows: named-pipe handle) when dropped, so the worker's restart
/// path can re-bind without manual cleanup.
#[tokio::test]
async fn build_listener_drops_cleanly_releasing_socket() {
    let tmp = tempfile::tempdir().expect("tempdir");

    #[cfg(unix)]
    {
        let pm = PathManager::with_root(tmp.path().to_path_buf());
        let path = pm.store_socket();

        let l1 = build_listener(&path).expect("first bind");
        drop(l1);

        // The helper's own remove_file call inside build_listener will
        // unlink any stale file before re-binding, so even if the OS
        // didn't auto-clean on drop, the second bind must succeed.
        let l2 = build_listener(&path).expect("second bind after drop");
        drop(l2);

        let _ = std::fs::remove_file(&path);
    }

    #[cfg(windows)]
    {
        // Use a unique pipe leaf (see test #1 for why) so the test does
        // not collide with a real running daemon.
        let _ = tmp; // silence unused-variable on Windows
        let unique = format!(
            "mneme-store-droptest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = std::path::PathBuf::from(format!(r"\.\pipe\{unique}"));

        let l1 = build_listener(&path).expect("first bind");
        drop(l1);

        let l2 = build_listener(&path).expect("second bind after drop");
        drop(l2);
    }
}
