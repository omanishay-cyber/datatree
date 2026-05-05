//! # mneme-parsers-rs — Rust SDK
//!
//! Parse source files into a typed code graph using mneme's tree-sitter engine.
//! No daemon required; runs entirely in-process.
//!
//! ## Quick start
//!
//! ```no_run
//! use mneme_parsers_rs::{parse_file, parse_source};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), mneme_parsers_rs::ParseError> {
//!     // From a file path — language detected from extension
//!     let graph = parse_file("src/main.rs").await?;
//!     println!("{} nodes, {} edges", graph.nodes.len(), graph.edges.len());
//!
//!     // From in-memory source — language supplied explicitly
//!     let graph = parse_source("rust", "fn add(a: i32, b: i32) -> i32 { a + b }").await?;
//!     println!("{} nodes", graph.nodes.len());
//!     Ok(())
//! }
//! ```
//!
//! ## Types
//!
//! - [`Graph`] — extraction result containing nodes and edges
//! - [`Node`] — a single code element (function, class, import, …)
//! - [`Edge`] — a directed relationship between two nodes
//! - [`NodeKind`] — discriminant for what a node represents
//! - [`EdgeKind`] — discriminant for what a relationship means
//! - [`ParseError`] — typed errors that can be matched exhaustively

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

// ---------------------------------------------------------------------------
// Re-export the public graph types from the internal parsers crate.
// These are the only types callers need to know about.
// ---------------------------------------------------------------------------

pub use mneme_parsers::job::{Edge, EdgeKind, Node, NodeKind};

// ---------------------------------------------------------------------------
// Error type — wraps ParserError with SDK-friendly variants
// ---------------------------------------------------------------------------

use std::path::Path;
use thiserror::Error;

/// All errors that [`parse_file`] and [`parse_source`] can produce.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The file extension or filename did not map to any supported language.
    ///
    /// Pass an explicit language string to [`parse_source`] instead, or rename
    /// the file to use a supported extension.
    #[error("cannot determine language from path `{path}` — use parse_source(language, source) to specify it explicitly")]
    UnknownLanguage {
        /// The path whose extension was not recognised.
        path: String,
    },

    /// The language string passed to [`parse_source`] was not recognised.
    ///
    /// Valid values include: `"rust"`, `"python"`, `"typescript"`, `"javascript"`,
    /// `"go"`, `"java"`, `"c"`, `"cpp"`, `"csharp"`, `"ruby"`, `"php"`,
    /// `"bash"`, `"json"`, `"toml"`, `"yaml"`, `"markdown"`, and Tier-2 variants
    /// (`"swift"`, `"kotlin"`, `"scala"`, `"julia"`, `"zig"`, `"haskell"`,
    /// `"svelte"`, `"solidity"`).
    #[error("unknown language `{language}` — check the supported language list in the docs")]
    UnknownLanguageStr {
        /// The unrecognised language string.
        language: String,
    },

    /// The language is known but was not compiled into this build.
    ///
    /// Enable the corresponding Cargo feature (e.g. `swift`) and recompile.
    #[error("language `{language}` is supported but not enabled in this build (add the `{language}` feature to Cargo.toml)")]
    LanguageNotEnabled {
        /// The language that was disabled at compile time.
        language: String,
    },

    /// The source could not be parsed (parser returned no tree — timeout or
    /// cancellation).
    #[error("parsing failed for `{path}` — the parser returned no tree (possible timeout)")]
    ParseFailed {
        /// The path being parsed when failure occurred.
        path: String,
    },

    /// An I/O error occurred while reading the file from disk.
    #[error("I/O error reading `{path}`: {source}")]
    Io {
        /// The path that could not be read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An internal parser error that doesn't fit the above categories.
    ///
    /// This is always a bug; please file an issue with the error text.
    #[error("internal parser error: {0}")]
    Internal(String),
}

