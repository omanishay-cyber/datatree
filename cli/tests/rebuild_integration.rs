//! Integration tests for `mneme rebuild` direct-DB fallback (audit L17).
//!
//! These tests exercise the rebuild path without spawning a real
//! supervisor or relying on `~/.mneme/`. The full happy path
//! (rebuild → re-parse → identical node count) is gated behind
//! `#[ignore]` because it shells out to the built binary and writes
//! to the user's actual `~/.mneme/projects/` shard. Run with:
//!
//! ```bash
//! cargo test --release -p mneme-cli --test rebuild_integration -- --ignored --nocapture
//! ```
//!
//! The default suite (no `--ignored`) verifies the lock-contention
//! branch, which is the failure mode L17 specifically guards against.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use mneme_cli::build_lock::BuildLock;

/// Resolve the path to the built `mneme` binary. Mirrors
/// `cli/tests/e2e_big_repo.rs::mneme_binary`.
fn mneme_binary() -> PathBuf {
    if let Ok(env) = std::env::var("MNEME_BIN") {
        return PathBuf::from(env);
    }
    let exe = if cfg!(windows) { "mneme.exe" } else { "mneme" };
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("cli/ has a parent (workspace root)")
        .to_path_buf();
    workspace_root.join("target").join("release").join(exe)
}

/// L17 acceptance: when another process holds the build lock for a
/// project, `mneme rebuild` exits cleanly with exit code 4 and the
/// "rebuild requires exclusive access" message — instead of hanging,
/// crashing, or corrupting the half-rebuilt shard.
#[test]
fn rebuild_lock_contention_exits_with_code_4() {
    // We need a project path to rebuild. Use the workspace root
    // itself — it's a real Rust project with a Cargo.toml so
    // `find_project_root_for_cwd` resolves cleanly. We do NOT
    // actually let rebuild run the inline build (we use --no-ipc +
    // pre-acquired lock so the second invocation immediately hits
    // the contention error and exits).
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root exists")
        .to_path_buf();

    // Acquire the lock for the workspace's ProjectId. The PathManager
    // path is `~/.mneme/projects/<sha256>/.lock` so we have to do
    // the same resolution the binary does.
    let project_id = common::ids::ProjectId::from_path(&workspace_root).expect("hash project");
    let paths = common::paths::PathManager::default_root();
    let project_root = paths.project_root(&project_id);
    // Acquire on this thread; hold for 5s.
    let _holder = BuildLock::acquire(project_id.as_str(), &project_root, Duration::ZERO)
        .expect("first acquire (test-side)");

    // Now run `mneme rebuild --yes --no-ipc` and expect exit 4.
    let bin = mneme_binary();
    if !bin.exists() {
        eprintln!(
            "skipping rebuild_lock_contention_exits_with_code_4 — \
             {} not built; run `cargo build --release` first",
            bin.display()
        );
        return;
    }

    let out = Command::new(&bin)
        .arg("rebuild")
        .arg(&workspace_root)
        .arg("--yes")
        .arg("--no-ipc")
        .arg("--lock-timeout-secs")
        .arg("0")
        .output()
        .expect("mneme rebuild failed to spawn");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "rebuild should have failed with contention error;\nstdout: {stdout}\nstderr: {stderr}"
    );
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code, 4,
        "expected exit code 4 (CliError::Ipc); got {code}\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("rebuild requires exclusive access")
            || stdout.contains("rebuild requires exclusive access"),
        "expected 'rebuild requires exclusive access' message;\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// L17 acceptance: full rebuild round-trip on a tiny fixture
/// project. Marked ignore because it writes real shard files to
/// `~/.mneme/projects/<id>/`. The `clean_up` env var (`MNEME_TEST_KEEP=1`)
/// keeps the shard so a developer can inspect.
#[test]
#[ignore]
fn rebuild_round_trip_preserves_node_count() {
    use std::fs;
    use tempfile::TempDir;

    let bin = mneme_binary();
    if !bin.exists() {
        panic!("mneme binary not built at {}", bin.display());
    }

    // Create a tiny fixture project: a few markdown + a Cargo.toml.
    let dir = TempDir::new().expect("tempdir");
    let project = dir.path().join("rebuild-fixture");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"rebuild-fixture\"\nversion = \"0.0.1\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src").join("lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\npub struct Gamma;\n",
    )
    .unwrap();

    // 1. Initial build.
    let build = Command::new(&bin)
        .arg("build")
        .arg(&project)
        .arg("--yes")
        .output()
        .expect("mneme build spawn");
    assert!(
        build.status.success(),
        "mneme build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    // 2. Read the post-build node count via doctor / status — or
    //    open graph.db directly. We use direct sqlite to avoid
    //    flaky CLI parsing.
    let project_id = common::ids::ProjectId::from_path(&project).unwrap();
    let paths = common::paths::PathManager::default_root();
    let graph_db = paths.project_root(&project_id).join("graph.db");
    let count_nodes = || -> i64 {
        let conn = rusqlite::Connection::open_with_flags(
            &graph_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        conn.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap()
    };
    let nodes_pre = count_nodes();
    assert!(nodes_pre > 0, "initial build produced 0 nodes");

    // 3. Rebuild (direct-DB path; no daemon).
    let rebuild = Command::new(&bin)
        .arg("rebuild")
        .arg(&project)
        .arg("--yes")
        .arg("--no-ipc")
        .output()
        .expect("mneme rebuild spawn");
    assert!(
        rebuild.status.success(),
        "mneme rebuild failed: {}\n{}",
        String::from_utf8_lossy(&rebuild.stdout),
        String::from_utf8_lossy(&rebuild.stderr)
    );
    let stdout = String::from_utf8_lossy(&rebuild.stdout);
    assert!(
        stdout.contains("rebuild complete"),
        "expected 'rebuild complete' in stdout: {stdout}"
    );

    // 4. Node count should be identical (same fixture, same parse).
    let nodes_post = count_nodes();
    assert_eq!(
        nodes_pre, nodes_post,
        "rebuild changed node count: {nodes_pre} → {nodes_post}"
    );

    // Cleanup unless told otherwise.
    if std::env::var("MNEME_TEST_KEEP").is_err() {
        let _ = fs::remove_dir_all(paths.project_root(&project_id));
    }
}
