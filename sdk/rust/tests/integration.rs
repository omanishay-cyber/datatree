//! Integration tests for the Rust SDK.
//!
//! These tests exercise the public API surface (`parse_file` / `parse_source`)
//! end-to-end against real tree-sitter grammars — no daemon, no IPC, no mocks.

use mneme_parsers_rs::{parse_source, EdgeKind, NodeKind};
use std::io::Write as _;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// parse_source — happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sdk_parse_source_rust_extracts_functions() {
    let src = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }
fn helper() -> i32 { 42 }
"#;
    let graph = parse_source("rust", src).await.expect("parse");
    let fns: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        fns.iter().any(|n| n.name == "add"),
        "add missing: {:?}",
        fns
    );
    assert!(
        fns.iter().any(|n| n.name == "helper"),
        "helper missing: {:?}",
        fns
    );
}

#[tokio::test]
async fn sdk_parse_source_python_extracts_class_and_method() {
    let src = "\
class Dog:\n\
    def bark(self):\n\
        return 'woof'\n";
    let graph = parse_source("python", src).await.expect("parse");
    let classes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    let fns: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(classes.iter().any(|n| n.name == "Dog"), "Dog class missing");
    assert!(fns.iter().any(|n| n.name == "bark"), "bark method missing");
}

#[tokio::test]
async fn sdk_parse_source_typescript_emits_import_edges() {
    let src = "import { useState, useEffect } from 'react';\n\
               export function App() { return null; }\n";
    let graph = parse_source("typescript", src).await.expect("parse");
    let import_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Imports))
        .collect();
    // K7: 2 bindings → 2 edges
    assert_eq!(
        import_edges.len(),
        2,
        "expected 2 import edges (one per binding); got {:?}",
        import_edges
    );
}

// ---------------------------------------------------------------------------
// parse_file — reads from disk
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sdk_parse_file_detects_language_from_extension() {
    let mut tmp = NamedTempFile::with_suffix(".rs").expect("tempfile");
    tmp.write_all(b"pub fn hello() -> &'static str { \"hi\" }\n")
        .expect("write");
    tmp.flush().expect("flush");

    let graph = mneme_parsers_rs::parse_file(tmp.path())
        .await
        .expect("parse");
    let fns: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        fns.iter().any(|n| n.name == "hello"),
        "hello fn missing from parsed graph: {:?}",
        fns
    );
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sdk_parse_source_unknown_language_returns_error() {
    let result = parse_source("brainfuck", "+++").await;
    assert!(
        matches!(
            result,
            Err(mneme_parsers_rs::ParseError::UnknownLanguageStr { .. })
        ),
        "expected UnknownLanguageStr, got {:?}",
        result
    );
}

#[tokio::test]
async fn sdk_parse_file_nonexistent_path_returns_io_error() {
    let result = mneme_parsers_rs::parse_file("/nonexistent/path/that/does/not/exist.rs").await;
    assert!(
        matches!(result, Err(mneme_parsers_rs::ParseError::Io { .. })),
        "expected Io error, got {:?}",
        result
    );
}

#[tokio::test]
async fn sdk_parse_file_unknown_extension_returns_unknown_language() {
    let mut tmp = NamedTempFile::with_suffix(".xyz999").expect("tempfile");
    tmp.write_all(b"something").expect("write");
    tmp.flush().expect("flush");
    let result = mneme_parsers_rs::parse_file(tmp.path()).await;
    assert!(
        matches!(
            result,
            Err(mneme_parsers_rs::ParseError::UnknownLanguage { .. })
        ),
        "expected UnknownLanguage, got {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Graph structure invariants
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sdk_graph_always_contains_file_node() {
    let graph = parse_source("rust", "fn x() {}").await.expect("parse");
    assert!(
        graph.nodes.iter().any(|n| n.kind == NodeKind::File),
        "every graph must contain a File root node"
    );
}

#[tokio::test]
async fn sdk_graph_is_json_serialisable() {
    let graph = parse_source("python", "def f(): pass\n")
        .await
        .expect("parse");
    let json = serde_json::to_string_pretty(&graph).expect("serialize");
    let back: mneme_parsers_rs::Graph = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.nodes.len(), graph.nodes.len());
    assert_eq!(back.edges.len(), graph.edges.len());
}

// ---------------------------------------------------------------------------
// Language alias coverage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sdk_parse_source_accepts_extension_aliases() {
    // "ts" and "typescript" and "TypeScript" must all resolve.
    for alias in ["ts", "typescript", "TypeScript"] {
        let graph = parse_source(alias, "const x: number = 1;\n")
            .await
            .unwrap_or_else(|e| panic!("alias `{alias}` failed: {e}"));
        assert!(
            !graph.nodes.is_empty(),
            "alias `{alias}` produced empty graph"
        );
    }
}
