//! Subcommand handlers.
//!
//! Each module exposes a `run(args) -> CliResult<()>` (or async equivalent
//! for IPC-bound commands). `main.rs` picks one based on the parsed
//! [`clap`] subcommand and bubbles the result.

pub mod abort;
pub mod audit;
pub mod blast;
pub mod build;
pub mod build_state;
pub mod cache;
pub mod call_graph;
pub mod daemon;
pub mod doctor;
pub mod drift;
pub mod export;
pub mod federated;
pub mod find_references;
pub mod godnodes;
pub mod graph_diff;
pub mod graphify;
pub mod history;
pub mod inject;
pub mod install;
pub mod log;
// CRIT-15 (2026-05-05 audit): cross-cutting IPC helpers extracted out
// of build.rs (8,368 lines) so they no longer make build.rs a
// de-facto utility crate. See ipc_helpers.rs header for migration.
pub mod ipc_helpers;
pub mod models;
pub mod post_tool;
pub mod pre_tool;
pub mod pretool_edit_write;
pub mod pretool_grep_read;
pub mod rebuild;
pub mod recall;
pub mod register_mcp;
pub mod rollback;
pub mod self_update;
pub mod session_end;
pub mod session_prime;
pub mod shard_summary;
pub mod snap;
pub mod status;
pub mod step;
pub mod turn_end;
pub mod uninstall;
pub mod update;
pub mod userprompt_submit;
pub mod view;
pub mod why;

/// Bug #38 (2026-05-07): strip the Windows `\\?\` long-path prefix
/// from a path string before display. The prefix is added by
/// `std::fs::canonicalize` on Windows and stored verbatim in the
/// graph, but it's pure visual cruft for the user — every recall /
/// blast / find-references / call-graph result carried it. No-op on
/// POSIX. Idempotent (safe to call on already-stripped strings).
///
/// Use this at the *display boundary* only. SQL lookups still match
/// the stored canonical form (some queries explicitly accept both
/// `path` and `\\?\path` shapes — see blast.rs:225).
#[inline]
pub fn display_path(p: &str) -> &str {
    p.strip_prefix(r"\\?\").unwrap_or(p)
}

#[cfg(test)]
mod display_path_tests {
    use super::display_path;

    #[test]
    fn strips_windows_long_path_prefix() {
        assert_eq!(
            display_path(r"\\?\C:\Users\User\bench-test\auth.py"),
            r"C:\Users\User\bench-test\auth.py"
        );
    }

    #[test]
    fn passes_through_posix_path() {
        assert_eq!(
            display_path("/home/anish/proj/auth.py"),
            "/home/anish/proj/auth.py"
        );
    }

    #[test]
    fn passes_through_already_stripped() {
        assert_eq!(display_path(r"C:\foo\bar.rs"), r"C:\foo\bar.rs");
    }

    #[test]
    fn passes_through_empty() {
        assert_eq!(display_path(""), "");
    }

    #[test]
    fn passes_through_unc() {
        // \\server\share\... is a real UNC path, NOT the long-path prefix.
        // Only \\?\ should be stripped.
        assert_eq!(
            display_path(r"\\server\share\file.txt"),
            r"\\server\share\file.txt"
        );
    }
}
