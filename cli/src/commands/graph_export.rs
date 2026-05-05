//! `mneme graph-export --format <fmt> [--out <file>] [--project <hash>]`
//!
//! Wave 3.1 (v0.4.0): a project-hash-addressable export front-end that
//! delegates to the same five emitters as `mneme export`. The difference
//! from the existing `export` command is the project-addressing model:
//!
//! - `mneme export --project <path>` resolves a path → ProjectId hash.
//! - `mneme graph-export --project <hash>` accepts the hash **directly**,
//!   letting callers that already know the shard address skip the filesystem
//!   stat that `canonicalize` requires (useful in CI pipelines where the
//!   source tree is not present but the ~/.mneme shard is mounted).
//!
//! When `--project` is omitted the command behaves identically to
//! `mneme export`: it derives the ProjectId from the current working
//! directory. Both resolution paths honour `MNEME_HOME`.
//!
//! Format dispatch is identical to `export.rs` — the five emitter fns
//! (`emit_graphml`, `emit_obsidian`, `emit_cypher`, `emit_svg`,
//! `emit_jsonld`) are re-exported from that module as `pub fn` and called
//! directly here so there is zero duplication of the format logic.
//!
//! # Output model
//!
//! | Format    | `--out` semantics                                         |
//! |-----------|-----------------------------------------------------------|
//! | graphml   | single `.graphml` file                                    |
//! | obsidian  | directory of `.md` files (or `.zip` bundle)               |
//! | cypher    | single `.cypher` script                                   |
//! | svg       | single `.svg` file                                        |
//! | jsonld    | single `.jsonld` file                                     |
//!
//! When `--out` is omitted the default filename is inferred from the
//! format: `graph.graphml`, `graph-vault/` (obsidian), `graph.cypher`,
//! `graph.svg`, `graph.jsonld`.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use clap::{Args, ValueEnum};
use rusqlite::{Connection, OpenFlags};
use tracing::info;

use crate::commands::export::{
    emit_cypher, emit_graphml, emit_jsonld, emit_obsidian, emit_svg, EdgeRow, NodeRow,
};
use crate::error::{CliError, CliResult};
use common::{ids::ProjectId, paths::PathManager};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// CLI args for `mneme graph-export`.
#[derive(Debug, Args)]
pub struct GraphExportArgs {
    /// Output format. Each format targets a different downstream tool:
    ///
    ///   graphml   — XML, imports into Gephi / yEd / Cytoscape.
    ///   obsidian  — Markdown vault with [[wikilinks]].
    ///   cypher    — Neo4j CREATE script.
    ///   svg       — Static rendered graph (deterministic grid layout).
    ///   jsonld    — schema.org Dataset + DefinedTerm knowledge graph.
    #[arg(long, value_enum)]
    pub format: GraphExportFormat,

    /// Destination file or directory. Defaults depend on `--format`:
    ///   graphml  → `graph.graphml`
    ///   obsidian → `graph-vault/`
    ///   cypher   → `graph.cypher`
    ///   svg      → `graph.svg`
    ///   jsonld   → `graph.jsonld`
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    /// Project shard to export. Accepts two forms:
    ///
    ///   1. A 16-character hex ProjectId hash (e.g. `8b0674d4027fc3b8`).
    ///      The shard is read directly from
    ///      `~/.mneme/projects/<hash>/graph.db`.
    ///   2. A filesystem path. The path is hashed on the fly to derive
    ///      the ProjectId (same as `mneme export --project <path>`).
    ///
    /// When omitted the current working directory is used as the path.
    #[arg(long)]
    pub project: Option<String>,

    /// Restrict to specific node kinds (`function`, `struct`, `module`, …).
    /// Repeatable: `--kinds function --kinds struct`.
    #[arg(long)]
    pub kinds: Vec<String>,

    /// Glob filter on `file_path` (e.g. `src/**/*.rs`).
    #[arg(long)]
    pub files: Option<String>,

    /// Cap total node count. Useful for SVG legibility.
    #[arg(long)]
    pub max_nodes: Option<usize>,

