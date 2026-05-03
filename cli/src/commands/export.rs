//! `mneme export --format=<fmt> -o <path>` — export the project's
//! code-graph to one of five portable, tool-friendly formats.
//!
//! v0.3.3 cocktail surface: the mneme graph stays the source of truth,
//! but the developer keeps their tool choices. Pipe the same project
//! into Yed/Cytoscape (GraphML), Obsidian (wiki-link markdown), Neo4j
//! (Cypher CREATE script), a static SVG diagram, or a JSON-LD knowledge
//! graph that any schema.org-aware consumer can read.
//!
//! Identity model:
//!   - Each node is keyed by its SQLite `id` (rendered as `n_<id>` in the
//!     output) so cross-format references stay stable.
//!   - Edges reference nodes by the same `n_<id>` token. Edges whose
//!     endpoint isn't in the exported node set (because of `--kinds` /
//!     `--files` / `--max-nodes`) are dropped silently — the export
//!     shouldn't reference dangling vertices.
//!
//! Format dispatcher (see [`run`]) reads the rows once, applies filters,
//! then hands the same `(nodes, edges)` slice to the chosen emitter so
//! every format sees the same view.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use tracing::info;

use crate::error::{CliError, CliResult};
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme export`.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Output format. Each one targets a different downstream tool —
    /// see the module-level docs for the mapping.
    #[arg(long, value_enum)]
    pub format: ExportFormat,
    /// Destination file. For `obsidian` the path is a directory (created
    /// if missing) OR a `.zip` to bundle into; every other format writes
    /// a single file.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Project root to resolve `graph.db` against. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// Restrict to specific node kinds (e.g. `function`, `struct`,
    /// `module`). Repeatable: `--kinds function --kinds struct`.
    #[arg(long)]
    pub kinds: Vec<String>,
    /// Glob filter on `file_path`. Same semantics as `mneme graph-diff`:
    /// `*` within a path component, `**` across `/`.
    #[arg(long)]
    pub files: Option<String>,
    /// Cap node count. Default = no cap. Useful for SVG legibility — the
    /// emitter will still render thousands of nodes if you ask, but the
    /// output stops being readable past ~500.
    #[arg(long)]
    pub max_nodes: Option<usize>,
    /// Include edges in the export. On by default; pass
    /// `--include-edges=false` for a node-only dump (handy for quick
    /// counts). Implemented as `Arg::action(Set)` so the value can be
    /// supplied either as `--include-edges false` or `--include-edges=false`,
    /// matching the `--format` flag's UX.
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
    )]
    pub include_edges: bool,
}

/// Output formats supported by [`run`]. New formats only need an
/// emitter function and a match arm in the dispatcher.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// `.graphml` — XML graph format. Imports cleanly into Yed,
    /// Cytoscape, Gephi, and igraph.
    Graphml,
    /// Directory (or `.zip`) of `.md` files using `[[wiki-link]]`
    /// syntax. Drop into an Obsidian vault to get a navigable
    /// knowledge graph.
    Obsidian,
    /// `.cypher` — Neo4j import script (`CREATE` statements wrapped
    /// in batched transactions).
    Cypher,
    /// `.svg` — pre-rendered force-directed-ish radial layout,
    /// coloured by node kind. Self-contained, no external CSS.
    Svg,
    /// `.jsonld` — schema.org `Dataset` + `DefinedTerm` document,
    /// semantic-web compatible.
    Jsonld,
}

/// One node row read out of `graph.db`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NodeRow {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
}

impl NodeRow {
    /// Stable cross-format identifier — `n_<sqlite_rowid>`.
    pub fn export_id(&self) -> String {
        format!("n_{}", self.id)
    }
    /// Filesystem-safe variant of `qualified_name` for Obsidian filenames.
    fn obsidian_filename(&self) -> String {
        let mut out = String::with_capacity(self.qualified_name.len() + 3);
        for c in self.qualified_name.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => out.push(c),
                ':' | '/' | '\\' | '<' | '>' | '|' | '?' | '*' | '"' | ' ' => out.push('_'),
                _ => out.push('_'),
            }
        }
        // Empty fallback — qualified_name is UNIQUE in the schema but a
        // synthetic graph could in theory put empty strings here.
        if out.is_empty() {
            out = format!("node_{}", self.id);
        }
        out.push_str(".md");
        out
    }
}

/// One edge row read out of `graph.db`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EdgeRow {
    pub kind: String,
    pub source_qualified: String,
    pub target_qualified: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: ExportArgs) -> CliResult<()> {
    let project_root = resolve_project_root(args.project.clone());
    let db_path = live_graph_db(&project_root)?;
    if !db_path.exists() {
        return Err(CliError::Other(format!(
            "graph.db not found at {}. Run `mneme build .` first.",
            db_path.display()
        )));
    }

    info!(path = %db_path.display(), "export: reading graph.db");

    let include_edges = args.include_edges;
    let kinds_filter: Option<HashSet<String>> = if args.kinds.is_empty() {
        None
    } else {
        Some(args.kinds.iter().cloned().collect())
    };
    let files_pat = args.files.clone();
    let max_nodes = args.max_nodes;

    let db_for_block = db_path.clone();
    let join_result =
        tokio::task::spawn_blocking(move || -> CliResult<(Vec<NodeRow>, Vec<EdgeRow>)> {
            let conn = open_ro(&db_for_block)?;
            let mut nodes = load_nodes(&conn)?;
            // Apply filters in-memory so every emitter sees the same slice.
            if let Some(ref allowed) = kinds_filter {
                nodes.retain(|n| allowed.contains(&n.kind));
            }
            if let Some(ref pat) = files_pat {
                let matcher = GlobMatcher::compile(pat);
                nodes.retain(|n| n.file_path.as_deref().is_some_and(|p| matcher.matches(p)));
            }
            if let Some(cap) = max_nodes {
                if nodes.len() > cap {
                    nodes.truncate(cap);
                }
            }
            let edges = if include_edges {
                // Keep only edges whose endpoints survived filtering.
                let surviving: HashSet<String> =
                    nodes.iter().map(|n| n.qualified_name.clone()).collect();
                let raw = load_edges(&conn)?;
                raw.into_iter()
                    .filter(|e| {
                        surviving.contains(&e.source_qualified)
                            && surviving.contains(&e.target_qualified)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Ok((nodes, edges))
        })
        .await
        .map_err(|e| CliError::Other(format!("export worker join failed: {e}")))?;
    let (nodes, edges) = join_result?;

    info!(
        nodes = nodes.len(),
        edges = edges.len(),
        "export: filters applied"
    );

    // Make sure the parent of `--output` exists. Single-file emitters
    // need this; the obsidian emitter creates its own dir.
    if !matches!(args.format, ExportFormat::Obsidian) {
        if let Some(parent) = args.output.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| CliError::io(parent, e))?;
            }
        }
    }

    let project_label = project_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    match args.format {
        ExportFormat::Graphml => emit_graphml(&nodes, &edges, &args.output)?,
        ExportFormat::Obsidian => emit_obsidian(&nodes, &edges, &args.output)?,
        ExportFormat::Cypher => emit_cypher(&nodes, &edges, &args.output)?,
        ExportFormat::Svg => emit_svg(&nodes, &edges, &args.output)?,
        ExportFormat::Jsonld => emit_jsonld(&nodes, &edges, &args.output, &project_label)?,
    }

    info!(path = %args.output.display(), "export: wrote output");
    Ok(())
}

// ---------------------------------------------------------------------------
// Graph load helpers
// ---------------------------------------------------------------------------

fn resolve_project_root(project: Option<PathBuf>) -> PathBuf {
    project
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn live_graph_db(project_root: &Path) -> CliResult<PathBuf> {
    let id = ProjectId::from_path(project_root).map_err(|e| {
        CliError::Other(format!(
            "cannot hash project path {}: {e}",
            project_root.display()
        ))
    })?;
    let paths = PathManager::default_root();
    Ok(paths.project_root(&id).join("graph.db"))
}

fn open_ro(db: &Path) -> CliResult<Connection> {
    Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {} read-only: {e}", db.display())))
}

fn load_nodes(conn: &Connection) -> CliResult<Vec<NodeRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end
             FROM nodes
             ORDER BY id ASC",
        )
        .map_err(|e| CliError::Other(format!("prepare nodes: {e}")))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(NodeRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
            })
        })
        .map_err(|e| CliError::Other(format!("query nodes: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| CliError::Other(format!("row map nodes: {e}")))?);
    }
    Ok(out)
}

fn load_edges(conn: &Connection) -> CliResult<Vec<EdgeRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT kind, source_qualified, target_qualified
             FROM edges",
        )
        .map_err(|e| CliError::Other(format!("prepare edges: {e}")))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(EdgeRow {
                kind: row.get(0)?,
                source_qualified: row.get(1)?,
                target_qualified: row.get(2)?,
            })
        })
        .map_err(|e| CliError::Other(format!("query edges: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| CliError::Other(format!("row map edges: {e}")))?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Emitters — each is a pure function (nodes, edges, output) -> CliResult<()>.
// ---------------------------------------------------------------------------

/// Emit GraphML using `quick-xml`'s streaming writer. We declare the
/// node + edge attribute keys exactly once at the top, then stream the
/// `<node>` and `<edge>` rows. Attribute values flow through
/// `BytesText::new`, which handles `&` `<` `>` `"` `'` escaping for us.
pub fn emit_graphml(nodes: &[NodeRow], edges: &[EdgeRow], output: &Path) -> CliResult<()> {
    use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
    use quick_xml::writer::Writer;

    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    {
        let mut w = Writer::new_with_indent(&mut buf, b' ', 2);
        w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(graphml_err)?;

        let mut graphml = BytesStart::new("graphml");
        graphml.push_attribute(("xmlns", "http://graphml.graphdrawing.org/xmlns"));
        graphml.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
        w.write_event(Event::Start(graphml)).map_err(graphml_err)?;

        // Node + edge attribute key declarations. Node ids are namespaced
        // (`d_node_kind` etc.) so they don't collide with edge ids — the
        // GraphML spec lets us reuse `attr.name` between for="node" and
        // for="edge", but distinct `id` tokens make jq / xsltproc filtering
        // less ambiguous.
        for (id, for_target, name, ty) in [
            ("d_node_kind", "node", "kind", "string"),
            ("d_node_name", "node", "name", "string"),
            ("d_node_qname", "node", "qualified_name", "string"),
            ("d_node_path", "node", "path", "string"),
            ("d_node_line", "node", "line", "int"),
            ("d_edge_kind", "edge", "kind", "string"),
        ] {
            let mut k = BytesStart::new("key");
            k.push_attribute(("id", id));
            k.push_attribute(("for", for_target));
            k.push_attribute(("attr.name", name));
            k.push_attribute(("attr.type", ty));
            w.write_event(Event::Empty(k)).map_err(graphml_err)?;
        }

        let mut graph = BytesStart::new("graph");
        graph.push_attribute(("id", "G"));
        graph.push_attribute(("edgedefault", "directed"));
        w.write_event(Event::Start(graph)).map_err(graphml_err)?;

        for n in nodes {
            let id = n.export_id();
            let mut node = BytesStart::new("node");
            node.push_attribute(("id", id.as_str()));
            w.write_event(Event::Start(node)).map_err(graphml_err)?;

            write_data(&mut w, "d_node_kind", &n.kind)?;
            write_data(&mut w, "d_node_name", &n.name)?;
            write_data(&mut w, "d_node_qname", &n.qualified_name)?;
            if let Some(p) = n.file_path.as_deref() {
                write_data(&mut w, "d_node_path", p)?;
            }
            if let Some(l) = n.line_start {
                write_data(&mut w, "d_node_line", &l.to_string())?;
            }
            w.write_event(Event::End(BytesEnd::new("node")))
                .map_err(graphml_err)?;
        }

        // Build the qualified_name -> export_id index ONCE. Edges that
        // reference filtered-out endpoints have already been dropped in
        // `run`, but a defensive `if let Some` keeps the emitter pure.
        let qname_to_id: HashMap<&str, String> = nodes
            .iter()
            .map(|n| (n.qualified_name.as_str(), n.export_id()))
            .collect();

        for (i, e) in edges.iter().enumerate() {
            let (src, dst) = match (
                qname_to_id.get(e.source_qualified.as_str()),
                qname_to_id.get(e.target_qualified.as_str()),
            ) {
                (Some(s), Some(d)) => (s.clone(), d.clone()),
                _ => continue,
            };
            let mut edge = BytesStart::new("edge");
            let edge_id = format!("e_{i}");
            edge.push_attribute(("id", edge_id.as_str()));
            edge.push_attribute(("source", src.as_str()));
            edge.push_attribute(("target", dst.as_str()));
            w.write_event(Event::Start(edge)).map_err(graphml_err)?;
            write_data(&mut w, "d_edge_kind", &e.kind)?;
            w.write_event(Event::End(BytesEnd::new("edge")))
                .map_err(graphml_err)?;
        }

        w.write_event(Event::End(BytesEnd::new("graph")))
            .map_err(graphml_err)?;
        w.write_event(Event::End(BytesEnd::new("graphml")))
            .map_err(graphml_err)?;
    }

    fs::write(output, &buf).map_err(|e| CliError::io(output, e))?;
    Ok(())
}

