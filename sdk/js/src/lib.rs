//! # Node.js bindings for mneme-parsers
//!
//! Exposes two async functions to JavaScript / TypeScript:
//!
//! - `parseFile(path: string): Promise<Graph>`
//! - `parseSource(language: string, source: string): Promise<Graph>`
//!
//! Internal types (`ParserPool`, `IncrementalParser`, `ParserLease`, etc.)
//! are NOT exposed — the public surface is intentionally minimal.

#![allow(dead_code)]

use mneme_parsers::{EdgeKind, Extractor, IncrementalParser, Language, NodeKind, ParserPool};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Public JS-visible types
// ---------------------------------------------------------------------------

/// A single code element extracted from a source file.
#[napi(object)]
pub struct Node {
    /// Stable content-addressed identifier (blake3 hash prefix).
    pub id: String,
    /// Element type: `"function"` | `"class"` | `"import"` | `"file"` |
    /// `"method"` | `"struct"` | `"trait"` | `"interface"` | `"enum"` |
    /// `"module"` | `"variable"` | `"constant"` | `"decorator"` | `"comment"`.
    pub kind: String,
    /// Human-readable identifier (empty string for anonymous elements).
    pub name: String,
    /// Absolute or synthetic path to the file containing this node.
    pub path: String,
    /// 1-indexed start line.
    pub line: u32,
    /// 1-indexed end line (inclusive).
    pub end_line: u32,
}

/// A directed relationship between two nodes in the code graph.
#[napi(object)]
pub struct Edge {
    /// Node id of the origin.
    pub source: String,
    /// Node id of the destination.
    pub target: String,
    /// Relationship type: `"calls"` | `"contains"` | `"imports"` |
    /// `"inherits"` | `"implements"` | `"decorated_by"` | `"generic"`.
    pub kind: String,
}

/// The result of a parse operation.
#[napi(object)]
pub struct Graph {
    /// All code elements extracted from the parsed source.
    pub nodes: Vec<Node>,
    /// All directed relationships between nodes.
    pub edges: Vec<Edge>,
}

// ---------------------------------------------------------------------------
// Conversions from internal types
// ---------------------------------------------------------------------------

fn node_kind_str(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Function => "function",
        NodeKind::Method => "method",
        NodeKind::Class => "class",
        NodeKind::Struct => "struct",
        NodeKind::Trait => "trait",
        NodeKind::Interface => "interface",
        NodeKind::Enum => "enum",
        NodeKind::Module => "module",
        NodeKind::Variable => "variable",
        NodeKind::Constant => "constant",
        NodeKind::Decorator => "decorator",
        NodeKind::Comment => "comment",
        NodeKind::Import => "import",
        NodeKind::File => "file",
    }
}

fn edge_kind_str(k: EdgeKind) -> &'static str {
    match k {
        EdgeKind::Calls => "calls",
        EdgeKind::Inherits => "inherits",
        EdgeKind::Implements => "implements",
        EdgeKind::DecoratedBy => "decorated_by",
        EdgeKind::Imports => "imports",
        EdgeKind::Contains => "contains",
        EdgeKind::Generic => "generic",
    }
}

fn to_js_graph(extracted: mneme_parsers::ExtractedGraph) -> Graph {
    let nodes = extracted
        .nodes
        .into_iter()
        .map(|n| Node {
            id: n.id,
            kind: node_kind_str(n.kind).to_owned(),
            name: n.name,
            path: n.file.to_string_lossy().into_owned(),
            line: n.line_range.0 as u32,
            end_line: n.line_range.1 as u32,
        })
        .collect();

    let edges = extracted
        .edges
        .into_iter()
        .map(|e| Edge {
            source: e.from,
            target: e.to,
            kind: edge_kind_str(e.kind).to_owned(),
        })
        .collect();

    Graph { nodes, edges }
}

// ---------------------------------------------------------------------------
// Error helper — `napi::Error` is `Error<Status>` by default.
// We build all errors using Status variants directly to avoid the
// `Error<Error>` nested generic that causes type-check failures.
// ---------------------------------------------------------------------------

fn invalid_arg(msg: impl Into<String>) -> napi::Error {
    napi::Error::new(Status::InvalidArg, msg.into())
}

fn runtime_err(msg: impl Into<String>) -> napi::Error {
    napi::Error::new(Status::GenericFailure, msg.into())
}

// ---------------------------------------------------------------------------
// Language lookup
// ---------------------------------------------------------------------------

fn language_from_str(s: &str) -> std::result::Result<Language, napi::Error> {
    let lower = s.to_ascii_lowercase();
    Language::from_extension(&lower)
        .or_else(|| {
            Language::ALL
                .iter()
                .find(|l| l.as_str() == lower.as_str())
                .copied()
        })
        .ok_or_else(|| {
            invalid_arg(format!(
                "unknown language `{s}` — check the supported language list in the docs"
            ))
        })
}

// ---------------------------------------------------------------------------
// Core async implementation
// ---------------------------------------------------------------------------

async fn do_parse(
    path: std::path::PathBuf,
    language: Language,
    bytes: Vec<u8>,
) -> std::result::Result<Graph, napi::Error> {
    let pool = Arc::new(ParserPool::new(1).map_err(|e| runtime_err(e.to_string()))?);
    let inc = IncrementalParser::new(pool);

    let result = inc
        .parse_file(&path, language, Arc::new(bytes.clone()))
        .await
        .map_err(|e| runtime_err(e.to_string()))?;

    let extractor = Extractor::new(language);
    let extracted = extractor
        .extract(&result.tree, &bytes, &path)
        .map_err(|e| runtime_err(e.to_string()))?;

    Ok(to_js_graph(extracted))
}

// ---------------------------------------------------------------------------
// Exported async functions
// ---------------------------------------------------------------------------

/// Parse the file at `path` and return a Promise that resolves to a Graph.
///
/// The language is detected automatically from the file extension.
///
/// @param path - Absolute or relative path to the source file.
/// @returns Promise<Graph>
///
/// @example
/// ```ts
/// import { parseFile } from '@mneme/parsers';
/// const g = await parseFile('src/lib.rs');
/// console.log(`${g.nodes.length} nodes, ${g.edges.length} edges`);
/// ```
#[napi]
pub async fn parse_file(path: String) -> std::result::Result<Graph, napi::Error> {
    let pb = std::path::PathBuf::from(&path);

    let language = Language::from_filename(&pb).ok_or_else(|| {
        invalid_arg(format!(
            "cannot determine language from path `{path}` — \
         use parseSource(language, source) to specify it explicitly"
        ))
    })?;

    let bytes = tokio::fs::read(&pb)
        .await
        .map_err(|e| runtime_err(format!("I/O error reading `{path}`: {e}")))?;

    do_parse(pb, language, bytes).await
}

/// Parse an in-memory `source` string for the given `language` identifier.
///
/// @param language - Language identifier string.
/// @param source   - The source code to parse.
/// @returns Promise<Graph>
///
/// @example
/// ```ts
/// import { parseSource } from '@mneme/parsers';
/// const g = await parseSource('python', 'def greet(name):\n    return f"Hello, {name}"\n');
/// const fns = g.nodes.filter(n => n.kind === 'function');
/// console.log(fns[0].name); // 'greet'
/// ```
#[napi]
pub async fn parse_source(
    language: String,
    source: String,
) -> std::result::Result<Graph, napi::Error> {
    let lang = language_from_str(&language)?;
    let synthetic_path = std::path::PathBuf::from(format!("<source>.{}", lang.as_str()));
    do_parse(synthetic_path, lang, source.into_bytes()).await
}
