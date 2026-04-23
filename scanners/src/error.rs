//! Scanner-wide error type.
//!
//! Every fallible function in this crate returns `Result<T, ScannerError>`.
//! Scanner failures are *isolated*: when one scanner produces an error we
//! record it on the [`crate::job::ScanResult`] and continue running the
//! remaining scanners — matching the design contract that "failure of one
//! scanner doesn't stop the others".

use std::io;
use thiserror::Error;

/// All errors produced by the scanners crate.
#[derive(Debug, Error)]
pub enum ScannerError {
    /// A scanner panicked or returned an internal error while processing a file.
    #[error("scanner '{scanner}' failed on '{file}': {message}")]
    ScannerFailed {
        /// Name of the offending scanner (e.g. "theme", "security").
        scanner: String,
        /// Path of the file being scanned.
        file: String,
        /// Free-form failure message.
        message: String,
    },

    /// IPC channel to the store-worker dropped, blocked, or refused our findings.
    #[error("store ipc error: {0}")]
    StoreIpc(String),

    /// MPSC scan-job channel closed unexpectedly.
    #[error("job channel closed")]
    JobChannelClosed,

    /// Configuration error (bad path, missing token file, etc.).
    #[error("invalid scanner configuration: {0}")]
    Config(String),

    /// A required regex failed to compile at startup. This is a programmer
    /// error and should never reach production.
    #[error("regex compile error: {0}")]
    Regex(#[from] regex::Error),

    /// Generic I/O error (filesystem, sockets, pipes).
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// JSON (de)serialization error from the IPC envelope.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for higher-level errors.
    #[error("internal error: {0}")]
    Other(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, ScannerError>;
