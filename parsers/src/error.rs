//! Error type for the parsers crate.
//!
//! All public APIs return `Result<_, ParserError>`; downstream workers
//! convert these into `livebus` events when they bubble out of a worker.

use std::path::PathBuf;
use thiserror::Error;

/// Top-level error for every operation the `parsers` crate exposes.
#[derive(Debug, Error)]
pub enum ParserError {
    /// The requested language is not built into this binary.
    ///
    /// Most likely cause: the relevant cargo feature (e.g. `swift`, `vue`) is
    /// disabled. The string is the language identifier the caller supplied.
    #[error("language `{0}` is not enabled in this build (check cargo features)")]
    LanguageNotEnabled(String),

    /// File extension or filename did not map to any known [`Language`].
    ///
    /// [`Language`]: crate::language::Language
    #[error("no language found for path `{0}`")]
    UnknownLanguage(PathBuf),

    /// `tree_sitter::Parser::set_language` failed — almost always an ABI
    /// mismatch between the grammar crate and the `tree-sitter` runtime.
    #[error("tree-sitter rejected the grammar for {language:?}: {source}")]
    SetLanguage {
        language: String,
        #[source]
        source: tree_sitter::LanguageError,
    },

    /// `parse()` returned `None` — usually a hard timeout/cancellation.
    #[error("parser returned no tree (cancelled or timed out) for `{0}`")]
    ParseFailed(PathBuf),

    /// A pre-compiled query (in [`crate::query_cache`]) failed to compile.
    /// This is a programmer bug, not a runtime input bug.
    #[error("query `{kind}` for {language:?} failed to compile: {source}")]
    QueryCompile {
        language: String,
        kind: &'static str,
        #[source]
        source: tree_sitter::QueryError,
    },

    /// Conversion of bytes to UTF-8 failed when slicing source text.
    #[error("invalid UTF-8 in source range {start}..{end}: {source}")]
    Utf8 {
        start: usize,
        end: usize,
        #[source]
        source: std::str::Utf8Error,
    },

    /// I/O error reading source from disk (only used by the binary entry).
    #[error("io error for `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// MPSC send failed — receiver was dropped (worker shutting down).
    #[error("worker channel closed; result discarded")]
    ChannelClosed,

    /// Caller attempted to acquire a parser slot that does not exist.
    #[error("no parser slot for language {0:?}")]
    NoParserForLanguage(String),

    /// Catch-all for unexpected internal invariants.
    #[error("internal parser error: {0}")]
    Internal(String),
}

impl ParserError {
    /// True when retrying the same job is worth the cost.
    ///
    /// Used by the supervisor's restart logic — see §3.3.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ParseFailed(_) | Self::ChannelClosed | Self::Io { .. }
        )
    }
}
