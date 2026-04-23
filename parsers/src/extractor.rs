//! Extractor — turns a parsed `tree_sitter::Tree` into mneme
//! [`Node`]s and [`Edge`]s.
//!
//! Per §25.10 we ALWAYS query the ERROR/MISSING patterns and tag adjacent
//! extractions with `confidence: AMBIGUOUS` so the graph is still built on
//! files with syntax issues.

use crate::error::ParserError;
use crate::job::{
    Confidence, Edge, EdgeKind, Node, NodeKind, SyntaxIssue, SyntaxIssueKind,
};
use crate::language::Language;
use crate::query_cache::{get_query, QueryKind};
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node as TsNode, QueryCursor, Range, Tree};
// `QueryCursor::matches` returns a `StreamingIterator` as of tree-sitter
// 0.25 (the lending-iterator rewrite). The trait lives in the external
// `streaming-iterator` crate — tree-sitter accepts it as the return type
// but does not re-export it.

/// Combined output of [`Extractor::extract`].
#[derive(Debug, Clone, Default)]
pub struct ExtractedGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub issues: Vec<SyntaxIssue>,
}

impl ExtractedGraph {
    /// True when the parser found at least one ERROR/MISSING — callers can
    /// downgrade their confidence accordingly (mirrors §25.10).
    pub fn has_syntax_issues(&self) -> bool {
        !self.issues.is_empty()
    }
}

/// Stateless extractor. All inputs flow through `extract`.
#[derive(Debug)]
pub struct Extractor {
    language: Language,
}

impl Extractor {
    /// Build an extractor pinned to one language.
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    /// The language this extractor targets.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Walk the tree once, run every cached query, and assemble the graph.
    ///
    /// `bytes` is the source the tree was parsed from; required for name
    /// extraction since `tree_sitter::Node::utf8_text` borrows from it.
    pub fn extract(
        &self,
        tree: &Tree,
        bytes: &[u8],
        file_path: &Path,
    ) -> Result<ExtractedGraph, ParserError> {
        let mut out = ExtractedGraph::default();

        // 1. Errors first — so we can decide whether the rest is AMBIGUOUS.
        let issues = self.collect_errors(tree, bytes)?;
        let degrade = !issues.is_empty();
        out.issues = issues;

        let confidence = if degrade {
            Confidence::Ambiguous
        } else {
            Confidence::Extracted
        };

        // 2. The file itself is always a node — call sites need an anchor.
        let file_node = Node {
            id: stable_id(file_path, 0, NodeKind::File),
            kind: NodeKind::File,
            name: file_path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file: file_path.to_path_buf(),
            byte_range: (0, bytes.len()),
            line_range: (1, count_lines(bytes)),
            language: self.language,
            confidence: Confidence::Extracted,
        };
        let file_id = file_node.id.clone();
        out.nodes.push(file_node);

        // 3. Functions, classes, decorators, comments → nodes.
        self.collect_named(
            tree,
            bytes,
            file_path,
            QueryKind::Functions,
            NodeKind::Function,
            confidence,
            &file_id,
            &mut out,
        )?;
        self.collect_named(
            tree,
            bytes,
            file_path,
            QueryKind::Classes,
            NodeKind::Class,
            confidence,
            &file_id,
            &mut out,
        )?;
        self.collect_named(
            tree,
            bytes,
            file_path,
            QueryKind::Decorators,
            NodeKind::Decorator,
            confidence,
            &file_id,
            &mut out,
        )?;
        self.collect_named(
            tree,
            bytes,
            file_path,
            QueryKind::Comments,
            NodeKind::Comment,
            confidence,
            &file_id,
            &mut out,
        )?;

        // 4. Imports → Node + Edge(file --imports--> module)
        self.collect_imports(tree, bytes, file_path, confidence, &file_id, &mut out)?;

        // 5. Calls → Edge(enclosing_fn --calls--> callee). The callee target
        //    is left as `unresolved_target` for the brain crate to resolve
        //    cross-file.
        self.collect_calls(tree, bytes, file_path, confidence, &mut out)?;

        // 6. Inheritance / decoration relationships — best-effort per language.
        self.collect_inheritance(tree, bytes, file_path, confidence, &mut out)?;

        Ok(out)
    }