    /// Include edges in the export (default: true).
    /// Pass `--include-edges=false` for a node-only dump.
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
    )]
    pub include_edges: bool,
}

/// Output formats for `mneme graph-export`. Mirrors [`super::export::ExportFormat`]
/// so the two commands are independently evolvable — new formats land in
/// both enums, or only in the one that makes sense.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum GraphExportFormat {
    /// `.graphml` — XML graph format. Opens in Gephi, yEd, Cytoscape, igraph.
    Graphml,
    /// Obsidian markdown vault (`[[wikilinks]]`). Drop the directory into a
    /// vault or pass a `.zip` path to bundle.
    Obsidian,
    /// Neo4j Cypher import script (`CREATE` + `MATCH` statements, batched).
    Cypher,
    /// Static SVG with a deterministic radial-by-kind layout. No external CSS.
    Svg,
    /// JSON-LD with schema.org `Dataset` + `DefinedTerm` vocabulary.
    Jsonld,
}

impl GraphExportFormat {
    /// Default output filename or directory name when `--out` is omitted.
    fn default_output(&self) -> &'static str {
        match self {
            GraphExportFormat::Graphml => "graph.graphml",
            GraphExportFormat::Obsidian => "graph-vault",
            GraphExportFormat::Cypher => "graph.cypher",
            GraphExportFormat::Svg => "graph.svg",
            GraphExportFormat::Jsonld => "graph.jsonld",
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Entry point used by `main.rs`.
pub async fn run(args: GraphExportArgs) -> CliResult<()> {
    let db_path = resolve_db_path(args.project.as_deref())?;

    if !db_path.exists() {
        return Err(CliError::Other(format!(
            "graph.db not found at {}. Run `mneme build .` first.",
            db_path.display()
        )));
    }

    info!(path = %db_path.display(), "graph-export: reading graph.db");

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
        .map_err(|e| CliError::Other(format!("graph-export worker join failed: {e}")))?;

    let (nodes, edges) = join_result?;

    info!(
        nodes = nodes.len(),
        edges = edges.len(),
        "graph-export: filters applied"
    );

    let output = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(args.format.default_output()));

    // Ensure parent directory exists for single-file emitters.
    if !matches!(args.format, GraphExportFormat::Obsidian) {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| CliError::io(parent, e))?;
            }
        }
    }

    // Derive a human-readable project label for the JSON-LD emitter.
    let project_label = db_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    match args.format {
        GraphExportFormat::Graphml => emit_graphml(&nodes, &edges, &output)?,
        GraphExportFormat::Obsidian => emit_obsidian(&nodes, &edges, &output)?,
        GraphExportFormat::Cypher => emit_cypher(&nodes, &edges, &output)?,
        GraphExportFormat::Svg => emit_svg(&nodes, &edges, &output)?,
        GraphExportFormat::Jsonld => emit_jsonld(&nodes, &edges, &output, &project_label)?,
    }

    println!(
        "graph-export: wrote {} ({} nodes, {} edges) → {}",
        args.format.default_output(),
        nodes.len(),
        edges.len(),
        output.display()
    );
    info!(path = %output.display(), "graph-export: done");
    Ok(())
}

// ---------------------------------------------------------------------------
// Project / DB resolution
// ---------------------------------------------------------------------------

