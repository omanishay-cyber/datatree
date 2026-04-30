//! Build-state checkpoint format — the resume-after-interrupt artifact
//! at `<project>/.mneme/build-state.json`.
//!
//! ## Why this exists
//!
//! `mneme build` is a long-running pipeline (parse → multimodal →
//! resolve-imports → leiden → embed → audit → tests → git → deps →
//! betweenness → intent → …). On a large repo it can run for minutes.
//! If the user hits Ctrl-C halfway through, the previous behavior was
//! "the next `mneme build` re-does the whole thing from scratch" —
//! the implicit per-file hash skip in `graph.db::files` worked, but
//! there was no explicit signal to the next build that "we got 7800/
//! 8000 files into the parse pass before being interrupted; resume
//! from there".
//!
//! ## Format
//!
//! Versioned JSON. Always read with `serde_json::from_slice` and
//! always written via [`save`] (which uses `tempfile + atomic rename`
//! to avoid producing a half-written state file if the writer is
//! itself interrupted).
//!
//! ```json
//! {
//!   "version": 1,
//!   "project_root": "/path/to/project",
//!   "phase": "parse",
//!   "files_done": 7800,
//!   "files_total": 8000,
//!   "last_completed_file": "/path/to/project/src/big_dir/foo.rs",
//!   "started_at": "2026-04-27T15:30:00Z",
//!   "updated_at": "2026-04-27T15:34:12Z"
//! }
//! ```
//!
//! ## Resume contract
//!
//! On the next `mneme build`, the code reads the state file. When
//! present + version matches + project_root matches:
//!   * The walker runs as normal but emits a "resuming from <file>"
//!     log line at the top.
//!   * Files lexically `<= last_completed_file` are skipped (the
//!     previous build already wrote them to `graph.db`; the
//!     per-file-hash skip in the existing path catches them as
//!     up-to-date too, but the explicit skip is cheaper).
//!
//! The state file is **deleted on a fully successful build** so a
//! clean rebuild leaves no stale artifact.
//!
//! ## Safety
//!
//! * The state file is **advisory only**. A corrupted, mismatched, or
//!   newer-version file is silently ignored — the build continues
//!   from scratch. Better to over-do work than to skip files because
//!   of a bad checkpoint.
//! * The schema is forward-compatible: future versions can add fields
//!   without breaking older builds (older builds simply ignore
//!   unknown fields via `serde(default)` / non-strict deserialize).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Bumped whenever the schema gains a required field. Older builds see
/// a higher version and silently ignore the file (treated as "no
/// resume info available").
pub const BUILD_STATE_VERSION: u32 = 1;

/// Coarse phase the build was in when the state was last persisted.
/// New phases can be added freely; consumers `match` on `&str`-shaped
/// names so adding a variant is forward-compatible.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildPhase {
    /// Per-file walk + parse + extract + persist nodes/edges.
    Parse,
    /// `run_multimodal_pass` — PDF / Markdown / image extraction.
    Multimodal,
    /// `run_resolve_imports_pass` — fixup of `import::*` pseudo-IDs.
    ResolveImports,
    /// `run_leiden_pass` — community detection.
    Leiden,
    /// `run_embedding_pass` — semantic vector population.
    Embedding,
    /// `run_audit_pass` — drift / theme / security scanners.
    Audit,
    /// `run_tests_pass` / `run_git_pass` / `run_deps_pass` etc.
    Auxiliary,
    /// All passes complete; ready to be deleted.
    Done,
}

/// On-disk shape of `<project>/.mneme/build-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildState {
    pub version: u32,
    /// The project root the state belongs to. If the user moved the
    /// project directory between runs this won't match and the file
    /// is ignored. Stored as a String because PathBuf serializes
    /// platform-specific path separators which we want to preserve.
    pub project_root: String,
    /// Coarse phase the previous run was in when interrupted (or
    /// when the periodic save fired — saves are not always at
    /// phase boundaries).
    pub phase: BuildPhase,
    /// How many files the parse pass had completed at save time.
    /// `0` for non-parse phases (the parse pass is the only one that
    /// has per-file granularity).
    #[serde(default)]
    pub files_done: u64,
    /// Total candidate files, when known. `0` until the walker has
    /// counted the project (which we deliberately don't pre-count
    /// for cost reasons, so this is usually 0 except on the final
    /// resume-from-state save).
    #[serde(default)]
    pub files_total: u64,
    /// The last file the parse pass successfully persisted to
    /// `graph.db`. Lexically-greater files have NOT been written.
    /// Empty string when the parse pass hasn't yet processed any file
    /// (so `<= last_completed_file` matches nothing — the walker
    /// proceeds from the start).
    #[serde(default)]
    pub last_completed_file: String,
    /// RFC 3339 timestamp of the build start.
    pub started_at: String,
    /// RFC 3339 timestamp of the most recent save.
    pub updated_at: String,
}

