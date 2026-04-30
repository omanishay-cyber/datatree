//! # parsers — mneme Tree-sitter worker pool
//!
//! A multi-language incremental parser pool used by the mneme daemon.
//!
//! Design references:
//! - §3.1 — process tree (this crate is the `parse-worker` binary)
//! - §21.3 — Tree-sitter Mastery (parser-per-worker, cached queries, incremental)
//! - §25.10 — Best practices (no shared parsers, query cache, ERROR/MISSING capture)
//!
//! Public entry points:
//! - [`Language`] — the supported-language enum
//! - [`ParserPool`] — borrows a [`tree_sitter::Parser`] per language per worker
//! - [`IncrementalParser`] — keeps previous trees in an LRU cache
//! - [`Extractor`] — turns a parsed [`tree_sitter::Tree`] into mneme
//!   [`Node`](job::Node) / [`Edge`](job::Edge) records
//! - [`Worker::run`] — async worker loop driving the above

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]
#![warn(rust_2018_idioms)]

pub mod dispatch;
pub mod error;
pub mod extractor;
pub mod incremental;
pub mod job;
pub mod language;
pub mod parser_pool;
pub mod query_cache;
pub mod worker;

#[cfg(test)]
mod tests;

pub use error::ParserError;
pub use extractor::{looks_like_test_path, ExtractedGraph, Extractor};
pub use incremental::IncrementalParser;
pub use job::{Confidence, Edge, EdgeKind, Node, NodeKind, ParseJob, ParseResult, SyntaxIssue};
pub use language::Language;
pub use parser_pool::{ParserLease, ParserPool};
pub use query_cache::{get_query, QueryKind};
pub use worker::Worker;