impl From<mneme_parsers::ParserError> for ParseError {
    fn from(e: mneme_parsers::ParserError) -> Self {
        use mneme_parsers::ParserError as PE;
        match e {
            PE::LanguageNotEnabled(l) => ParseError::LanguageNotEnabled { language: l },
            PE::UnknownLanguage(p) => ParseError::UnknownLanguage {
                path: p.to_string_lossy().into_owned(),
            },
            PE::ParseFailed(p) => ParseError::ParseFailed {
                path: p.to_string_lossy().into_owned(),
            },
            PE::Io { path, source } => ParseError::Io {
                path: path.to_string_lossy().into_owned(),
                source,
            },
            other => ParseError::Internal(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Graph — the public result type
// ---------------------------------------------------------------------------

/// The output of a parse operation: a graph of nodes and directed edges.
///
/// Nodes represent code elements (functions, classes, imports, etc.).
/// Edges represent relationships (calls, contains, imports, inherits, etc.).
///
/// # JSON representation
///
/// Both `Node` and `Edge` implement `serde::Serialize`/`Deserialize`.
/// You can round-trip through JSON with `serde_json`:
///
/// ```
/// # let graph = mneme_parsers_rs::Graph { nodes: vec![], edges: vec![] };
/// let json = serde_json::to_string_pretty(&graph).unwrap();
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Graph {
    /// All code elements extracted from the parsed source.
    pub nodes: Vec<Node>,
    /// All directed relationships between nodes.
    pub edges: Vec<Edge>,
}

// ---------------------------------------------------------------------------
// parse_file — entry point for on-disk files
// ---------------------------------------------------------------------------

/// Parse the file at `path` and return its code graph.
///
/// The language is detected automatically from the file extension. For
/// languages that require an explicit hint (e.g. in-memory snippets),
/// use [`parse_source`] instead.
///
/// # Errors
///
/// - [`ParseError::UnknownLanguage`] — unrecognised extension
/// - [`ParseError::LanguageNotEnabled`] — grammar not compiled in
/// - [`ParseError::Io`] — could not read the file
/// - [`ParseError::ParseFailed`] — tree-sitter returned no tree
///
/// # Example
///
/// ```no_run
/// #[tokio::main]
/// async fn main() -> Result<(), mneme_parsers_rs::ParseError> {
///     let graph = mneme_parsers_rs::parse_file("src/lib.rs").await?;
///     for node in &graph.nodes {
///         println!("{:?} {}", node.kind, node.name);
///     }
///     Ok(())
/// }
/// ```
pub async fn parse_file(path: impl AsRef<Path>) -> Result<Graph, ParseError> {
    let path = path.as_ref();

    let language = mneme_parsers::Language::from_filename(path).ok_or_else(|| {
        ParseError::UnknownLanguage {
            path: path.to_string_lossy().into_owned(),
        }
    })?;

    let bytes = tokio::fs::read(path).await.map_err(|e| ParseError::Io {
        path: path.to_string_lossy().into_owned(),
        source: e,
    })?;

    parse_bytes(path, language, bytes).await
}

// ---------------------------------------------------------------------------
// parse_source — entry point for in-memory strings
// ---------------------------------------------------------------------------

/// Parse an in-memory `source` string for the given `language` identifier.
///
/// `language` is a case-insensitive string matching `Language::as_str()`.
/// Valid values: `"rust"`, `"python"`, `"typescript"`, `"tsx"`,
/// `"javascript"`, `"jsx"`, `"go"`, `"java"`, `"c"`, `"cpp"`, `"csharp"`,
/// `"ruby"`, `"php"`, `"bash"`, `"json"`, `"toml"`, `"yaml"`, `"markdown"`,
/// `"swift"`, `"kotlin"`, `"scala"`, `"julia"`, `"zig"`, `"haskell"`,
/// `"svelte"`, `"solidity"`.
///
/// # Errors
///
/// - [`ParseError::UnknownLanguageStr`] — unrecognised language string
/// - [`ParseError::LanguageNotEnabled`] — grammar not compiled in
/// - [`ParseError::ParseFailed`] — tree-sitter returned no tree
///
/// # Example
///
/// ```no_run
/// #[tokio::main]
/// async fn main() -> Result<(), mneme_parsers_rs::ParseError> {
///     let src = "def greet(name):\n    return f'Hello, {name}'\n";
///     let graph = mneme_parsers_rs::parse_source("python", src).await?;
///     assert!(!graph.nodes.is_empty());
///     Ok(())
/// }
/// ```
pub async fn parse_source(language: &str, source: &str) -> Result<Graph, ParseError> {
    let lang_id = language_from_str(language)?;
    let synthetic_path = std::path::PathBuf::from(format!("<source>.{}", lang_id.as_str()));
    parse_bytes(&synthetic_path, lang_id, source.as_bytes().to_vec()).await
}

// ---------------------------------------------------------------------------
// Internal shared implementation
// ---------------------------------------------------------------------------

async fn parse_bytes(
    path: &Path,
    language: mneme_parsers::Language,
    bytes: Vec<u8>,
) -> Result<Graph, ParseError> {
    use mneme_parsers::{Extractor, IncrementalParser, ParserPool};
    use std::sync::Arc;

    let pool = Arc::new(ParserPool::new(1).map_err(ParseError::from)?);
    let inc = IncrementalParser::new(pool);

    let result = inc
        .parse_file(path, language, Arc::new(bytes.clone()))
        .await
        .map_err(ParseError::from)?;

    let extractor = Extractor::new(language);
    let extracted = extractor
        .extract(&result.tree, &bytes, path)
        .map_err(ParseError::from)?;

    Ok(Graph {
        nodes: extracted.nodes,
        edges: extracted.edges,
    })
}

/// Map a user-supplied language string to the internal [`Language`] enum.
fn language_from_str(s: &str) -> Result<mneme_parsers::Language, ParseError> {
    let lower = s.to_ascii_lowercase();
    // Try extension-style lookup first (handles aliases like "ts", "py").
    if let Some(l) = mneme_parsers::Language::from_extension(&lower) {
        return Ok(l);
    }
    // Then try the canonical as_str() names.
    for lang in mneme_parsers::Language::ALL {
        if lang.as_str() == lower.as_str() {
            return Ok(*lang);
        }
    }
    Err(ParseError::UnknownLanguageStr {
        language: s.to_owned(),
    })
}