/// Resolve the `graph.db` path from the optional `--project` argument.
///
/// Accepts two forms:
///   1. A 16-character hex string — treated as a raw ProjectId.
///   2. Anything else — treated as a filesystem path and hashed.
///   3. `None` — current working directory is hashed.
fn resolve_db_path(project: Option<&str>) -> CliResult<PathBuf> {
    let paths = PathManager::default_root();

    match project {
        // Pure hex string → treat as a raw ProjectId. Lets callers address
        // shards without needing the source tree present on disk.
        Some(s) if looks_like_hex_id(s) => {
            let id = ProjectId::from_hash(s);
            Ok(paths.project_root(&id).join("graph.db"))
        }
        // Path (relative or absolute) → hash it the same way `mneme build` does.
        Some(path_str) => {
            let p = PathBuf::from(path_str);
            let canonical = fs::canonicalize(&p).unwrap_or(p);
            let id = ProjectId::from_path(&canonical).map_err(|e| {
                CliError::Other(format!(
                    "cannot hash project path {}: {e}",
                    canonical.display()
                ))
            })?;
            Ok(paths.project_root(&id).join("graph.db"))
        }
        // Default: CWD.
        None => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let id = ProjectId::from_path(&cwd).map_err(|e| {
                CliError::Other(format!(
                    "cannot hash cwd {}: {e}",
                    cwd.display()
                ))
            })?;
            Ok(paths.project_root(&id).join("graph.db"))
        }
    }
}

