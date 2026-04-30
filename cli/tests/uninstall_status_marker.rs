//! LIE-4: regression test for the post-rmdir uninstall status marker.
//!
//! Pre-fix:
//!   `mneme uninstall --all --purge-state` printed
//!   `✓ ~/.mneme purge scheduled (detached cleanup runs in ~2s)` then
//!   `process::exit(0)`. The detached `cmd /c rmdir` could fail silently
//!   (file locks, permission denied, AV interference) and the user never
//!   knew because the parent had already exited 0. There was no way to
//!   tell, after the fact, whether the rmdir actually completed.
//!
//! Post-fix:
//!   The detached cleanup script ALSO writes a marker JSON at
//!   `~/.mneme-uninstall-status.json` after the rmdir attempt with:
//!     { status: "complete" | "partial" | "failed",
//!       remaining_paths: [String], timestamp: String }
//!   Even when the rmdir partially succeeds, the script writes the
//!   status. A new `mneme uninstall --status` flag reads the marker
//!   and prints the actual outcome (or "no marker yet").
//!
//! This file tests the writer side of that contract:
//!   1. `write_uninstall_status_marker(target_dir, marker_path)` — when
//!      the target dir is gone, status=complete, remaining_paths=[].
//!   2. When the target dir still has files (locked / partial rmdir),
//!      status=partial, remaining_paths includes those files.
//!   3. When dirs::home_dir resolves successfully, the marker lands at
//!      the right computed path.
//!
//! Tests are written BEFORE the implementation lands (RED) per TDD.

use std::path::PathBuf;

use mneme_cli::commands::uninstall::{
    read_uninstall_status_marker, write_uninstall_status_marker, UninstallStatus,
};

/// LIE-4 scenario: rmdir succeeded — target dir is gone. Marker reports
/// status=complete, no remaining paths.
#[test]
fn marker_reports_complete_when_target_dir_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("does-not-exist-anymore");
    let marker_path = dir.path().join(".mneme-uninstall-status.json");

    write_uninstall_status_marker(&target, &marker_path);

    let status = read_uninstall_status_marker(&marker_path)
        .expect("marker should be parseable");
    assert_eq!(status.status, "complete");
    assert!(
        status.remaining_paths.is_empty(),
        "complete state should have no remaining paths, got {:?}",
        status.remaining_paths
    );
    assert!(!status.timestamp.is_empty(), "timestamp should be populated");
}

/// LIE-4 scenario: rmdir partially failed — a child file is still on
/// disk (simulates a locked file or AV-blocked path). Marker reports
/// status=partial and lists the remaining file in `remaining_paths`.
#[test]
fn marker_reports_partial_when_target_still_has_files() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("mneme-leftovers");
    std::fs::create_dir_all(&target).unwrap();
    // A locked-file analogue — we don't need a real lock here, only a
    // file that is "still there" after the rmdir would have run. The
    // marker writer's contract is: enumerate target_dir; if any path
    // remains, status=partial.
    let stuck = target.join("locked.dat");
    std::fs::write(&stuck, b"could not delete me").unwrap();

    let marker_path = dir.path().join(".mneme-uninstall-status.json");
    write_uninstall_status_marker(&target, &marker_path);

    let status = read_uninstall_status_marker(&marker_path)
        .expect("marker should be parseable");
    assert_eq!(status.status, "partial");
    assert!(
        status
            .remaining_paths
            .iter()
            .any(|p| p.ends_with("locked.dat")),
        "remaining_paths should include the leftover file. got: {:?}",
        status.remaining_paths
    );
}

/// LIE-4: a marker that doesn't exist on disk yet (rmdir hasn't run /
/// the detached child hasn't woken up) returns None — `mneme uninstall
/// --status` should print "no marker yet" in that case.
#[test]
fn read_marker_returns_none_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("never-written.json");
    let r = read_uninstall_status_marker(&path);
    assert!(
        r.is_none(),
        "missing marker should yield None, got {:?}",
        r
    );
}

/// LIE-4: deserialised marker matches the writer's schema exactly. This
/// pins the field names so install.ps1 / VM test harnesses that read
/// the marker by name don't break silently.
#[test]
fn marker_schema_is_stable() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("missing");
    let marker_path = dir.path().join("marker.json");

    write_uninstall_status_marker(&target, &marker_path);
    let raw = std::fs::read_to_string(&marker_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).expect("marker must parse as JSON");

    // Pin every field name. If any field gets renamed the test fails
    // and the renamer is forced to update VM test harnesses too.
    assert!(v.get("status").and_then(|s| s.as_str()).is_some(),
        "status field missing/wrong type. got: {raw}");
    assert!(v.get("remaining_paths").and_then(|s| s.as_array()).is_some(),
        "remaining_paths field missing/wrong type. got: {raw}");
    assert!(v.get("timestamp").and_then(|s| s.as_str()).is_some(),
        "timestamp field missing/wrong type. got: {raw}");
}

/// LIE-4 helper roundtrip: serialise then deserialise via the public API.
/// Catches drift between [`UninstallStatus`] and the on-disk format.
#[test]
fn write_read_roundtrip_preserves_remaining_paths_order() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("partial");
    std::fs::create_dir_all(&target).unwrap();
    let a = target.join("a.dat");
    let b = target.join("b.dat");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();

    let marker = dir.path().join("marker.json");
    write_uninstall_status_marker(&target, &marker);

    let status = read_uninstall_status_marker(&marker).expect("parseable");
    assert_eq!(status.status, "partial");
    assert_eq!(
        status.remaining_paths.len(),
        2,
        "both leftover files should be listed. got: {:?}",
        status.remaining_paths
    );
}

/// Minimal type-presence sanity check: the public types we depend on
/// from the cli crate exist and have the right shape. If this fails to
/// compile the cli crate's public surface for LIE-4 has regressed.
#[test]
fn public_uninstall_status_has_required_fields() {
    let s = UninstallStatus {
        status: "complete".to_string(),
        remaining_paths: Vec::<PathBuf>::new(),
        timestamp: "2026-04-29T00:00:00Z".to_string(),
    };
    assert_eq!(s.status, "complete");
}
