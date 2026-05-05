//! # Python bindings for mneme-parsers
//!
//! Exposes two public functions to Python:
//!
//! - `parse_file(path)` — parse a file from disk; language detected from extension
//! - `parse_source(language, source)` — parse an in-memory string
//!
//! Both return a `Graph` Python object with `.nodes` and `.edges` lists.
//!
//! Internal types (`ParserPool`, `IncrementalParser`, `ParserLease`, etc.) are
//! NOT exposed — the public surface is intentionally minimal.
//!
//! Thread-safety: pyo3 0.28 uses `py.detach()` (formerly `allow_threads`) to
//! release the GIL during the Tokio parse call. Callers may invoke these
//! functions from Python threads without issue.

// The #[pymodule] macro generates an inner module also named `mneme_parsers`,
// which would shadow the extern crate of the same name. We alias the crate to
// avoid the ambiguity.
extern crate mneme_parsers as mnp;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Runtime — one shared single-threaded Tokio runtime per process.
// ---------------------------------------------------------------------------

fn runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Tokio runtime init failed")
    })
}

// ---------------------------------------------------------------------------
// Python-visible types
// ---------------------------------------------------------------------------

/// A single code element extracted from a source file.
///
/// Attributes
/// ----------
/// id : str
///     Stable content-addressed identifier (blake3 hash prefix).
/// kind : str
///     Element type: ``"function"``, ``"class"``, ``"import"``, ``"file"``, etc.
/// name : str
///     Human-readable identifier (empty string for anonymous elements).
/// path : str
///     Absolute or synthetic path to the file containing this node.
/// line : int
///     1-indexed start line.
/// end_line : int
///     1-indexed end line (inclusive).
#[pyclass(name = "Node", frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyNode {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub line: usize,
    #[pyo3(get)]
    pub end_line: usize,
}

#[pymethods]
impl PyNode {
    fn __repr__(&self) -> String {
        format!(
            "Node(kind={:?}, name={:?}, path={:?}, line={})",
            self.kind, self.name, self.path, self.line
        )
    }
}

/// A directed relationship between two nodes in the code graph.
///
/// Attributes
/// ----------
/// source : str
///     Node id of the origin.
/// target : str
///     Node id of the destination.
/// kind : str
///     Relationship type: ``"calls"``, ``"contains"``, ``"imports"``, etc.
#[pyclass(name = "Edge", frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyEdge {
    #[pyo3(get)]
    pub source: String,
    #[pyo3(get)]
    pub target: String,
    #[pyo3(get)]
    pub kind: String,
}

#[pymethods]
impl PyEdge {
    fn __repr__(&self) -> String {
        format!(
            "Edge(source={:?}, target={:?}, kind={:?})",
            self.source, self.target, self.kind
        )
    }
}

/// The result of a parse operation.
///
/// Attributes
/// ----------
/// nodes : list[Node]
///     All code elements extracted from the parsed source.
/// edges : list[Edge]
///     All directed relationships between nodes.
#[pyclass(name = "Graph", frozen, skip_from_py_object)]
pub struct PyGraph {
    #[pyo3(get)]
    pub nodes: Vec<PyNode>,
    #[pyo3(get)]
    pub edges: Vec<PyEdge>,
}