fn write_data<W: Write>(
    w: &mut quick_xml::writer::Writer<W>,
    key_id: &str,
    value: &str,
) -> CliResult<()> {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
    let mut data = BytesStart::new("data");
    data.push_attribute(("key", key_id));
    w.write_event(Event::Start(data)).map_err(graphml_err)?;
    w.write_event(Event::Text(BytesText::new(value)))
        .map_err(graphml_err)?;
    w.write_event(Event::End(BytesEnd::new("data")))
        .map_err(graphml_err)?;
    Ok(())
}

fn graphml_err(e: quick_xml::Error) -> CliError {
    CliError::Other(format!("graphml emit: {e}"))
}

/// Emit an Obsidian-vault-shaped collection of markdown files. If the
/// output path ends in `.zip`, bundle into a zip; otherwise create the
/// directory and drop one `.md` per node.
pub fn emit_obsidian(nodes: &[NodeRow], edges: &[EdgeRow], output: &Path) -> CliResult<()> {
    // Pre-compute per-node "calls" / "called by" for cheap O(N+E) lookup.
    let mut calls: HashMap<&str, Vec<&EdgeRow>> = HashMap::new();
    let mut called_by: HashMap<&str, Vec<&EdgeRow>> = HashMap::new();
    for e in edges {
        calls
            .entry(e.source_qualified.as_str())
            .or_default()
            .push(e);
        called_by
            .entry(e.target_qualified.as_str())
            .or_default()
            .push(e);
    }

    // Stable-output, dedup filename collisions across nodes that share a
    // sanitised qualified_name (e.g. two `mod tests` blocks).
    let mut used_names: HashSet<String> = HashSet::new();
    let mut entries: Vec<(String, String)> = Vec::with_capacity(nodes.len()); // (filename, body)
    for n in nodes {
        let mut fname = n.obsidian_filename();
        if used_names.contains(&fname) {
            // Disambiguate by appending the SQLite id.
            let stem = fname.trim_end_matches(".md");
            fname = format!("{stem}__{}.md", n.id);
        }
        used_names.insert(fname.clone());

        let body = build_obsidian_body(n, &calls, &called_by);
        entries.push((fname, body));
    }

    let is_zip = output
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"));

    if is_zip {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| CliError::io(parent, e))?;
            }
        }
        let buf = build_obsidian_zip(&entries)?;
        fs::write(output, buf).map_err(|e| CliError::io(output, e))?;
    } else {
        fs::create_dir_all(output).map_err(|e| CliError::io(output, e))?;
        for (fname, body) in &entries {
            let path = output.join(fname);
            fs::write(&path, body).map_err(|e| CliError::io(&path, e))?;
        }
    }
    Ok(())
}