    // ---- helpers --------------------------------------------------------

    fn collect_errors(
        &self,
        tree: &Tree,
        bytes: &[u8],
    ) -> Result<Vec<SyntaxIssue>, ParserError> {
        let _ = bytes;
        let mut out = Vec::new();
        // Prefer the query path; if the grammar rejects the (ERROR) query
        // pattern (some stricter grammars do), fall back to a plain walk.
        match get_query(self.language, QueryKind::Errors) {
            Ok(q) => {
                let mut cursor = QueryCursor::new();
                let mut matches = cursor.matches(&q, tree.root_node(), bytes);
                while let Some(m) = matches.next() {
                    for cap in m.captures {
                        let n = cap.node;
                        let kind = if n.is_missing() {
                            SyntaxIssueKind::Missing
                        } else {
                            SyntaxIssueKind::Error
                        };
                        let r = n.range();
                        out.push(SyntaxIssue {
                            kind,
                            byte_range: (r.start_byte, r.end_byte),
                            line_range: (r.start_point.row + 1, r.end_point.row + 1),
                            hint: format!("{} at line {}", n.kind(), r.start_point.row + 1),
                        });
                    }
                }
            }
            Err(_) => {
                // Fall through to has_error walk.
            }
        }
        if tree.root_node().has_error() && out.is_empty() {
            walk_for_errors(tree.root_node(), &mut out);
        }
        Ok(out)
    }

