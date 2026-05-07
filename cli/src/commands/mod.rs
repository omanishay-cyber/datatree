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
