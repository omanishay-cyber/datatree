//! ParseJob / ParseResult — the IPC-shaped types passed across MPSC channels.
//!
//! These mirror the shared graph types datatree's `common` crate will own;
//! when that crate exists they can be re-exported under
//! `parsers::job::{Node, Edge, ...}`. Until then the local definitions are
//! authoritative and the rest of the daemon imports from here.

use crate::language::Language;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Confidence — datatree's three-level extraction trust scale (§21.6.2 row 11)
// ---------------------------------------------------------------------------

/// How much trust to place in an extracted node or edge.
///
/// Tags propagate from the extractor all the way to the graph store so
/// downstream queries can filter by confidence (e.g. "only EXTRACTED").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// Directly observed in the syntax tree, no inference required.
    Extracted,
    /// Resolved via heuristic / partial information (e.g. import alias).
    Inferred,
    /// Touched by a syntax error or otherwise uncertain — graph is still
    /// built, but consumers should treat the row carefully (§25.10).
    Ambiguous,
}

impl Confidence {
    /// Coarse numeric weight used when ranking (matches §21.6.2 row 11).
    pub fn weight(self) -> f32 {
        match self {
            Confidence::Extracted => 1.0,
            Confidence::Inferred => 0.6,
            Confidence::Ambiguous => 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// What kind of program element a [`Node`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Function,
    Method,
    Class,
    Struct,
    Trait,
    Interface,
    Enum,
    Module,
    Variable,
    Constant,
    Decorator,
    Comment,
    Import,
    File,
}

/// A single graph node extracted from a parsed file.
///
/// Stable serialization keys: every field is part of the public IPC schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Stable ID — `blake3(file_path || ":" || start_byte || ":" || kind)`.
    pub id: String,
    /// What this node is.
    pub kind: NodeKind,
    /// Human-readable name (function/class/etc identifier; "" if anonymous).
    pub name: String,
    /// Path to the file the node lives in (canonical, absolute).
    pub file: PathBuf,
    /// Byte range in the file at parse time.
    pub byte_range: (usize, usize),
    /// Line range (1-indexed, inclusive on both ends).
    pub line_range: (usize, usize),
    /// Source language.
    pub language: Language,
    /// Trust tag — see [`Confidence`].
    pub confidence: Confidence,
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// What relationship an [`Edge`] expresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// `caller --calls--> callee`
    Calls,
    /// `child --inherits--> parent`
    Inherits,
    /// `class --implements--> trait_or_interface`
    Implements,
    /// `function --decorated_by--> decorator`
    DecoratedBy,
    /// `file --imports--> module_or_file`
    Imports,
    /// `module --contains--> sub_node`
    Contains,
    /// Anything else — kept generic so new extractors don't require a schema bump.
    Generic,
}

/// A directed graph edge between two [`Node`] ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    /// Optional human-readable target text — used when `to` cannot yet be
    /// resolved (e.g. an import path before the file is parsed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_target: Option<String>,
}

// ---------------------------------------------------------------------------
// SyntaxIssue
// ---------------------------------------------------------------------------

/// A captured `(ERROR)` or `(MISSING)` node from the parsed tree.
///
/// Per §25.10 we record these but still build the graph; the issue is
/// surfaced via `confidence: AMBIGUOUS` on adjacent nodes / edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyntaxIssue {
    pub kind: SyntaxIssueKind,
    pub byte_range: (usize, usize),
    pub line_range: (usize, usize),
    /// Best-effort human-readable hint (e.g. "missing `}`").
    pub hint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxIssueKind {
    Error,
    Missing,
}

// ---------------------------------------------------------------------------
// ParseJob
// ---------------------------------------------------------------------------

/// One unit of work for a parse worker.
///
/// `content` is shared via [`Arc`] so re-tries don't re-clone the bytes.
#[derive(Debug, Clone)]
pub struct ParseJob {
    /// Canonical absolute path the source came from.
    pub file_path: PathBuf,
    /// Language of the source — caller must have already mapped extension.
    pub language: Language,
    /// File contents (UTF-8). Arc-wrapped because the same bytes may be
    /// fed to multiple downstream consumers (extractor + drift scanner).
    pub content: Arc<Vec<u8>>,
    /// If present, hints to the [`IncrementalParser`] which prior tree to
    /// reuse — `None` means "first parse" or "tree was evicted".
    pub prev_tree_id: Option<u64>,
    /// Optional content hash to short-circuit parses on identical bytes.
    /// `None` means "always parse"; the file watcher fills this when known.
    pub content_hash: Option<[u8; 32]>,
    /// Caller-supplied id used to correlate the result back to the request.
    pub job_id: u64,
}

impl ParseJob {
    /// Convenience constructor for tests.
    pub fn new(path: impl Into<PathBuf>, language: Language, content: Vec<u8>) -> Self {
        Self {
            file_path: path.into(),
            language,
            content: Arc::new(content),
            prev_tree_id: None,
            content_hash: None,
            job_id: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// ParseResult
// ---------------------------------------------------------------------------

/// What a worker hands back when a [`ParseJob`] is complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub job_id: u64,
    pub file_path: PathBuf,
    pub language: Language,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub syntax_errors: Vec<SyntaxIssue>,
    /// Wall-clock time spent inside `Parser::parse` and the extractor.
    pub parse_duration_ms: u64,
    /// Whether the previous tree was reused (true → incremental path).
    pub incremental: bool,
}

impl ParseResult {
    /// Empty result — used by the worker when content was unchanged
    /// (content_hash matched the cached entry).
    pub fn unchanged(job: &ParseJob) -> Self {
        Self {
            job_id: job.job_id,
            file_path: job.file_path.clone(),
            language: job.language,
            nodes: Vec::new(),
            edges: Vec::new(),
            syntax_errors: Vec::new(),
            parse_duration_ms: 0,
            incremental: true,
        }
    }

    /// `parse_duration` setter that converts from `Duration`.
    pub fn with_duration(mut self, dur: Duration) -> Self {
        self.parse_duration_ms = dur.as_millis() as u64;
        self
    }
}