    fn collect_named(
        &self,
        tree: &Tree,
        bytes: &[u8],
        file_path: &Path,
        query_kind: QueryKind,
        node_kind: NodeKind,
        confidence: Confidence,
        file_id: &str,
        out: &mut ExtractedGraph,
    ) -> Result<(), ParserError> {
        let q = get_query(self.language, query_kind)?;
        let name_idx = q.capture_index_for_name("name");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, tree.root_node(), bytes);
        while let Some(m) = matches.next() {
            // We always want the OUTER capture (the whole function/class).
            let outer = m.captures.last().map(|c| c.node);
            let Some(outer) = outer else { continue };
            let name = name_idx
                .and_then(|idx| {
                    m.captures
                        .iter()
                        .find(|c| c.index == idx)
                        .and_then(|c| c.node.utf8_text(bytes).ok())
                })
                .unwrap_or("")
                .to_string();
            let r = outer.range();
            let id = stable_id(file_path, r.start_byte, node_kind);
            out.nodes.push(Node {
                id: id.clone(),
                kind: node_kind,
                name,
                file: file_path.to_path_buf(),
                byte_range: (r.start_byte, r.end_byte),
                line_range: (r.start_point.row + 1, r.end_point.row + 1),
                language: self.language,
                confidence,
            });
            out.edges.push(Edge {
                from: file_id.to_string(),
                to: id,
                kind: EdgeKind::Contains,
                confidence,
                unresolved_target: None,
            });
        }
        Ok(())
    }

    fn collect_imports(
        &self,
        tree: &Tree,
        bytes: &[u8],
        file_path: &Path,
        confidence: Confidence,
        file_id: &str,
        out: &mut ExtractedGraph,
    ) -> Result<(), ParserError> {
        let q = get_query(self.language, QueryKind::Imports)?;
        let source_idx = q.capture_index_for_name("source");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, tree.root_node(), bytes);
        while let Some(m) = matches.next() {
            let outer = m.captures.last().map(|c| c.node);
            let Some(outer) = outer else { continue };
            let target = source_idx
                .and_then(|idx| {
                    m.captures
                        .iter()
                        .find(|c| c.index == idx)
                        .and_then(|c| c.node.utf8_text(bytes).ok())
                })
                .map(|s| s.trim_matches(|c| c == '"' || c == '\'').to_string())
                .unwrap_or_else(|| {
                    outer
                        .utf8_text(bytes)
                        .unwrap_or("<unknown>")
                        .lines()
                        .next()
                        .unwrap_or("<unknown>")
                        .trim()
                        .to_string()
                });
            let r = outer.range();
            let id = stable_id(file_path, r.start_byte, NodeKind::Import);
            out.nodes.push(Node {
                id: id.clone(),
                kind: NodeKind::Import,
                name: target.clone(),
                file: file_path.to_path_buf(),
                byte_range: (r.start_byte, r.end_byte),
                line_range: (r.start_point.row + 1, r.end_point.row + 1),
                language: self.language,
                confidence,
            });
            out.edges.push(Edge {
                from: file_id.to_string(),
                to: id,
                kind: EdgeKind::Imports,
                confidence,
                unresolved_target: Some(target),
            });
        }
        Ok(())
    }

    fn collect_calls(
        &self,
        tree: &Tree,
        bytes: &[u8],
        file_path: &Path,
        confidence: Confidence,
        out: &mut ExtractedGraph,
    ) -> Result<(), ParserError> {
        let q = get_query(self.language, QueryKind::Calls)?;
        let callee_idx = q.capture_index_for_name("callee");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, tree.root_node(), bytes);
        while let Some(m) = matches.next() {
            let call_node = m.captures.last().map(|c| c.node);
            let Some(call_node) = call_node else { continue };
            let callee_text = callee_idx
                .and_then(|idx| {
                    m.captures
                        .iter()
                        .find(|c| c.index == idx)
                        .and_then(|c| c.node.utf8_text(bytes).ok())
                })
                .unwrap_or("<unresolved>")
                .to_string();

            // Find the enclosing function/method node so we can attribute
            // the edge. If none, the call is module-top-level — anchor to
            // the file node.
            let enclosing = enclosing_callable(call_node, self.language);
            let from_id = match enclosing {
                Some(n) => stable_id(file_path, n.range().start_byte, NodeKind::Function),
                None => stable_id(file_path, 0, NodeKind::File),
            };
            let to_id = format!("call::{}::{}", file_path.display(), callee_text);
            out.edges.push(Edge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::Calls,
                confidence,
                unresolved_target: Some(callee_text),
            });
        }
        Ok(())
    }

    fn collect_inheritance(
        &self,
        tree: &Tree,
        bytes: &[u8],
        file_path: &Path,
        confidence: Confidence,
        out: &mut ExtractedGraph,
    ) -> Result<(), ParserError> {
        // Lightweight, language-by-language. Anything we don't understand
        // is silently skipped — the brain crate runs a fuller resolver.
        let mut cursor = tree.walk();
        for node in iter_all(tree.root_node(), &mut cursor) {
            let kind = node.kind();
            let parent_target: Option<String> = match (self.language, kind) {
                (Language::Python, "class_definition") => {
                    // class Foo(Bar, Baz): ...
                    node.child_by_field_name("superclasses")
                        .and_then(|s| s.utf8_text(bytes).ok())
                        .map(|s| s.trim_matches(|c| c == '(' || c == ')').to_string())
                }
                (Language::TypeScript | Language::Tsx | Language::JavaScript | Language::Jsx,
                    "class_declaration") => node
                    .child_by_field_name("heritage")
                    .or_else(|| node.child_by_field_name("superclass"))
                    .and_then(|s| s.utf8_text(bytes).ok())
                    .map(|s| s.to_string()),
                (Language::Java, "class_declaration") => node
                    .child_by_field_name("superclass")
                    .and_then(|s| s.utf8_text(bytes).ok())
                    .map(|s| s.to_string()),
                _ => None,
            };
            if let Some(target) = parent_target {
                let from_id = stable_id(file_path, node.range().start_byte, NodeKind::Class);
                let to_id = format!("ext::{}::{}", file_path.display(), target.trim());
                out.edges.push(Edge {
                    from: from_id,
                    to: to_id,
                    kind: EdgeKind::Inherits,
                    confidence,
                    unresolved_target: Some(target.trim().to_string()),
                });
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tree-walking helpers
// ---------------------------------------------------------------------------

fn iter_all<'a>(
    root: TsNode<'a>,
    cursor: &mut tree_sitter::TreeCursor<'a>,
) -> Vec<TsNode<'a>> {
    let mut out = Vec::new();
    cursor.reset(root);
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        for i in 0..n.child_count() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    out
}

fn walk_for_errors(root: TsNode<'_>, out: &mut Vec<SyntaxIssue>) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            let r = n.range();
            out.push(SyntaxIssue {
                kind: if n.is_missing() {
                    SyntaxIssueKind::Missing
                } else {
                    SyntaxIssueKind::Error
                },
                byte_range: (r.start_byte, r.end_byte),
                line_range: (r.start_point.row + 1, r.end_point.row + 1),
                hint: format!("{} at line {}", n.kind(), r.start_point.row + 1),
            });
        }
        for i in 0..n.child_count() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
}

