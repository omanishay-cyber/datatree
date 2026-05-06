//! Cross-cutting IPC helpers used by every CLI command that talks to
//! the supervisor.
//!
//! ## CRIT-15 fix (2026-05-05 audit)
//!
//! These helpers used to live inside `cli/src/commands/build.rs`, the
//! 8,368-line god file that owns the build pipeline. Nine unrelated
//! commands (audit, blast, daemon, doctor, drift, godnodes, recall,
//! self_update, status, step) imported them via
//! `crate::commands::build::{make_client, handle_response, BUILD_IPC_TIMEOUT}`,
//! which made `build.rs` a de-facto utility crate AND meant any change
//! to the build pipeline risked breaking unrelated commands.
//!
//! Extracting them here gives each cross-cutting helper its own home
//! without introducing a new top-level crate. Future PRs should import
//! from `crate::commands::ipc_helpers::*` instead of from `build`.
//!
//! `build.rs` re-exports these via `pub use` so the existing nine
//! callers continue to compile until they migrate.
//!
//! ## HIGH-47 fix (2026-05-06, 2026-05-05 audit): path resolution helpers
//!
//! `resolve_project_root` and `graph_db_path` were duplicated VERBATIM
//! across five commands (blast, export, godnodes, graph_diff, recall) under
//! three names (`resolve_project_root`, `paths_graph_db`, `live_graph_db`).
//! They now live here as the single canonical implementation. All five
//! commands import from this module and their local copies are deleted.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{CliError, CliResult};
use crate::ipc::IpcClient;
use common::{ids::ProjectId, paths::PathManager};

/// B-001: per-round-trip timeout for build-pipeline IPC. The default
/// `IpcClient` budget is 120s; that's appropriate for hooks but lets a
/// wedged supervisor turn `mneme build` into a 74-minute hang (as
/// observed on EC2 2026-04-27). 5s is generous for a JSON round-trip
/// against a healthy supervisor and forces a fast fallback when one
/// isn't.
pub(crate) const BUILD_IPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Resolve `project` to an absolute, canonicalised path. Falls back to
/// CWD if the user passed nothing.
pub(crate) fn resolve_project(arg: Option<PathBuf>) -> CliResult<PathBuf> {
    let raw = arg.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let canonical = std::fs::canonicalize(&raw).unwrap_or(raw);
    Ok(canonical)
}

/// Build an IPC client honoring `--socket` overrides.
pub(crate) fn make_client(socket_override: Option<PathBuf>) -> IpcClient {
    match socket_override {
        Some(p) => IpcClient::new(p),
        None => IpcClient::default_path(),
    }
}

/// B-001/B-002: build pipeline variant of [`make_client`]. The build
/// pipeline must NEVER auto-spawn a second `mneme-daemon` on connect
/// failure, and every per-call round-trip must be tightly bounded so a
/// stuck supervisor surfaces as a fast error instead of a 74-minute
/// hang.
///
/// ## Hooks NEVER auto-spawn either (Bug E, 2026-04-29)
///
/// The hook commands (`mneme inject` / `pre_tool` / `post_tool` /
/// `session_*` / `turn_end`) use [`crate::hook_payload::make_hook_client`]
/// instead, which sets [`IpcClient::with_no_autospawn`]. The supervisor
/// not being up means mneme is intentionally inactive; the user runs
/// `mneme daemon start` to activate context capture.
///
/// Today the only auto-spawn caller is [`make_client`] itself, used
/// by commands the user explicitly types (`mneme recall`, `mneme
/// blast`, `mneme step`, `mneme audit`, etc.).
pub(crate) fn make_client_for_build(socket_override: Option<PathBuf>) -> IpcClient {
    make_client(socket_override)
        .with_no_autospawn()
        .with_timeout(BUILD_IPC_TIMEOUT)
}

// ---------------------------------------------------------------------------
// HIGH-47 (2026-05-06, 2026-05-05 audit): consolidated path helpers
// ---------------------------------------------------------------------------

/// Canonicalise the user's `--project` flag (or CWD) to an absolute path.
///
/// Previously duplicated verbatim as `resolve_project_root` in blast.rs,
/// export.rs, godnodes.rs, graph_diff.rs, and recall.rs. All five now
/// delegate here. See HIGH-47 fix note in this module's header.
///
/// Unlike [`resolve_project`] (which returns `CliResult<PathBuf>`), this
/// function never errors — canonicalization failure falls back to the
/// original path, and a missing `--project` flag falls back to CWD. Call
/// sites that need the `CliResult` wrapper should use [`resolve_project`]
/// instead.
pub(crate) fn resolve_project_root(project: Option<PathBuf>) -> PathBuf {
    project
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Map a resolved project root to its `graph.db` path.
///
/// Uses the same `PathManager`/`ProjectId` chain the CLI always has — the
/// supervisor derives the same location so shard selection cannot drift.
///
/// Previously duplicated as `paths_graph_db` (blast.rs, godnodes.rs,
/// recall.rs) and `live_graph_db` (export.rs, graph_diff.rs). All five
/// sites now call this single function. See HIGH-47 fix note in this
/// module's header.
pub(crate) fn graph_db_path(root: &Path) -> CliResult<PathBuf> {
    let id = ProjectId::from_path(root).map_err(|e| {
        CliError::Other(format!("cannot hash project path {}: {e}", root.display()))
    })?;
    let paths = PathManager::default_root();
    Ok(paths.project_root(&id).join("graph.db"))
}