fn build_obsidian_body(
    n: &NodeRow,
    calls: &HashMap<&str, Vec<&EdgeRow>>,
    called_by: &HashMap<&str, Vec<&EdgeRow>>,
) -> String {
    let mut out = String::with_capacity(256);
    out.push_str("# ");
    out.push_str(&n.name);
    out.push_str("\n\n");
    out.push_str(&format!("- **Kind:** {}\n", n.kind));
    out.push_str(&format!("- **Qualified name:** `{}`\n", n.qualified_name));
    if let Some(p) = &n.file_path {
        match n.line_start {
            Some(l) if l > 0 => out.push_str(&format!("- **Path:** [[{}:{}]]\n", p, l)),
            _ => out.push_str(&format!("- **Path:** [[{}]]\n", p)),
        }
    }
    if let Some(l) = n.line_end {
        if l > 0 {
            out.push_str(&format!("- **Line end:** {l}\n"));
        }
    }
    out.push('\n');

    if let Some(out_edges) = calls.get(n.qualified_name.as_str()) {
        if !out_edges.is_empty() {
            // Group by edge kind so the section is readable on big graphs.
            let mut by_kind: BTreeSet<(&str, &str)> = BTreeSet::new();
            for e in out_edges {
                by_kind.insert((e.kind.as_str(), e.target_qualified.as_str()));
            }
            out.push_str("## Outgoing\n\n");
            for (kind, target) in by_kind {
                out.push_str(&format!("- *{kind}* [[{target}]]\n"));
            }
            out.push('\n');
        }
    }
    if let Some(in_edges) = called_by.get(n.qualified_name.as_str()) {
        if !in_edges.is_empty() {
            let mut by_kind: BTreeSet<(&str, &str)> = BTreeSet::new();
            for e in in_edges {
                by_kind.insert((e.kind.as_str(), e.source_qualified.as_str()));
            }
            out.push_str("## Incoming\n\n");
            for (kind, source) in by_kind {
                out.push_str(&format!("- *{kind}* [[{source}]]\n"));
            }
            out.push('\n');
        }
    }
    out.push_str("---\n");
    out.push_str("*Generated by `mneme export --format=obsidian`.*\n");
    out
}

