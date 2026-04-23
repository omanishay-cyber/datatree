//! Single error type returned by every CLI handler.
//!
//! Handlers should bubble these via `?` and let `main.rs` map them to a
//! process exit code. We deliberately use `thiserror` rather than `anyhow`
//! at the boundary so callers (including the supervisor when it shells out
//! to `datatree`) can pattern-match on a stable variant set.

use std::path::PathBuf;

/// Result alias used throughout the crate.
pub type CliResult<T> = std::result::Result<T, CliError>;

/// Every failure mode the `datatree` CLI can hit.
///
/// The variants are intentionally coarse — granular structured info lives in
/// the `source` chain, surfaced via `Display` when `main.rs` prints the error
/// at exit. The exit codes are stable: hooks rely on them.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// `--help` / `--version` short-circuit. Not really an error; main.rs
    /// returns 0 when it sees this.
    #[error("clap exit: {0}")]
    Clap(#[from] clap::Error),

    /// IO at the OS level (file read/write, fs metadata, …).
    #[error("io error at {path:?}: {source}")]
    Io {
        /// Optional path that triggered the error.
        path: Option<PathBuf>,
        /// Underlying cause.
        #[source]
        source: std::io::Error,
    },

    /// JSON encode / decode failure (hook payloads, MCP config blobs).
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML encode / decode failure (Codex `~/.codex/config.toml`, etc.).
    #[error("toml decode error: {0}")]
    TomlDe(#[from] toml::de::Error),

    /// TOML serialize failure.
    #[error("toml encode error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    /// Couldn't reach the supervisor over the control socket.
    #[error("ipc error: {0}")]
    Ipc(String),

    /// The supervisor returned an explicit error response.
    #[error("supervisor: {0}")]
    Supervisor(String),

    /// User asked for a platform we don't know how to write a manifest for.
    #[error("unknown platform: {0}")]
    UnknownPlatform(String),

    /// User passed an invalid `--scope=...` value.
    #[error("invalid scope: {0} (expected global|user|project)")]
    InvalidScope(String),

    /// We refused to perform a destructive write because backup failed.
    #[error("backup failed for {path}: {reason}")]
    BackupFailed {
        /// File we tried to back up.
        path: PathBuf,
        /// Why the backup write failed.
        reason: String,
    },

    /// User edited the contents of the marker block; refusing to clobber.
    /// Re-running with `--force` overrides this.
    #[error("marker block at {path} was edited by user (sha mismatch); pass --force to overwrite")]
    MarkerEdited {
        /// Manifest path.
        path: PathBuf,
    },

    /// Free-form error for handler-specific failures that don't deserve
    /// a dedicated variant. Use sparingly — prefer adding a real variant.
    #[error("{0}")]
    Other(String),
}

impl CliError {
    /// Build a [`CliError::Io`] without the path being known.
    pub fn io_pathless(source: std::io::Error) -> Self {
        CliError::Io {
            path: None,
            source,
        }
    }

    /// Build a [`CliError::Io`] tagged with a path for nicer messages.
    pub fn io<P: Into<PathBuf>>(path: P, source: std::io::Error) -> Self {
        CliError::Io {
            path: Some(path.into()),
            source,
        }
    }

    /// Stable exit code for this error. Hooks branch on these in shells.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Clap(_) => 0,
            CliError::Io { .. } => 2,
            CliError::Json(_) | CliError::TomlDe(_) | CliError::TomlSer(_) => 3,
            CliError::Ipc(_) => 4,
            CliError::Supervisor(_) => 5,
            CliError::UnknownPlatform(_) | CliError::InvalidScope(_) => 6,
            CliError::BackupFailed { .. } | CliError::MarkerEdited { .. } => 7,
            CliError::Other(_) => 1,
        }
    }
}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        CliError::Other(format!("{err:#}"))
    }
}

// Note: we intentionally do NOT impl From<std::io::Error> for CliError so
// callers are forced to attach a path via [`CliError::io`] / [`io_pathless`].
// This keeps error messages actionable.