#[pymethods]
impl PyGraph {
    fn __repr__(&self) -> String {
        format!(
            "Graph(nodes={}, edges={})",
            self.nodes.len(),
            self.edges.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Conversion from internal types
// ---------------------------------------------------------------------------

fn node_kind_str(k: mnp::NodeKind) -> &'static str {
    use mnp::NodeKind as NK;
    match k {
        NK::Function => "function",
        NK::Method => "method",
        NK::Class => "class",
        NK::Struct => "struct",
        NK::Trait => "trait",
        NK::Interface => "interface",
        NK::Enum => "enum",
        NK::Module => "module",
        NK::Variable => "variable",
        NK::Constant => "constant",
        NK::Decorator => "decorator",
        NK::Comment => "comment",
        NK::Import => "import",
        NK::File => "file",
    }
}

fn edge_kind_str(k: mnp::EdgeKind) -> &'static str {
    use mnp::EdgeKind as EK;
    match k {
        EK::Calls => "calls",
        EK::Inherits => "inherits",
        EK::Implements => "implements",
        EK::DecoratedBy => "decorated_by",
        EK::Imports => "imports",
        EK::Contains => "contains",
        EK::Generic => "generic",
    }
}

fn to_py_graph(extracted: mnp::ExtractedGraph) -> PyGraph {
    let nodes = extracted
        .nodes
        .into_iter()
        .map(|n| PyNode {
            id: n.id,
            kind: node_kind_str(n.kind).to_owned(),
            name: n.name,
            path: n.file.to_string_lossy().into_owned(),
            line: n.line_range.0,
            end_line: n.line_range.1,
        })
        .collect();

    let edges = extracted
        .edges
        .into_iter()
        .map(|e| PyEdge {
            source: e.from,
            target: e.to,
            kind: edge_kind_str(e.kind).to_owned(),
        })
        .collect();

    PyGraph { nodes, edges }
}

// ---------------------------------------------------------------------------
// Core implementation (runs inside Tokio, GIL released)
// ---------------------------------------------------------------------------

async fn do_parse_file(path: std::path::PathBuf) -> Result<PyGraph, String> {
    let language = mnp::Language::from_filename(&path)
        .ok_or_else(|| format!("cannot determine language from path `{}`", path.display()))?;

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("I/O error reading `{}`: {}", path.display(), e))?;

    run_extract(path, language, bytes).await
}

async fn do_parse_source(language_str: String, source: String) -> Result<PyGraph, String> {
    let lower = language_str.to_ascii_lowercase();
    let language = mnp::Language::from_extension(&lower)
        .or_else(|| {
            mnp::Language::ALL
                .iter()
                .find(|l| l.as_str() == lower.as_str())
                .copied()
        })
        .ok_or_else(|| format!("unknown language `{language_str}`"))?;

    let synthetic_path = std::path::PathBuf::from(format!("<source>.{}", language.as_str()));
    run_extract(synthetic_path, language, source.into_bytes()).await
}

async fn run_extract(
    path: std::path::PathBuf,
    language: mnp::Language,
    bytes: Vec<u8>,
) -> Result<PyGraph, String> {
    let pool = Arc::new(mnp::ParserPool::new(1).map_err(|e| e.to_string())?);
    let inc = mnp::IncrementalParser::new(pool);

    let result = inc
        .parse_file(&path, language, Arc::new(bytes.clone()))
        .await
        .map_err(|e| e.to_string())?;

    let extractor = mnp::Extractor::new(language);
    let extracted = extractor
        .extract(&result.tree, &bytes, &path)
        .map_err(|e| e.to_string())?;

    Ok(to_py_graph(extracted))
}

// ---------------------------------------------------------------------------
// Public Python-facing functions
// ---------------------------------------------------------------------------

/// Parse the file at *path* and return its code graph.
///
/// The language is detected automatically from the file extension.
///
/// Parameters
/// ----------
/// path : str
///     Path to the source file.
///
/// Returns
/// -------
/// Graph
///
/// Raises
/// ------
/// ValueError
///     If the language cannot be determined or the file cannot be read.
#[pyfunction]
fn parse_file(py: Python<'_>, path: &str) -> PyResult<PyGraph> {
    let path = std::path::PathBuf::from(path);
    let rt = runtime();
    // pyo3 0.28: GIL release uses py.detach() (was allow_threads in 0.23-).
    py.detach(|| {
        rt.block_on(do_parse_file(path))
            .map_err(PyValueError::new_err)
    })
}

/// Parse an in-memory *source* string for the given *language*.
///
/// Parameters
/// ----------
/// language : str
///     Language identifier, case-insensitive.
/// source : str
///     The source code to parse.
///
/// Returns
/// -------
/// Graph
///
/// Raises
/// ------
/// ValueError
///     If the language is unknown or not enabled in this build.
#[pyfunction]
fn parse_source(py: Python<'_>, language: &str, source: &str) -> PyResult<PyGraph> {
    let lang = language.to_owned();
    let src = source.to_owned();
    let rt = runtime();
    py.detach(|| {
        rt.block_on(do_parse_source(lang, src))
            .map_err(PyValueError::new_err)
    })
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// mneme_parsers — tree-sitter-based code graph extraction.
#[pymodule]
fn mneme_parsers(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_file, m)?)?;
    m.add_function(wrap_pyfunction!(parse_source, m)?)?;
    m.add_class::<PyGraph>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyEdge>()?;
    Ok(())
}