fn build_obsidian_zip(entries: &[(String, String)]) -> CliResult<Vec<u8>> {
    use zip::write::FileOptions;
    use zip::CompressionMethod;
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for (fname, body) in entries {
            w.start_file(fname, opts)
                .map_err(|e| CliError::Other(format!("zip start_file {fname}: {e}")))?;
            w.write_all(body.as_bytes())
                .map_err(|e| CliError::Other(format!("zip write {fname}: {e}")))?;
        }
        w.finish()
            .map_err(|e| CliError::Other(format!("zip finish: {e}")))?;
    }
    Ok(buf.into_inner())
}

/// Emit a Neo4j Cypher import script. `CREATE` per node + per edge,
/// batched into transactions of [`CYPHER_BATCH_SIZE`] statements so the
/// neo4j browser can stream them without a 30s timeout on a big graph.
const CYPHER_BATCH_SIZE: usize = 1000;
pub fn emit_cypher(nodes: &[NodeRow], edges: &[EdgeRow], output: &Path) -> CliResult<()> {
    let mut out = String::with_capacity(nodes.len() * 128 + edges.len() * 96);
    out.push_str("// mneme code-graph export — Cypher\n");
    out.push_str("// Generated by `mneme export --format=cypher`.\n");
    out.push_str("// Apply with: cypher-shell < graph.cypher\n");
    out.push('\n');

    let mut batch = 0usize;
    out.push_str(":begin\n");
    for n in nodes {
        out.push_str(&format!(
            "CREATE (:Node:{label} {{id: '{id}', kind: '{kind}', name: '{name}', qualified_name: '{qn}'{path}{line}}});\n",
            label = cypher_label(&n.kind),
            id = cypher_str(&n.export_id()),
            kind = cypher_str(&n.kind),
            name = cypher_str(&n.name),
            qn = cypher_str(&n.qualified_name),
            path = match &n.file_path {
                Some(p) => format!(", path: '{}'", cypher_str(p)),
                None => String::new(),
            },
            line = match n.line_start {
                Some(l) if l > 0 => format!(", line: {l}"),
                _ => String::new(),
            },
        ));
        batch += 1;
        if batch >= CYPHER_BATCH_SIZE {
            out.push_str(":commit\n:begin\n");
            batch = 0;
        }
    }

    let qname_to_id: HashMap<&str, String> = nodes
        .iter()
        .map(|n| (n.qualified_name.as_str(), n.export_id()))
        .collect();
    for e in edges {
        let (src, dst) = match (
            qname_to_id.get(e.source_qualified.as_str()),
            qname_to_id.get(e.target_qualified.as_str()),
        ) {
            (Some(s), Some(d)) => (s.clone(), d.clone()),
            _ => continue,
        };
        out.push_str(&format!(
            "MATCH (a:Node {{id: '{a}'}}), (b:Node {{id: '{b}'}}) CREATE (a)-[:{kind}]->(b);\n",
            a = cypher_str(&src),
            b = cypher_str(&dst),
            kind = cypher_rel(&e.kind),
        ));
        batch += 1;
        if batch >= CYPHER_BATCH_SIZE {
            out.push_str(":commit\n:begin\n");
            batch = 0;
        }
    }
    out.push_str(":commit\n");

    fs::write(output, out).map_err(|e| CliError::io(output, e))?;
    Ok(())
}