/// Heuristic: a pure hex string of 8–64 chars is treated as a ProjectId
/// rather than a filesystem path. Paths always contain at least one
/// non-hex character (`.`, `/`, `\`, `-`) unless they are legitimately
/// ambiguous — in which case the user should pass an absolute path.
fn looks_like_hex_id(s: &str) -> bool {
    let len = s.len();
    (8..=64).contains(&len) && s.chars().all(|c| c.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// SQLite helpers (local copies — kept independent of export.rs so the two
// commands can evolve separately without coupling their DB schemas).
// ---------------------------------------------------------------------------

fn open_ro(db: &std::path::Path) -> CliResult<Connection> {
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
// Glob matcher — local copy, same algorithm as export.rs / graph_diff.rs.
// Kept local so the command evolves independently.
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

    // -------------------------------------------------------------------------
    // Shared toy graph — 5 nodes, 4 edges, used across every format test.
    // -------------------------------------------------------------------------

    fn toy_nodes() -> Vec<NodeRow> {
        vec![
            NodeRow {
                id: 1,
                kind: "module".into(),
                name: "app".into(),
                qualified_name: "crate::app".into(),
                file_path: Some("src/app.rs".into()),
                line_start: Some(1),
                line_end: Some(100),
            },
            NodeRow {
                id: 2,
                kind: "function".into(),
                name: "init".into(),
                qualified_name: "crate::app::init".into(),
                file_path: Some("src/app.rs".into()),
                line_start: Some(10),
                line_end: Some(25),
            },
            NodeRow {
                id: 3,
                kind: "struct".into(),
                name: "Config".into(),
                qualified_name: "crate::app::Config".into(),
                file_path: Some("src/config.rs".into()),
                line_start: Some(5),
                line_end: Some(30),
            },
            NodeRow {
                id: 4,
                kind: "function".into(),
                name: "load".into(),
                qualified_name: "crate::app::Config::load".into(),
                file_path: Some("src/config.rs".into()),
                line_start: Some(32),
                line_end: Some(55),
            },
            NodeRow {
                id: 5,
                kind: "trait".into(),
                name: "Loader".into(),
                qualified_name: "crate::traits::Loader".into(),
                file_path: Some("src/traits.rs".into()),
                line_start: Some(1),
                line_end: Some(8),
            },
        ]
    }

    fn toy_edges() -> Vec<EdgeRow> {
        vec![
            EdgeRow {
                kind: "contains".into(),
                source_qualified: "crate::app".into(),
                target_qualified: "crate::app::init".into(),
            },
            EdgeRow {
                kind: "contains".into(),
                source_qualified: "crate::app".into(),
                target_qualified: "crate::app::Config".into(),
            },
            EdgeRow {
                kind: "contains".into(),
                source_qualified: "crate::app::Config".into(),
                target_qualified: "crate::app::Config::load".into(),
            },
            EdgeRow {
                kind: "implements".into(),
                source_qualified: "crate::app::Config".into(),
                target_qualified: "crate::traits::Loader".into(),
            },
        ]
    }

    // -------------------------------------------------------------------------
    // Clap parsing
    // -------------------------------------------------------------------------

    #[derive(Parser, Debug)]
    #[command(name = "mneme")]
    struct TestCli {
        #[command(subcommand)]
        cmd: TestCmd,
    }

    #[derive(clap::Subcommand, Debug)]
    enum TestCmd {
        #[command(name = "graph-export")]
        GraphExport(GraphExportArgs),
    }

    #[test]
    fn parses_graph_export_graphml() {
        let cli = TestCli::try_parse_from([
            "mneme",
            "graph-export",
            "--format=graphml",
            "--out",
            "g.graphml",
        ])
        .expect("parse");
        let TestCmd::GraphExport(args) = cli.cmd;
        assert_eq!(args.format, GraphExportFormat::Graphml);
        assert_eq!(args.out.as_deref(), Some(std::path::Path::new("g.graphml")));
        assert!(args.include_edges, "defaults to true");
        assert!(args.project.is_none());
    }

    #[test]
    fn parses_graph_export_with_project_hash() {
        let cli = TestCli::try_parse_from([
            "mneme",
            "graph-export",
            "--format=svg",
            "--project",
            "8b0674d4027fc3b8",
            "--max-nodes",
            "200",
            "--include-edges=false",
        ])
        .expect("parse");
        let TestCmd::GraphExport(args) = cli.cmd;
        assert_eq!(args.format, GraphExportFormat::Svg);
        assert_eq!(args.project.as_deref(), Some("8b0674d4027fc3b8"));
        assert_eq!(args.max_nodes, Some(200));
        assert!(!args.include_edges);
    }

    #[test]
    fn parses_kinds_and_files_filter() {
        let cli = TestCli::try_parse_from([
            "mneme",
            "graph-export",
            "--format=cypher",
            "--kinds",
            "function",
            "--kinds",
            "struct",
            "--files",
            "src/**/*.rs",
        ])
        .expect("parse");
        let TestCmd::GraphExport(args) = cli.cmd;
        assert_eq!(args.kinds, vec!["function", "struct"]);
        assert_eq!(args.files.as_deref(), Some("src/**/*.rs"));
    }

    // -------------------------------------------------------------------------
    // Default output name
    // -------------------------------------------------------------------------

    #[test]
    fn default_output_names_match_format() {
        assert_eq!(GraphExportFormat::Graphml.default_output(), "graph.graphml");
        assert_eq!(GraphExportFormat::Obsidian.default_output(), "graph-vault");
        assert_eq!(GraphExportFormat::Cypher.default_output(), "graph.cypher");
        assert_eq!(GraphExportFormat::Svg.default_output(), "graph.svg");
        assert_eq!(GraphExportFormat::Jsonld.default_output(), "graph.jsonld");
    }

    // -------------------------------------------------------------------------
    // Hex-ID heuristic
    // -------------------------------------------------------------------------

    #[test]
    fn looks_like_hex_id_detects_pure_hex() {
        // 16-char canonical ProjectId
        assert!(looks_like_hex_id("8b0674d4027fc3b8"));
        // 8-char minimum
        assert!(looks_like_hex_id("deadbeef"));
        // 64-char maximum
        assert!(looks_like_hex_id(&"a".repeat(64)));
    }

    #[test]
    fn looks_like_hex_id_rejects_paths() {
        assert!(!looks_like_hex_id("/home/user/project"));
        assert!(!looks_like_hex_id("./my-project"));
        assert!(!looks_like_hex_id("C:\\Users\\project"));
        assert!(!looks_like_hex_id("project-name"));
        // Too short (< 8)
        assert!(!looks_like_hex_id("abc"));
        // Too long (> 64)
        assert!(!looks_like_hex_id(&"a".repeat(65)));
        // Contains non-hex
        assert!(!looks_like_hex_id("8b0674d4027fc3bG"));
    }

    // -------------------------------------------------------------------------
    // Format emitters — each validated against the toy 5-node/4-edge graph.
    // -------------------------------------------------------------------------

    #[test]
    fn graphml_emitter_produces_valid_xml_with_correct_counts() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.graphml");
        emit_graphml(&toy_nodes(), &toy_edges(), &out).expect("emit_graphml");

        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.starts_with("<?xml"), "must start with XML decl");
        assert!(text.contains("<graphml"), "must contain graphml root");

        // Verify the node count via substring matching.
        let node_count = text.matches("<node id=").count();
        assert_eq!(node_count, 5, "5 <node> elements");

        // Edge count.
        let edge_count = text.matches("<edge ").count();
        assert_eq!(edge_count, 4, "4 <edge> elements");

        // All 5 qualified names appear in the output.
        for n in toy_nodes() {
            assert!(
                text.contains(&n.qualified_name),
                "qualified_name {} not found in graphml",
                n.qualified_name
            );
        }
    }

    #[test]
    fn graphml_roundtrip_node_and_edge_counts() {
        // Export → re-parse with quick-xml → count Start events for
        // <node> and <edge> — confirms the document is balanced and
        // the counts survive the round-trip.
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("rt.graphml");
        emit_graphml(&toy_nodes(), &toy_edges(), &out).expect("emit");
        let text = std::fs::read_to_string(&out).unwrap();

        use quick_xml::events::Event;
        use quick_xml::reader::Reader;
        let mut reader = Reader::from_str(&text);
        let mut node_starts = 0usize;
        let mut edge_starts = 0usize;
        let mut depth = 0i32;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).expect("xml parses") {
                Event::Start(e) => {
                    depth += 1;
                    match e.name().as_ref() {
                        b"node" => node_starts += 1,
                        b"edge" => edge_starts += 1,
                        _ => {}
                    }
                }
                Event::End(_) => depth -= 1,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        assert_eq!(depth, 0, "XML tags must be balanced");
        assert_eq!(node_starts, 5, "5 node elements");
        assert_eq!(edge_starts, 4, "4 edge elements");
    }

    #[test]
    fn obsidian_emitter_produces_one_md_per_node() {
        let td = tempfile::tempdir().unwrap();
        let vault = td.path().join("vault");
        emit_obsidian(&toy_nodes(), &toy_edges(), &vault).expect("emit_obsidian");

        let md_files: Vec<_> = std::fs::read_dir(&vault)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .collect();
        assert_eq!(md_files.len(), 5, "one .md per node");
    }

    #[test]
    fn obsidian_emitter_contains_wikilinks() {
        let td = tempfile::tempdir().unwrap();
        let vault = td.path().join("vault");
        emit_obsidian(&toy_nodes(), &toy_edges(), &vault).expect("emit_obsidian");

        // The `app` module has outgoing "contains" edges to `init` and `Config`.
        // Its .md should have [[wikilinks]] for both targets.
        let app_md = std::fs::read_dir(&vault)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.file_name().to_string_lossy().contains("crate__app."))
            .expect("crate__app.md exists");
        let content = std::fs::read_to_string(app_md.path()).unwrap();
        // Must contain a wikilink to the init function.
        assert!(
            content.contains("[[crate::app::init]]"),
            "app.md should wikilink to init"
        );
        assert!(content.contains("# app"), "frontmatter heading");
    }

    #[test]
    fn cypher_emitter_produces_correct_create_statements() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.cypher");
        emit_cypher(&toy_nodes(), &toy_edges(), &out).expect("emit_cypher");

        let text = std::fs::read_to_string(&out).unwrap();

        // One CREATE per node.
        let node_creates = text.matches("CREATE (:Node:").count();
        assert_eq!(node_creates, 5, "5 node CREATE statements");

        // One MATCH-CREATE per edge.
        let edge_creates = text.matches("MATCH (a:Node").count();
        assert_eq!(edge_creates, 4, "4 edge MATCH-CREATE statements");

        // Transaction batching keywords present.
        assert!(text.contains(":begin"), "must have :begin");
        assert!(text.contains(":commit"), "must have :commit");

        // Relationship type is SCREAMING_SNAKE_CASE.
        assert!(
            text.contains("CONTAINS") || text.contains("IMPLEMENTS"),
            "relationship types must be uppercase"
        );
    }

    #[test]
    fn svg_emitter_produces_circles_and_lines() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.svg");
        emit_svg(&toy_nodes(), &toy_edges(), &out).expect("emit_svg");

        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.starts_with("<?xml"), "SVG must start with XML decl");
        assert!(text.contains("<svg "), "must contain <svg> element");

        let circles = text.matches("<circle").count();
        // Legend circles + node circles; we must have at least 5 node circles.
        assert!(circles >= 5, "at least one circle per node; got {circles}");

        let lines = text.matches("<line").count();
        assert_eq!(lines, 4, "one line per edge");
    }

    #[test]
    fn jsonld_emitter_produces_valid_schema_org_document() {
        let td = tempfile::tempdir().unwrap();
        let out = td.path().join("g.jsonld");
        emit_jsonld(&toy_nodes(), &toy_edges(), &out, "test-project").expect("emit_jsonld");

        let text = std::fs::read_to_string(&out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid JSON-LD");

        assert_eq!(v["@context"], "https://schema.org");
        assert_eq!(v["@type"], "Dataset");

        let parts = v["hasPart"].as_array().expect("hasPart array");
        assert_eq!(parts.len(), 5, "5 nodes in hasPart");

        let relations = v["isBasedOn"].as_array().expect("isBasedOn array");
        assert_eq!(relations.len(), 4, "4 edges in isBasedOn");

        // Every node entry must have @type = DefinedTerm.
        for part in parts {
            assert_eq!(part["@type"], "DefinedTerm");
        }
    }

    // -------------------------------------------------------------------------
    // Filter logic — exercised without touching the filesystem.
    // -------------------------------------------------------------------------

    #[test]
    fn kinds_filter_drops_non_matching_nodes() {
        let mut nodes = toy_nodes();
        let allowed: HashSet<String> = ["function".to_string()].into_iter().collect();
        nodes.retain(|n| allowed.contains(&n.kind));
        assert_eq!(nodes.len(), 2, "only init and load survive");
        assert!(nodes.iter().all(|n| n.kind == "function"));
    }

    #[test]
    fn max_nodes_cap_is_respected() {
        let mut nodes = toy_nodes();
        let cap = 3usize;
        if nodes.len() > cap {
            nodes.truncate(cap);
        }
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn glob_filter_restricts_to_matching_file_paths() {
        let nodes = toy_nodes();
        let matcher = GlobMatcher::compile("src/config.rs");
        let kept: Vec<_> = nodes
            .iter()
            .filter(|n| n.file_path.as_deref().is_some_and(|p| matcher.matches(p)))
            .collect();
        assert_eq!(kept.len(), 2, "Config and load are in src/config.rs");
        assert!(kept.iter().all(|n| n.file_path.as_deref() == Some("src/config.rs")));
    }

    #[test]
    fn glob_double_star_matches_nested_paths() {
        let m = GlobMatcher::compile("src/**/*.rs");
        assert!(m.matches("src/app.rs"));
        assert!(m.matches("src/commands/export.rs"));
        assert!(!m.matches("tests/lib.rs"));
        // Windows separator normalisation.
        assert!(m.matches("src\\commands\\export.rs"));
    }

    // -------------------------------------------------------------------------
    // Edge dangling-endpoint guard (edges filtered when nodes are filtered).
    // -------------------------------------------------------------------------

    #[test]
    fn dangling_edges_are_dropped_when_nodes_are_filtered() {
        // Keep only trait nodes.  All edges reference non-trait nodes
        // on at least one side → all should be dropped.
        let mut nodes = toy_nodes();
        nodes.retain(|n| n.kind == "trait");
        let surviving: HashSet<String> = nodes.iter().map(|n| n.qualified_name.clone()).collect();
        let edges: Vec<EdgeRow> = toy_edges()
            .into_iter()
            .filter(|e| {
                surviving.contains(&e.source_qualified) && surviving.contains(&e.target_qualified)
            })
            .collect();
        assert!(edges.is_empty(), "no edges survive when only trait nodes remain");
    }
}
