//! The [`Scanner`] trait and the [`Finding`] / [`Severity`] data model.
//!
//! Every concrete scanner under `crate::scanners::*` implements [`Scanner`].
//! Scanners are pure: they take a borrowed file path, the already-loaded
//! file contents, and an optional borrowed AST handle. They MUST NOT do
//! file I/O on the hot path — the worker passes content as `&str`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Opaque AST handle. The parse-worker owns the real Tree-sitter trees;
/// scanners only ever borrow a reference. We keep it as a unit type for
/// now so that downstream consumers can replace it with a richer enum
/// without breaking the [`Scanner`] trait signature.
#[derive(Debug, Clone, Copy)]
pub struct Ast<'a> {
    /// Stable identifier referencing the AST inside the parse-worker's cache.
    pub ast_id: u64,
    /// Lifetime guard so callers can attach extra references later.
    pub lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a> Ast<'a> {
    /// Build a borrowed AST handle from an id.
    #[must_use]
    pub fn new(ast_id: u64) -> Self {
        Self {
            ast_id,
            lifetime: std::marker::PhantomData,
        }
    }
}

/// Severity tier for a [`Finding`]. Ordered from most to least urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Active vulnerability or breakage. Block merge.
    Critical,
    /// Definite rule violation. Must be fixed before release.
    Error,
    /// Probable issue or stylistic violation. Fix when possible.
    Warning,
    /// Informational signal — drift or hint, not a defect.
    Info,
}

impl Severity {
    /// Human-readable label used in CLI output and the vision app.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// A single rule violation discovered by a scanner. Persisted into
/// `findings.db` by the store-worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable rule identifier, e.g. `theme.hardcoded-hex`.
    pub rule_id: String,
    /// Severity tier.
    pub severity: Severity,
    /// Absolute path of the offending file (already-canonicalized by caller).
    pub file: String,
    /// 1-based line number where the offending span starts.
    pub line_start: u32,
    /// 1-based line number where the offending span ends (inclusive).
    pub line_end: u32,
    /// 0-based column where the offending span starts.
    pub column_start: u32,
    /// 0-based column where the offending span ends.
    pub column_end: u32,
    /// Human-readable description of what is wrong.
    pub message: String,
    /// Optional verbatim suggestion. When `auto_fixable == true` this is the
    /// drop-in replacement. The §25.13 live-corrective drift mode uses this
    /// as the one-keystroke patch in the Command Center.
    pub suggestion: Option<String>,
    /// Whether the finding can be safely auto-applied.
    pub auto_fixable: bool,
}

impl Finding {
    /// Convenience builder for the common case (single line, no fix).
    #[must_use]
    pub fn new_line(
        rule_id: impl Into<String>,
        severity: Severity,
        file: impl Into<String>,
        line: u32,
        column_start: u32,
        column_end: u32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            file: file.into(),
            line_start: line,
            line_end: line,
            column_start,
            column_end,
            message: message.into(),
            suggestion: None,
            auto_fixable: false,
        }
    }

    /// Attach an auto-fix suggestion.
    #[must_use]
    pub fn with_fix(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self.auto_fixable = true;
        self
    }
}

/// Trait every scanner module implements. Implementations MUST be `Sync`
/// + `Send` because the worker pool dispatches them across tokio tasks.
pub trait Scanner: Send + Sync {
    /// Stable scanner name (lowercase, no spaces). Used for filtering
    /// (`ScanJob::scanner_filter`) and for telemetry.
    fn name(&self) -> &str;

    /// Cheap, side-effect-free check answering "should I be invoked on this
    /// file at all?" Most scanners gate on file extension. Returning `false`
    /// here prevents the worker from copying the content into the scanner.
    fn applies_to(&self, file: &Path) -> bool;

    /// Run the scanner. Implementations must NOT do file I/O — the worker
    /// has already loaded `content`.
    ///
    /// `ast` is provided when a parse-worker has cached a Tree-sitter tree
    /// for `file`; scanners that don't need it ignore the argument.
    fn scan(&self, file: &Path, content: &str, ast: Option<Ast<'_>>) -> Vec<Finding>;
}

/// Helper to compute a 1-based line number and 0-based column for a
/// byte offset inside `content`. Used by every scanner.
#[must_use]
pub fn line_col_of(content: &str, byte_offset: usize) -> (u32, u32) {
    let mut line: u32 = 1;
    let mut col: u32 = 0;
    let upper = byte_offset.min(content.len());
    for (i, b) in content.as_bytes().iter().enumerate() {
        if i >= upper {
            break;
        }
        if *b == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
