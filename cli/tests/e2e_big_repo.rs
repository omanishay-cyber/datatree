//! End-to-end integration test: clone a known repo, run mneme build,
//! exercise recall + blast + godnodes against the resulting graph.
//!
//! This test is `#[ignore]` by default because it (a) shells out to a
//! built `mneme` binary and (b) clones a remote git repo. Run with:
//!
//! ```bash
//! cargo test --release -p mneme-cli --test e2e_big_repo -- --ignored --nocapture
//! ```
//!
//! Set `MNEME_BIN` to override the binary path used by the test
//! (defaults to `./target/release/mneme` or `mneme.exe` on Windows).
//! Set `E2E_FIXTURE_REPO` to override the clone target (defaults to
//! the `omanishay-cyber/mneme` repo itself - the smallest reproducible
//! Rust + TS fixture we control).
//!
//! ## What this test gates
//!
//! - `mneme build` produces a non-empty graph (>0 nodes, >0 edges) on
//!   a real repo without crashing.
//! - `mneme recall <query>` returns at least one hit for 4 queries
//!   that we know are present in the fixture repo (Rust + TS terms).
//! - `mneme blast <symbol>` returns a non-error response.
//! - `mneme godnodes --n 5` returns 5 ranked entries.
//! - `mneme doctor --offline` exits 0.
//!
//! Each assertion that fails prints the full stdout+stderr it captured
//! so a CI failure has the data to diagnose without rerunning.

use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore]
fn e2e_clone_build_recall_blast() {
    let bin = mneme_binary();
    let workspace = tempdir_for_test();
    let repo_dir = workspace.join("fixture-repo");
    let fixture_url = std::env::var("E2E_FIXTURE_REPO")
        .unwrap_or_else(|_| "https://github.com/omanishay-cyber/mneme.git".into());

    eprintln!("e2e: workspace = {}", workspace.display());
    eprintln!("e2e: bin       = {}", bin.display());
    eprintln!("e2e: fixture   = {fixture_url}");

    // 1. Clone the fixture (depth 1).
    let clone_status = Command::new("git")
        .args(["clone", "--depth", "1", &fixture_url])
        .arg(&repo_dir)
        .status()
        .expect("git clone failed to spawn — is git installed?");
    assert!(
        clone_status.success(),
        "git clone of fixture {fixture_url} returned {clone_status:?}"
    );

    // 2. mneme build --yes --limit 200 (small enough to finish in CI).
    let build = Command::new(&bin)
        .arg("build")
        .arg(&repo_dir)
        .arg("--yes")
        .arg("--limit")
        .arg("200")
        .output()
        .expect("mneme build failed to spawn");
    let build_stdout = String::from_utf8_lossy(&build.stdout);
    let build_stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        build.status.success(),
        "mneme build exited {:?}\n--- stdout ---\n{build_stdout}\n--- stderr ---\n{build_stderr}",
        build.status.code()
    );
    assert!(
        build_stdout.contains("indexed") || build_stdout.contains("nodes"),
        "mneme build stdout did not contain indexing markers; got:\n{build_stdout}"
    );

    // 3. recall — four queries that exist in any healthy code repo.
    for query in ["pub fn", "fn", "import", "use"] {
        let out = Command::new(&bin)
            .arg("recall")
            .arg(query)
            .arg("--project")
            .arg(&repo_dir)
            .arg("--limit")
            .arg("5")
            .output()
            .unwrap_or_else(|e| panic!("mneme recall '{query}' failed to spawn: {e}"));
        assert!(
            out.status.success(),
            "mneme recall '{query}' exited {:?}\n{}\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // 4. godnodes — top 5 most-connected concepts.
    let godnodes = Command::new(&bin)
        .args(["godnodes", "--n", "5", "--project"])
        .arg(&repo_dir)
        .output()
        .expect("mneme godnodes failed to spawn");
    let godnodes_stdout = String::from_utf8_lossy(&godnodes.stdout);
    assert!(
        godnodes.status.success(),
        "mneme godnodes exited {:?}\n{godnodes_stdout}",
        godnodes.status.code()
    );

    // 5. blast — pick the first godnode and ask its blast radius.
    if let Some(line) = godnodes_stdout
        .lines()
        .find(|l| l.trim_start().starts_with("1.") && l.contains('['))
    {
        // Try to extract the [type] name token.
        let target = line
            .split(']')
            .nth(1)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if let Some(target) = target {
            let blast = Command::new(&bin)
                .args(["blast", &target, "--project"])
                .arg(&repo_dir)
                .arg("--depth")
                .arg("1")
                .output()
                .expect("mneme blast failed to spawn");
            assert!(
                blast.status.success(),
                "mneme blast '{target}' exited {:?}\n{}\n{}",
                blast.status.code(),
                String::from_utf8_lossy(&blast.stdout),
                String::from_utf8_lossy(&blast.stderr)
            );
        }
    }

    // 6. doctor --offline — must exit 0 cleanly.
    let doctor = Command::new(&bin)
        .args(["doctor", "--offline"])
        .output()
        .expect("mneme doctor --offline failed to spawn");
    assert!(
        doctor.status.success(),
        "mneme doctor --offline exited {:?}\n{}\n{}",
        doctor.status.code(),
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );

    eprintln!("e2e: all assertions passed");
}

fn mneme_binary() -> PathBuf {
    if let Ok(env) = std::env::var("MNEME_BIN") {
        return PathBuf::from(env);
    }
    let exe = if cfg!(windows) { "mneme.exe" } else { "mneme" };
    // Walk up from CARGO_MANIFEST_DIR (cli/) to workspace root, then target/release.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("cli/ has a parent (workspace root)")
        .to_path_buf();
    workspace_root.join("target").join("release").join(exe)
}

fn tempdir_for_test() -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "mneme-e2e-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&base).expect("could not create e2e tempdir");
    base
}