/// Single-quote escape for Cypher string literals. Cypher accepts `\'`
/// inside single-quoted strings; we also escape backslashes so paths on
/// Windows survive the round trip without being mis-interpreted.
fn cypher_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Cypher labels must be `[A-Za-z][A-Za-z0-9_]*`. Map `kind` strings
/// like `function` directly when they fit; sanitise others.
fn cypher_label(kind: &str) -> String {
    let mut out = String::with_capacity(kind.len() + 1);
    let mut chars = kind.chars();
    if let Some(c) = chars.next() {
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('K');
            out.push(c);
        }
    } else {
        return "Unknown".into();
    }
    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

/// Cypher relationship types: same rules as labels but conventionally
/// SCREAMING_SNAKE_CASE.
fn cypher_rel(kind: &str) -> String {
    let label = cypher_label(kind);
    label.to_uppercase()
}

/// Emit a static SVG with a radial-by-kind layout. No external CSS, no
/// JS — drop the file in a browser or paste it into a markdown doc.
/// Cap at the caller's `--max-nodes` (default unbounded). Nodes are
/// grouped concentrically: each unique `kind` gets its own ring.
pub fn emit_svg(nodes: &[NodeRow], edges: &[EdgeRow], output: &Path) -> CliResult<()> {
    // Layout parameters. Tuned for legibility with ~100 nodes; bigger
    // graphs still render but text labels start overlapping past ~300.
    const VIEWBOX: f64 = 1200.0;
    const CENTER: f64 = VIEWBOX / 2.0;
    const NODE_RADIUS: f64 = 6.0;
    const RING_GAP: f64 = 80.0;
    const FIRST_RING: f64 = 120.0;
    const LABEL_OFFSET: f64 = 10.0;

    // Colour palette — uses the brand gradient anchors plus tertiary
    // hues. New kinds fall back to a neutral grey so a fresh language
    // import doesn't crash the renderer.
    let palette: HashMap<&str, &str> = [
        ("function", "#41E1B5"),
        ("method", "#41E1B5"),
        ("struct", "#4191E1"),
        ("class", "#4191E1"),
        ("type", "#22D3EE"),
        ("trait", "#22D3EE"),
        ("interface", "#22D3EE"),
        ("module", "#A78BFA"),
        ("namespace", "#A78BFA"),
        ("variable", "#FBBF24"),
        ("constant", "#FBBF24"),
        ("enum", "#F472B6"),
    ]
    .into_iter()
    .collect();
    let default_colour = "#94A3B8";

    // Group by kind, stable order.
    let mut by_kind: HashMap<&str, Vec<&NodeRow>> = HashMap::new();
    let mut kind_order: Vec<&str> = Vec::new();
    for n in nodes {
        let k = n.kind.as_str();
        if !by_kind.contains_key(k) {
            kind_order.push(k);
        }
        by_kind.entry(k).or_default().push(n);
    }

    // Position table: qualified_name -> (x, y, colour, label).
    let mut positions: HashMap<&str, (f64, f64, &str, &str)> = HashMap::new();
    for (ring_i, k) in kind_order.iter().enumerate() {
        let radius = FIRST_RING + (ring_i as f64) * RING_GAP;
        let group = &by_kind[k];
        let count = group.len() as f64;
        for (i, n) in group.iter().enumerate() {
            let theta = (i as f64) / count.max(1.0) * std::f64::consts::TAU;
            let x = CENTER + radius * theta.cos();
            let y = CENTER + radius * theta.sin();
            let colour = palette.get(k).copied().unwrap_or(default_colour);
            positions.insert(n.qualified_name.as_str(), (x, y, colour, n.name.as_str()));
        }
    }

    let mut svg = String::with_capacity(nodes.len() * 192 + edges.len() * 96);
    svg.push_str(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {VIEWBOX} {VIEWBOX}\" \
         width=\"{VIEWBOX}\" height=\"{VIEWBOX}\" font-family=\"system-ui, -apple-system, sans-serif\" font-size=\"10\">\n",
    ));
    svg.push_str("  <rect width=\"100%\" height=\"100%\" fill=\"#0B1220\"/>\n");
    svg.push_str(&format!(
        "  <text x=\"{x}\" y=\"24\" fill=\"#E5E7EB\" font-size=\"16\" text-anchor=\"middle\">mneme code-graph ({n} nodes, {e} edges)</text>\n",
        x = CENTER, n = nodes.len(), e = edges.len()
    ));

    // Edges first so node circles sit on top.
    svg.push_str("  <g stroke=\"#334155\" stroke-opacity=\"0.45\" stroke-width=\"0.6\">\n");
    for e in edges {
        if let (Some(s), Some(t)) = (
            positions.get(e.source_qualified.as_str()),
            positions.get(e.target_qualified.as_str()),
        ) {
            svg.push_str(&format!(
                "    <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\"/>\n",
                s.0, s.1, t.0, t.1
            ));
        }
    }
    svg.push_str("  </g>\n");

    // Nodes.
    svg.push_str("  <g>\n");
    for n in nodes {
        let (x, y, colour, label) = positions[n.qualified_name.as_str()];
        svg.push_str(&format!(
            "    <circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{NODE_RADIUS}\" fill=\"{colour}\"><title>{}</title></circle>\n",
            xml_escape(&format!("{} ({})", n.qualified_name, n.kind)),
        ));
        // Label only for nodes outside the densest centre — keeps the
        // SVG legible past ~50 nodes.
        let dx = x - CENTER;
        let dy = y - CENTER;
        if (dx * dx + dy * dy).sqrt() > FIRST_RING - 1.0 {
            svg.push_str(&format!(
                "    <text x=\"{:.2}\" y=\"{:.2}\" fill=\"#E5E7EB\" text-anchor=\"middle\">{}</text>\n",
                x,
                y - NODE_RADIUS - LABEL_OFFSET,
                xml_escape(label)
            ));
        }
    }
    svg.push_str("  </g>\n");

    // Legend.
    let mut legend_y = VIEWBOX - 24.0 * (kind_order.len() as f64) - 24.0;
    svg.push_str("  <g font-size=\"11\">\n");
    for k in &kind_order {
        let colour = palette.get(*k).copied().unwrap_or(default_colour);
        svg.push_str(&format!(
            "    <circle cx=\"24\" cy=\"{legend_y:.2}\" r=\"5\" fill=\"{colour}\"/>\n\
             <text x=\"36\" y=\"{ty:.2}\" fill=\"#E5E7EB\">{k}</text>\n",
            ty = legend_y + 4.0,
            k = xml_escape(k),
        ));
        legend_y += 24.0;
    }
    svg.push_str("  </g>\n");
    svg.push_str("</svg>\n");

    fs::write(output, svg).map_err(|e| CliError::io(output, e))?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Emit JSON-LD using schema.org's `Dataset` + `DefinedTerm` vocabulary.
