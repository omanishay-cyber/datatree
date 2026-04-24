//! Mneme CLI — library surface.
//!
//! The `mneme` binary in `main.rs` is intentionally thin: it parses
//! [`clap`] subcommands and dispatches to handlers exposed here. Putting the
//! handlers behind a library boundary lets us:
//!
//! 1. unit-test marker injection, platform detection, and IPC framing without
//!    spawning a subprocess;
//! 2. let the supervisor crate or integration tests reuse helpers (e.g.
//!    [`platforms::PlatformDetector`]) without duplicating logic;
//! 3. expose a stable surface for future plugins that want to embed mneme.
//!
//! ## Module map
//!
//! - [`commands`]   — one module per subcommand (`install`, `build`, `recall`, …)
//! - [`platforms`]  — the 18-platform integration matrix from design §21.4
//! - [`ipc`]        — async client to the supervisor's control socket
//! - [`markers`]    — idempotent injection: `<!-- mneme-start v1.0 --> … -->`
//! - [`error`]      — `CliError`, the single error type the binary returns
//!
//! Re-exports are kept narrow so adding new commands does not pollute the
//! public surface.

#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]
#![warn(missing_debug_implementations)]

pub mod commands;
pub mod error;
pub mod hook_payload;
pub mod ipc;
pub mod markers;
pub mod platforms;
pub mod receipts;
pub mod skill_matcher;

#[cfg(test)]
pub mod tests;

pub use error::{CliError, CliResult};
pub use ipc::{IpcClient, IpcRequest, IpcResponse};
pub use markers::{MarkerBlock, MarkerInjector, MARKER_END, MARKER_START_PREFIX};
pub use platforms::{InstallScope, Platform, PlatformDetector};

/// Marker version embedded inside `<!-- mneme-start v{VERSION} -->`.
/// Bumping this forces a re-write of every platform's manifest.
pub const MARKER_VERSION: &str = "1.0";

/// Default IPC socket / named-pipe filename. Resolved relative to the
/// runtime dir returned by [`runtime_dir`].
pub const DEFAULT_IPC_SOCKET_NAME: &str = "mneme-supervisor.sock";

/// Returns the platform-appropriate runtime directory used by the supervisor
/// for its IPC socket and pidfile. Mirrors the supervisor's resolution logic
/// so the CLI can connect without out-of-band config.
pub fn runtime_dir() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("MNEME_RUNTIME_DIR") {
        return std::path::PathBuf::from(custom);
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".mneme").join("run");
    }
    std::env::temp_dir().join("mneme-run")
}

/// Returns the platform-appropriate state directory (databases, snapshots,
/// crash dumps). Per design §13: every panic writes a minidump to
/// `~/.mneme/crashes/`.
pub fn state_dir() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("MNEME_STATE_DIR") {
        return std::path::PathBuf::from(custom);
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".mneme");
    }
    std::env::temp_dir().join("mneme-state")
}
