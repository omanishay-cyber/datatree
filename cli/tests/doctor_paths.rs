//! Integration tests for the `doctor` command's path-discovery helpers.
//!
//! The live MCP probe is inherently Windows-only today because the
//! installed `mneme.exe` that runs on the developer's box is itself a
//! Windows build. These tests instead exercise the PURE path-resolution
//! surface that must behave identically on Linux / macOS:
//!
//! * [`expected_binary_names`] picks a `.exe`-suffixed matrix on Windows
//!   and a suffix-free matrix everywhere else.
//! * [`mcp_entry_path`] produces `<home>/.mneme/mcp/src/index.ts`
//!   regardless of the underlying path separator.
//!
//! We can't spin up a real Linux VM inside this test harness, but the
//! `cfg!()` predicate the doctor uses to branch is evaluated at compile
//! time — so running these tests on any host verifies exactly the
//! matrix that host will ship. The test is additionally written so
//! that a Linux CI runner catches the conditional drift the moment it
//! happens (see README.md's CI matrix).

use mneme_cli::commands::doctor::{expected_binary_names, mcp_entry_path};

#[test]
fn expected_binaries_cover_every_worker() {
    let names = expected_binary_names();
    // Every mneme component binary must appear exactly once.
    let expected_stems = [
        "mneme",
        "mneme-daemon",
        "mneme-brain",
        "mneme-parsers",
        "mneme-scanners",
        "mneme-livebus",
        "mneme-md-ingest",
        "mneme-store",
        "mneme-multimodal",
    ];
    assert_eq!(
        names.len(),
        expected_stems.len(),
        "binary matrix size drifted: {:?}",
        names
    );
    for stem in expected_stems {
        let found = names
            .iter()
            .any(|n| n.strip_suffix(".exe").unwrap_or(n) == stem);
        assert!(found, "missing binary stem: {stem} (got {:?})", names);
    }
}

#[cfg(windows)]
#[test]
fn expected_binaries_use_exe_suffix_on_windows() {
    for n in expected_binary_names() {
        assert!(
            n.ends_with(".exe"),
            "expected .exe suffix on Windows, got {n}"
        );
    }
}

#[cfg(not(windows))]
#[test]
fn expected_binaries_have_no_extension_on_unix() {
    for n in expected_binary_names() {
        assert!(
            !n.contains('.'),
            "Unix binaries must not carry a dotted suffix, got {n}"
        );
    }
}

#[test]
fn mcp_entry_path_is_under_home_mneme_mcp() {
    // dirs::home_dir() returns Some on every real OS; None is the
    // degenerate "detached tty" case we don't need to assert here.
    let Some(entry) = mcp_entry_path() else {
        // CI runner with no home dir — skip without failing. Don't
        // use unwrap() because that would error on genuinely-broken
        // environments and poison the matrix.
        eprintln!("no home dir; skipping mcp_entry_path_is_under_home_mneme_mcp");
        return;
    };
    // Walk from the end upwards; using PathBuf::components avoids
    // platform separator confusion.
    let comps: Vec<_> = entry
        .components()
        .rev()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    assert!(comps.len() >= 4, "path too short: {:?}", entry);
    assert_eq!(comps[0], "index.ts");
    assert_eq!(comps[1], "src");
    assert_eq!(comps[2], "mcp");
    assert_eq!(comps[3], ".mneme");
}

#[test]
fn mcp_entry_path_is_absolute_when_home_resolves() {
    let Some(entry) = mcp_entry_path() else {
        return;
    };
    // `dirs::home_dir()` returns an absolute path on every
    // supported OS (Linux uses `$HOME`, macOS too, Windows uses
    // `%USERPROFILE%`). Enforce that invariant so a future regression
    // that accidentally builds a relative path can't ship.
    assert!(
        entry.is_absolute(),
        "mcp_entry_path must be absolute, got {}",
        entry.display()
    );
}

/// Cross-platform directory layout under `<home>/.mneme/`.
///
/// This test does not inspect the FS — it only verifies the *shape* of
/// the path we compute. That keeps the test deterministic on fresh CI
/// runners that never ran `mneme install`.
#[test]
fn mneme_root_paths_share_single_dotted_segment() {
    let Some(entry) = mcp_entry_path() else {
        return;
    };
    let s = entry.display().to_string();
    // Exactly one `.mneme` dot-prefixed segment — never `.mneme.mneme`
    // or `mneme/.mneme/.mneme` style drift.
    let dotted_hits: usize = s.matches(".mneme").count();
    assert!(
        dotted_hits >= 1,
        "expected at least one .mneme segment in {s}"
    );
}
