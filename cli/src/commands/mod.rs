//! Subcommand handlers.
//!
//! Each module exposes a `run(args) -> CliResult<()>` (or async equivalent
//! for IPC-bound commands). `main.rs` picks one based on the parsed
//! [`clap`] subcommand and bubbles the result.

pub mod audit;
pub mod blast;
pub mod build;
pub mod daemon;
pub mod doctor;
pub mod drift;
pub mod godnodes;
pub mod graphify;
pub mod history;
pub mod inject;
pub mod install;
pub mod models;
pub mod post_tool;
pub mod pre_tool;
pub mod rebuild;
pub mod recall;
pub mod session_end;
pub mod session_prime;
pub mod snap;
pub mod status;
pub mod step;
pub mod turn_end;
pub mod uninstall;
pub mod update;
pub mod view;