impl BuildState {
    /// Construct a fresh state for the given project. Caller should
    /// `save` after each pass / file batch.
    pub fn new(project_root: &Path) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            version: BUILD_STATE_VERSION,
            project_root: project_root.display().to_string(),
            phase: BuildPhase::Parse,
            files_done: 0,
            files_total: 0,
            last_completed_file: String::new(),
            started_at: now.clone(),
            updated_at: now,
        }
    }

    /// Mark progress on the parse pass. Called from the per-file loop
    /// — typically every 25 files (the same cadence as the existing
    /// `indexed % 25 == 0` log line) to avoid hammering the disk.
    pub fn mark_parse_progress(&mut self, files_done: u64, last_file: &Path) {
        self.phase = BuildPhase::Parse;
        self.files_done = files_done;
        self.last_completed_file = last_file.display().to_string();
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Move the state into a non-parse phase. Resets the per-file
    /// counters because non-parse phases don't have file-level
    /// granularity (the resume on the next run still works because
    /// graph.db's file-hash skip covers re-parses).
    pub fn enter_phase(&mut self, phase: BuildPhase) {
        self.phase = phase;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

/// Resolve the on-disk state file path for `project_root`. The file
/// lives at `<project>/.mneme/build-state.json`. We deliberately
/// place it inside the project (NOT under `~/.mneme/projects/<id>/`)
/// so a user nuking `~/.mneme` for any reason doesn't lose their
/// in-progress checkpoint.
pub fn state_path(project_root: &Path) -> PathBuf {
    project_root.join(".mneme").join("build-state.json")
}

/// Read the state from disk if present + parseable + version-compatible.
/// Returns `None` for any error (missing, corrupt, version-mismatch,
/// project-root-mismatch). The caller treats `None` as "no resume
/// info; build from scratch".
///
/// ## Path comparison
///
/// The CLI passes `project_root` post-`std::fs::canonicalize`, which
/// on Windows expands to a `\\?\C:\…` UNC-style absolute path. The
/// state file may have been written with a different prefix (a fresh
/// run from a different shell, a test fixture writing the raw path,
/// etc.). We canonicalize BOTH sides before comparing — when the
/// underlying directory is the same, the comparison succeeds even if
/// one side has the verbatim prefix and the other doesn't.
pub fn load(project_root: &Path) -> Option<BuildState> {
    let path = state_path(project_root);
    let bytes = std::fs::read(&path).ok()?;
    let state: BuildState = serde_json::from_slice(&bytes).ok()?;
    if state.version != BUILD_STATE_VERSION {
        return None;
    }
    if !same_path(&state.project_root, project_root) {
        return None;
    }
    Some(state)
}

/// Compare a stored path string with a runtime `Path`, tolerant of
/// Windows verbatim-prefix differences and trailing-separator quirks.
/// Both sides are run through `dunce::canonicalize` (or the std lib
/// fallback on non-Windows) so `\\?\C:\foo` and `C:\foo` compare equal
/// when they refer to the same real directory.
fn same_path(stored: &str, actual: &Path) -> bool {
    let stored_p = Path::new(stored);
    // Try canonical form on both sides; fall back to display-string
    // equality when canonicalize fails (e.g. the dir was deleted).
    match (
        std::fs::canonicalize(stored_p),
        std::fs::canonicalize(actual),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => stored_p == actual || stored == actual.display().to_string(),
    }
}

/// Atomically persist the state to disk. Uses tempfile + rename so a
/// crash during the write never leaves a half-written state file.
///
/// On Windows, `fs::rename` over an existing file fails with
/// `ERROR_ALREADY_EXISTS`. We fall back to remove-then-rename, which
/// is non-atomic but acceptable for an advisory checkpoint (the
/// caller will save again at the next phase / file batch).
pub fn save(project_root: &Path, state: &BuildState) -> std::io::Result<()> {
    let final_path = state_path(project_root);
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = final_path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("serialize build state: {e}"),
        )
    })?;
    std::fs::write(&tmp_path, &bytes)?;
    // Best-effort atomic replace.
    match std::fs::rename(&tmp_path, &final_path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows fallback: remove then rename. Not atomic; an
            // interrupt between the two leaves no state file, which
            // is the same as never having saved — caller continues
            // safely on the next iteration.
            let _ = std::fs::remove_file(&final_path);
            std::fs::rename(&tmp_path, &final_path)
        }
    }
}

/// Delete the state file. Called on a fully successful build so a
/// re-build doesn't pick up stale resume info.
pub fn clear(project_root: &Path) {
    let path = state_path(project_root);
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn roundtrip_save_then_load() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        let mut state = BuildState::new(project);
        state.mark_parse_progress(42, &project.join("src/foo.rs"));
        save(project, &state).expect("save");
        let loaded = load(project).expect("load");
        assert_eq!(loaded.files_done, 42);
        assert_eq!(loaded.phase, BuildPhase::Parse);
        assert!(loaded.last_completed_file.contains("foo.rs"));
    }

    #[test]
    fn load_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert!(load(tmp.path()).is_none());
    }

    #[test]
    fn load_corrupt_returns_none() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        std::fs::create_dir_all(state_path(project).parent().unwrap()).unwrap();
        std::fs::write(state_path(project), b"NOT-JSON").unwrap();
        assert!(load(project).is_none());
    }

    #[test]
    fn load_wrong_project_root_returns_none() {
        let tmp = TempDir::new().unwrap();
        let mut state = BuildState::new(Path::new("/some/other/project"));
        state.mark_parse_progress(1, Path::new("/some/other/project/foo.rs"));
        save(tmp.path(), &state).expect("save");
        // Loading from a different project_root than what was saved
        // returns None — guards against cross-project state leakage.
        assert!(load(tmp.path()).is_none());
    }

    #[test]
    fn clear_removes_state() {
        let tmp = TempDir::new().unwrap();
        let state = BuildState::new(tmp.path());
        save(tmp.path(), &state).unwrap();
        assert!(state_path(tmp.path()).exists());
        clear(tmp.path());
        assert!(!state_path(tmp.path()).exists());
    }

    #[test]
    fn version_mismatch_returns_none() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        std::fs::create_dir_all(state_path(project).parent().unwrap()).unwrap();
        // Write a state file with a future version.
        let payload = serde_json::json!({
            "version": 999,
            "project_root": project.display().to_string(),
            "phase": "parse",
            "files_done": 0,
            "files_total": 0,
            "last_completed_file": "",
            "started_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(state_path(project), payload.to_string()).unwrap();
        assert!(load(project).is_none());
    }
}