/// Walk up from `node` to the enclosing function-like node for this language.
fn enclosing_callable<'a>(node: TsNode<'a>, lang: Language) -> Option<TsNode<'a>> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if is_callable_kind(n.kind(), lang) {
            return Some(n);
        }
        cur = n.parent();
    }
    None
}

fn is_callable_kind(kind: &str, lang: Language) -> bool {
    match lang {
        Language::Python => matches!(kind, "function_definition"),
        Language::Rust => matches!(kind, "function_item" | "function_signature_item"),
        Language::Go => matches!(kind, "function_declaration" | "method_declaration"),
        Language::Java | Language::CSharp => matches!(kind, "method_declaration"),
        Language::C | Language::Cpp => matches!(kind, "function_definition"),
        Language::Ruby => matches!(kind, "method"),
        Language::Php => matches!(kind, "function_definition" | "method_declaration"),
        Language::Bash => matches!(kind, "function_definition"),
        Language::TypeScript
        | Language::Tsx
        | Language::JavaScript
        | Language::Jsx => matches!(
            kind,
            "function_declaration"
                | "method_definition"
                | "function_expression"
                | "arrow_function"
        ),
        // --- Tier 2 community grammars ---------------------------------
        Language::Swift => matches!(kind, "function_declaration"),
        Language::Kotlin => matches!(kind, "function_declaration"),
        Language::Scala => matches!(kind, "function_definition" | "function_declaration"),
        Language::Solidity => {
            matches!(kind, "function_definition" | "modifier_definition")
        }
        Language::Julia => {
            matches!(kind, "function_definition" | "short_function_definition")
        }
        Language::Zig => matches!(kind, "FnProto"),
        Language::Haskell => matches!(kind, "function"),
        _ => matches!(kind, "function_declaration" | "function_definition"),
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn stable_id(path: &Path, start_byte: usize, kind: NodeKind) -> String {
    let mut h = blake3::Hasher::new();
    h.update(path.as_os_str().to_string_lossy().as_bytes());
    h.update(b":");
    h.update(start_byte.to_le_bytes().as_ref());
    h.update(b":");
    h.update(format!("{:?}", kind).as_bytes());
    let hash = h.finalize();
    format!("n_{}", &hash.to_hex().to_string()[..16])
}

fn count_lines(bytes: &[u8]) -> usize {
    1 + bytes.iter().filter(|&&b| b == b'\n').count()
}

#[allow(dead_code)]
fn range_to_tuple(r: Range) -> (usize, usize) {
    (r.start_byte, r.end_byte)
}

// Re-export for downstream type stability.
pub type GraphPath = PathBuf;