/// The result is a single document — semantic-web consumers like
/// json-ld.org / Google's structured-data tools accept it as-is.
pub fn emit_jsonld(
    nodes: &[NodeRow],
    edges: &[EdgeRow],
    output: &Path,
    project_label: &str,
) -> CliResult<()> {
    use serde_json::{json, Map, Value};

    let mut parts: Vec<Value> = Vec::with_capacity(nodes.len());
    for n in nodes {
        let mut obj = Map::new();
        obj.insert("@type".into(), Value::String("DefinedTerm".into()));
        obj.insert(
            "@id".into(),
            Value::String(format!("node:{}", n.export_id())),
        );
        obj.insert("name".into(), Value::String(n.name.clone()));
        obj.insert(
            "alternateName".into(),
            Value::String(n.qualified_name.clone()),
        );
        obj.insert("termCode".into(), Value::String(n.kind.clone()));
        if let Some(p) = &n.file_path {
            let url = match n.line_start {
                Some(l) if l > 0 => format!("file:///{}#L{l}", p.replace('\\', "/")),
                _ => format!("file:///{}", p.replace('\\', "/")),
            };
            obj.insert("url".into(), Value::String(url));
        }
        parts.push(Value::Object(obj));
    }

    let mut relations: Vec<Value> = Vec::with_capacity(edges.len());
    let qname_set: HashSet<&str> = nodes.iter().map(|n| n.qualified_name.as_str()).collect();
    let qname_to_id: HashMap<&str, String> = nodes
        .iter()
        .map(|n| (n.qualified_name.as_str(), n.export_id()))
        .collect();
    for e in edges {
        if !qname_set.contains(e.source_qualified.as_str())
            || !qname_set.contains(e.target_qualified.as_str())
        {
            continue;
        }
        let from = &qname_to_id[e.source_qualified.as_str()];
        let to = &qname_to_id[e.target_qualified.as_str()];
        relations.push(json!({
            "@type": "Action",
            "actionStatus": e.kind.clone(),
            "agent": { "@id": format!("node:{from}") },
            "object": { "@id": format!("node:{to}") },
        }));
    }

    let doc = json!({
        "@context": "https://schema.org",
        "@type": "Dataset",
        "name": format!("mneme code-graph for {project_label}"),
        "description": "Auto-generated by `mneme export --format=jsonld`.",
        "creator": {
            "@type": "Person",
            "name": "Anish Trivedi & Kruti Trivedi",
        },
        "license": "https://www.apache.org/licenses/LICENSE-2.0",
        "isAccessibleForFree": true,
        "hasPart": parts,
        "isBasedOn": relations,
        "size": format!("{} nodes, {} edges", nodes.len(), edges.len()),
    });
    let s = serde_json::to_string_pretty(&doc).map_err(CliError::Json)?;
    fs::write(output, s).map_err(|e| CliError::io(output, e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Glob matcher — copy of the one in graph_diff.rs.
// Kept local so the two commands evolve independently; the matcher is
// ~40 lines and pulling it into a shared module would couple the
// command-level cocktail surfaces unnecessarily.
// ---------------------------------------------------------------------------

struct GlobMatcher {
    re: regex::Regex,
}
impl GlobMatcher {
    fn compile(pattern: &str) -> Self {
        let mut out = String::with_capacity(pattern.len() * 2 + 4);
        out.push('^');
        let bytes = pattern.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            match c {
                '*' => {
                    let next = bytes.get(i + 1).copied().map(|b| b as char);
                    let next2 = bytes.get(i + 2).copied().map(|b| b as char);
                    if next == Some('*') && next2 == Some('/') {
                        out.push_str("(?:.*/)?");
                        i += 3;
                        continue;
                    }
                    if next == Some('*') {
                        out.push_str(".*");
                        i += 2;
                        continue;
                    }
                    out.push_str("[^/]*");
                }
                '?' => out.push_str("[^/]"),
                '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                    out.push('\\');
                    out.push(c);
                }
                _ => out.push(c),
            }
            i += 1;
        }
        out.push('$');
        let re = regex::Regex::new(&out).unwrap_or_else(|_| regex::Regex::new("^$").unwrap());
        Self { re }
    }
    fn matches(&self, s: &str) -> bool {
        let normalised = s.replace('\\', "/");
        self.re.is_match(&normalised)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rusqlite::params;

    /// Drives clap parsing without re-exporting the binary's enum.
    #[derive(Parser, Debug)]
    #[command(name = "mneme")]
    struct TestCli {
        #[command(subcommand)]
        cmd: TestCmd,
    }
    #[derive(clap::Subcommand, Debug)]
    enum TestCmd {
        Export(ExportArgs),
    }

    #[test]
    fn parses_clap_args() {
        let cli =
            TestCli::try_parse_from(["mneme", "export", "--format=graphml", "-o", "g.graphml"])
                .expect("parse");
        let TestCmd::Export(args) = cli.cmd;
        assert_eq!(args.format, ExportFormat::Graphml);
        assert_eq!(args.output, PathBuf::from("g.graphml"));
        assert!(args.include_edges, "include_edges defaults to true");
    }

    #[test]
    fn parses_filters_and_obsidian_format() {
        let cli = TestCli::try_parse_from([
            "mneme",
            "export",
            "--format=obsidian",
            "-o",
            "vault/",
            "--kinds",
            "function",
            "--kinds",
            "struct",
            "--files",
            "src/**/*.rs",
            "--max-nodes",
            "100",
            "--include-edges=false",
        ])
        .expect("parse");
        let TestCmd::Export(args) = cli.cmd;
        assert_eq!(args.format, ExportFormat::Obsidian);
        assert_eq!(args.kinds, vec!["function", "struct"]);
        assert_eq!(args.files.as_deref(), Some("src/**/*.rs"));
        assert_eq!(args.max_nodes, Some(100));
        assert!(!args.include_edges);
    }

    fn sample_nodes() -> Vec<NodeRow> {
        vec![
            NodeRow {
                id: 1,
                kind: "function".into(),
                name: "alpha".into(),
                qualified_name: "crate::alpha".into(),
                file_path: Some("src/lib.rs".into()),
                line_start: Some(1),
                line_end: Some(5),
            },
            NodeRow {
                id: 2,
                kind: "struct".into(),
                name: "Beta".into(),
                qualified_name: "crate::Beta".into(),
                file_path: Some("src/lib.rs".into()),
                line_start: Some(8),
                line_end: Some(20),
            },
            NodeRow {
                id: 3,
                kind: "function".into(),
                name: "gamma".into(),
                qualified_name: "crate::Beta::gamma".into(),
                file_path: Some("src/beta.rs".into()),
                line_start: Some(2),
                line_end: Some(12),
            },
        ]
    }
    fn sample_edges() -> Vec<EdgeRow> {
        vec![
            EdgeRow {
                kind: "calls".into(),
                source_qualified: "crate::alpha".into(),
                target_qualified: "crate::Beta::gamma".into(),
            },
            EdgeRow {
                kind: "contains".into(),
                source_qualified: "crate::Beta".into(),
                target_qualified: "crate::Beta::gamma".into(),
            },
        ]
    }

    #[test]
    fn graphml_output_is_valid_xml() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.graphml");
        emit_graphml(&sample_nodes(), &sample_edges(), &out).expect("emit");
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(
            text.starts_with("<?xml"),
            "should start with XML decl: {text}"
        );
        assert!(text.contains("<graphml"));
        assert!(text.contains("<node id=\"n_1\""));
        assert!(text.contains("<edge"));

        // Round-trip via quick-xml's reader so we know the doc is balanced.
        use quick_xml::events::Event;
        use quick_xml::reader::Reader;
        let mut reader = Reader::from_str(&text);
        let mut depth = 0i32;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).expect("xml parses") {
                Event::Start(_) => depth += 1,
                Event::End(_) => depth -= 1,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        assert_eq!(depth, 0, "balanced tags");
    }

    #[test]
    fn cypher_emits_create_per_node() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.cypher");
        emit_cypher(&sample_nodes(), &sample_edges(), &out).expect("emit");
        let text = std::fs::read_to_string(&out).unwrap();
        let creates = text.matches("CREATE (:Node:").count();
        assert_eq!(creates, sample_nodes().len(), "one node CREATE per row");
        let matches = text.matches("MATCH (a:Node").count();
        assert_eq!(matches, sample_edges().len(), "one MATCH per edge");
        assert!(text.contains(":begin"));
        assert!(text.contains(":commit"));
    }

    #[test]
    fn obsidian_emits_one_md_per_node_in_dir() {
        let td = tempfile::tempdir().unwrap();
        let dir = td.path().join("vault");
        emit_obsidian(&sample_nodes(), &sample_edges(), &dir).expect("emit");
        let files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .collect();
        assert_eq!(files.len(), sample_nodes().len());
        // The function `alpha` should reference `crate::Beta::gamma` via wiki link.
        let alpha_path = files
            .iter()
            .find(|e| e.file_name().to_string_lossy().contains("alpha"))
            .expect("alpha.md exists");
        let alpha_text = std::fs::read_to_string(alpha_path.path()).unwrap();
        assert!(alpha_text.contains("[[crate::Beta::gamma]]"));
        assert!(alpha_text.contains("# alpha"));
    }

    #[test]
    fn obsidian_emits_zip_when_extension_is_zip() {
        let td = tempfile::tempdir().unwrap();
        let zip_path = td.path().join("vault.zip");
        emit_obsidian(&sample_nodes(), &sample_edges(), &zip_path).expect("emit");
        let bytes = std::fs::read(&zip_path).unwrap();
        // Zip local file header magic.
        assert_eq!(&bytes[..4], b"PK\x03\x04", "looks like a zip");
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).expect("read zip");
        assert_eq!(z.len(), sample_nodes().len(), "one entry per node");
        // Spot-check that one entry parses as the expected markdown.
        let mut found_alpha = false;
        for i in 0..z.len() {
            let mut entry = z.by_index(i).unwrap();
            if entry.name().contains("alpha") {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut entry, &mut s).unwrap();
                assert!(s.contains("# alpha"));
                found_alpha = true;
            }
        }
        assert!(found_alpha, "alpha entry exists in zip");
    }

    #[test]
    fn svg_emits_circles_and_lines() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.svg");
        emit_svg(&sample_nodes(), &sample_edges(), &out).expect("emit");
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.starts_with("<?xml"));
        assert!(text.contains("<svg"));
        let circles = text.matches("<circle").count();
        assert!(
            circles >= sample_nodes().len(),
            "at least one circle per node"
        );
        let lines = text.matches("<line").count();
        assert_eq!(lines, sample_edges().len());
    }

    #[test]
    fn jsonld_round_trips_via_serde() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.jsonld");
        emit_jsonld(&sample_nodes(), &sample_edges(), &out, "demo").expect("emit");
        let text = std::fs::read_to_string(&out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(v["@context"], "https://schema.org");
        assert_eq!(v["@type"], "Dataset");
        let parts = v["hasPart"].as_array().expect("hasPart array");
        assert_eq!(parts.len(), sample_nodes().len());
        let relations = v["isBasedOn"].as_array().expect("isBasedOn array");
        assert_eq!(relations.len(), sample_edges().len());
    }

    /// `--kinds=struct` should drop the function nodes during the
    /// in-memory filter pass. We exercise the predicate directly so the
    /// test doesn't need a `graph.db`.
    #[test]
    fn kinds_filter_drops_non_matching() {
        let mut nodes = sample_nodes();
        let allowed: HashSet<String> = ["struct".to_string()].into_iter().collect();
        nodes.retain(|n| allowed.contains(&n.kind));
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "struct");
    }

    #[test]
    fn max_nodes_caps_output() {
        let mut nodes = sample_nodes();
        let cap = 2;
        if nodes.len() > cap {
            nodes.truncate(cap);
        }
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn files_glob_filter_works() {
        let nodes = sample_nodes();
        let matcher = GlobMatcher::compile("src/beta.rs");
        let kept: Vec<_> = nodes
            .iter()
            .filter(|n| n.file_path.as_deref().is_some_and(|p| matcher.matches(p)))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "gamma");
    }

    #[test]
    fn glob_double_star_matches_subdirs() {
        let m = GlobMatcher::compile("src/**/*.rs");
        assert!(m.matches("src/lib.rs"));
        assert!(m.matches("src/cli/main.rs"));
        assert!(!m.matches("tests/lib.rs"));
    }

    #[test]
    fn cypher_label_sanitises_kind() {
        assert_eq!(cypher_label("function"), "Function");
        assert_eq!(cypher_label("type-alias"), "Type_alias");
        assert_eq!(cypher_label(""), "Unknown");
        assert_eq!(cypher_label("9foo"), "K9foo");
    }

    #[test]
    fn cypher_str_escapes_quotes_and_backslashes() {
        assert_eq!(cypher_str("a'b"), "a\\'b");
        assert_eq!(cypher_str("c:\\path"), "c:\\\\path");
        assert_eq!(cypher_str("line\nbreak"), "line\\nbreak");
    }

    #[test]
    fn obsidian_filename_is_filesystem_safe() {
        let n = NodeRow {
            id: 7,
            kind: "function".into(),
            name: "f".into(),
            qualified_name: "crate::module::f<T>".into(),
            file_path: None,
            line_start: None,
            line_end: None,
        };
        let fname = n.obsidian_filename();
        for c in fname.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.',
                "non-safe char {c:?} in {fname}",
            );
        }
        assert!(fname.ends_with(".md"));
    }

    /// End-to-end: build a synthetic graph.db, point the loader at it,
    /// and verify the GraphML emitter produces something parseable.
    #[test]
    fn end_to_end_graphml_from_synthetic_db() {
        let td = tempfile::tempdir().unwrap();
        let db = td.path().join("graph.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                qualified_name TEXT UNIQUE NOT NULL,
                file_path TEXT,
                line_start INTEGER,
                line_end INTEGER
            );
            CREATE TABLE edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                source_qualified TEXT NOT NULL,
                target_qualified TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        for (kind, name, qn, file, ls, le) in [
            ("function", "alpha", "crate::alpha", "src/lib.rs", 1, 5),
            ("function", "beta", "crate::beta", "src/lib.rs", 7, 10),
        ] {
            conn.execute(
                "INSERT INTO nodes (kind, name, qualified_name, file_path, line_start, line_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![kind, name, qn, file, ls, le],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO edges (kind, source_qualified, target_qualified) VALUES (?1, ?2, ?3)",
            params!["calls", "crate::alpha", "crate::beta"],
        )
        .unwrap();

        let conn_ro = open_ro(&db).unwrap();
        let nodes = load_nodes(&conn_ro).unwrap();
        let edges = load_edges(&conn_ro).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(edges.len(), 1);

        let out = td.path().join("g.graphml");
        emit_graphml(&nodes, &edges, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("crate::alpha"));
        assert!(text.contains("crate::beta"));
    }
}
