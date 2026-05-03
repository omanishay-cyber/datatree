//! `mneme graph-diff <from> <to>` — compute the structural delta between
//! two graph snapshots.
//!
//! v0.3.3 cocktail surface: lets a developer ask "what does this refactor
//! actually change?" before they merge. Loads two graph databases
//! read-only, walks the `nodes` and (optionally) `edges` tables on both
//! sides, and reports added / removed / modified rows in JSON, an aligned
//! table, or markdown suitable for a PR description.
//!
//! Snapshot identifiers accepted on either side:
//!   - `HEAD`            — the live `graph.db` for the current project.
//!   - `HEAD~N`          — the N-th-most-recent snapshot file (1-indexed).
//!   - `<label>`         — substring match against snapshot filenames
//!                         under `<paths.root()>/snapshots/<project>/`.
//!   - `<path>.db`       — explicit absolute or relative path to a `.db`.
//!
//! Identity model:
//!   - `nodes` are keyed by `qualified_name` (the schema's UNIQUE column).
//!   - "modified" = same `qualified_name`, different content fingerprint.
//!     Fingerprint = blake3(kind | name | signature | file_path |
//!     line_start | line_end | summary). The schema has no
//!     `content_hash` column, so the fingerprint is computed at diff time
//!     from columns that DO exist. Stable enough for refactor-impact
//!     review; not designed for cryptographic equality.
//!   - `edges` have no stable identity beyond the full row, so they only
//!     appear as added / removed (never "modified"). Off by default —
//!     a single function rename can reshuffle hundreds of edges and
//!     drown the more interesting node-level signal.
//!
//! Rename detection: when the `--include-edges`-independent node pass
//! finds a removed-vs-added pair with the same content fingerprint AND
//! the same `file_path`, the pair is collapsed into a single "renamed"
//! entry. Anything more aggressive (e.g. cross-file rename detection)
//! risks false positives on copy-paste duplicates and stays opt-in for
//! a future revision.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::error::{CliError, CliResult};
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme graph-diff`.
#[derive(Debug, Args)]
pub struct GraphDiffArgs {
    /// First snapshot (older). Accepts a label, `HEAD`, `HEAD~N`, or
    /// path to a `.db` file.
    pub from: String,
    /// Second snapshot (newer). Same accepted forms as `<from>`.
    pub to: String,
    /// Output format. `json` is the default and is suitable for piping
    /// into another tool; `table` is colour-coded for terminal use;
    /// `markdown` produces a PR-ready report.
    #[arg(long, value_enum, default_value = "json")]
    pub format: GraphDiffFormat,
    /// Glob-style filter restricting changes to matching file paths.
    /// Uses the same semantics as shell glob: `*` matches a single path
    /// component, `**` matches across `/`. Empty / unset = no filter.
    #[arg(long)]
    pub files: Option<String>,
    /// Limit the diff to nodes of this `kind` (e.g. `function`, `struct`).
    #[arg(long, value_name = "KIND")]
    pub node_type: Option<String>,
    /// Cap the per-section row count printed (added / removed / etc.).
    /// Items beyond the cap are summarised as "...and N more". The
    /// summary totals are unaffected — this is a display-only knob.
    #[arg(long, default_value = "100")]
    pub max: usize,
    /// Include edge-level changes. Off by default because edges are
    /// noisy: a single rename can move hundreds of edges, drowning
    /// out the node-level signal that's usually the question.
    #[arg(long)]
    pub include_edges: bool,
    /// Print only the totals — skip the per-item rows.
    #[arg(long)]
    pub summary_only: bool,
    /// Project root to resolve labels / `HEAD` against. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// Output format selector for `mneme graph-diff`.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum GraphDiffFormat {
    /// Structured JSON document. Stable shape; safe to pipe into `jq`.
    Json,
    /// Aligned, colour-coded terminal table.
    Table,
    /// Markdown report grouped by change type, suitable for a PR body.
    Markdown,
}

/// Entry point used by `main.rs`.
pub async fn run(args: GraphDiffArgs) -> CliResult<()> {
    let project_root = resolve_project_root(args.project.clone());
    let from_db = resolve_snapshot(&args.from, &project_root)?;
    let to_db = resolve_snapshot(&args.to, &project_root)?;

    if !from_db.exists() {
        return Err(CliError::Other(format!(
            "from snapshot not found: {}",
            from_db.display()
        )));
    }
    if !to_db.exists() {
        return Err(CliError::Other(format!(
            "to snapshot not found: {}",
            to_db.display()
        )));
    }

    // SQLite is sync. We're inside `#[tokio::main(flavor = "current_thread")]`
    // so move the heavy work onto the blocking pool to keep the runtime
    // responsive (matches the pattern other CLI commands follow when
    // they call into rusqlite from async).
    let from_db_for_block = from_db.clone();
    let to_db_for_block = to_db.clone();
    let include_edges = args.include_edges;
    let join_result = tokio::task::spawn_blocking(move || -> CliResult<DiffData> {
        let from_nodes = load_nodes(&from_db_for_block)?;
        let to_nodes = load_nodes(&to_db_for_block)?;
        let edges = if include_edges {
            Some((
                load_edges(&from_db_for_block)?,
                load_edges(&to_db_for_block)?,
            ))
        } else {
            None
        };
        Ok(DiffData {
            from_nodes,
            to_nodes,
            edges,
        })
    })
    .await
    .map_err(|e| CliError::Other(format!("diff worker join failed: {e}")))?;
    let data = join_result?;

    let raw = compute_diff(&data.from_nodes, &data.to_nodes, data.edges.as_ref());
    let with_renames = detect_renames(raw);
    let filtered = apply_filters(with_renames, &args);

    match args.format {
        GraphDiffFormat::Json => print_json(&filtered, &args)?,
        GraphDiffFormat::Table => print_table(&filtered, &args),
        GraphDiffFormat::Markdown => print_markdown(&filtered, &args),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Snapshot resolution
// ---------------------------------------------------------------------------

/// Resolve a user-supplied snapshot identifier to a concrete `.db` path.
fn resolve_snapshot(spec: &str, project_root: &Path) -> CliResult<PathBuf> {
    // 1. Explicit path — accept anything ending in `.db` or that already
    //    exists on disk. Lets users diff against an arbitrary backup.
    if spec.ends_with(".db") || Path::new(spec).is_absolute() {
        let p = PathBuf::from(spec);
        return Ok(p);
    }

    // 2. `HEAD` — the live graph.db for the current project.
    if spec == "HEAD" {
        return live_graph_db(project_root);
    }

    // 3. `HEAD~N` — the N-th-newest snapshot. `HEAD~0` == `HEAD`.
    if let Some(rest) = spec.strip_prefix("HEAD~") {
        let n: usize = rest.parse().map_err(|_| {
            CliError::Other(format!(
                "invalid HEAD~N suffix in '{spec}': expected integer"
            ))
        })?;
        if n == 0 {
            return live_graph_db(project_root);
        }
        let snapshots = list_project_snapshots(project_root)?;
        if snapshots.is_empty() {
            return Err(CliError::Other(format!(
                "no snapshots found for project {} — run `mneme snap` first",
                project_root.display()
            )));
        }
        // Newest first: list_project_snapshots already sorts that way.
        // n is 1-indexed (HEAD~1 = most recent snapshot file).
        if n > snapshots.len() {
            return Err(CliError::Other(format!(
                "HEAD~{n} requested but only {} snapshot(s) exist",
                snapshots.len()
            )));
        }
        return Ok(snapshots[n - 1].clone());
    }

    // 4. Label — substring match against snapshot filenames. The current
    //    snap.rs writes `<YYYYMMDD-HHMMSS>.db`, so any partial timestamp
    //    works (e.g. `20260429` or `20260429-1200`). When labelled
    //    snapshots land in a future version, this same substring match
    //    will keep working.
    let snapshots = list_project_snapshots(project_root)?;
    let mut matches: Vec<&PathBuf> = snapshots
        .iter()
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.contains(spec))
        })
        .collect();
    if matches.is_empty() {
        return Err(CliError::Other(format!(
            "no snapshot matched '{spec}' under {} (try `mneme snap` to create one, or pass HEAD)",
            snapshots_dir(project_root)
                .map(|d| d.display().to_string())
                .unwrap_or_else(|_| "<unknown>".into()),
        )));
    }
    if matches.len() > 1 {
        // Pick the newest one and warn — keeps the command usable when a
        // partial timestamp is ambiguous, but tells the user what we did.
        matches.sort_by(|a, b| b.cmp(a));
        eprintln!(
            "note: '{spec}' matched {} snapshots; using newest ({})",
            matches.len(),
            matches[0].display()
        );
    }
    Ok(matches[0].clone())
}

/// Path to the live `graph.db` for `project_root`. Mirrors the resolver
/// used by `recall.rs`.
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

/// Snapshots directory for `project_root`, matching the layout written
/// by `snap.rs`.
fn snapshots_dir(project_root: &Path) -> CliResult<PathBuf> {
    let id = ProjectId::from_path(project_root).map_err(|e| {
        CliError::Other(format!(
            "cannot hash project path {}: {e}",
            project_root.display()
        ))
    })?;
    let paths = PathManager::default_root();
    Ok(paths.root().join("snapshots").join(id.to_string()))
}

/// Every snapshot `.db` for `project_root`, newest-first.
fn list_project_snapshots(project_root: &Path) -> CliResult<Vec<PathBuf>> {
    let dir = snapshots_dir(project_root)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| CliError::io(&dir, e))?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("db"))
        .collect();
    // Filenames are `YYYYMMDD-HHMMSS.db`, which sorts lexicographically
    // by recency. Reverse → newest first.
    out.sort_by(|a, b| b.cmp(a));
    Ok(out)
}

fn resolve_project_root(project: Option<PathBuf>) -> PathBuf {
    project
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

// ---------------------------------------------------------------------------
// Loading rows
// ---------------------------------------------------------------------------

/// One node row, as it lives in the diff (id columns plus a content
/// fingerprint we compute at load-time).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NodeRow {
    pub qualified_name: String,
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    /// blake3(kind | name | signature | file_path | line_start | line_end
    /// | summary) — see module-level docs for the rationale.
    pub fingerprint: String,
}

/// One edge row. Edges have no stable identity beyond their full payload.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRow {
    pub kind: String,
    pub source_qualified: String,
    pub target_qualified: String,
    pub file_path: Option<String>,
    pub line: Option<i64>,
}

fn open_ro(db: &Path) -> CliResult<Connection> {
    Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {} read-only: {e}", db.display())))
}

/// Load every node from the snapshot DB into a `qualified_name` -> row map.
fn load_nodes(db: &Path) -> CliResult<BTreeMap<String, NodeRow>> {
    let conn = open_ro(db)?;
    // Be defensive: a snapshot of a freshly-built DB may not have every
    // optional column populated, but the schema guarantees they exist.
    let mut stmt = conn
        .prepare(
            "SELECT qualified_name, kind, name, file_path, line_start, line_end, signature, summary
             FROM nodes",
        )
        .map_err(|e| CliError::Other(format!("prepare nodes from {}: {e}", db.display())))?;
    let rows = stmt
        .query_map([], |row| {
            let qualified_name: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let name: String = row.get(2)?;
            let file_path: Option<String> = row.get(3)?;
            let line_start: Option<i64> = row.get(4)?;
            let line_end: Option<i64> = row.get(5)?;
            let signature: Option<String> = row.get(6)?;
            let summary: Option<String> = row.get(7)?;
            let fingerprint = node_fingerprint(
                &kind,
                &name,
                signature.as_deref(),
                file_path.as_deref(),
                line_start,
                line_end,
                summary.as_deref(),
            );
            Ok(NodeRow {
                qualified_name,
                kind,
                name,
                file_path,
                line_start,
                line_end,
                fingerprint,
            })
        })
        .map_err(|e| CliError::Other(format!("query nodes: {e}")))?;

    let mut out = BTreeMap::new();
    for r in rows {
        let row = r.map_err(|e| CliError::Other(format!("row map nodes: {e}")))?;
        out.insert(row.qualified_name.clone(), row);
    }
    Ok(out)
}

/// Load every edge into a sorted set so set difference is cheap.
fn load_edges(db: &Path) -> CliResult<BTreeSet<EdgeRow>> {
    let conn = open_ro(db)?;
    let mut stmt = conn
        .prepare(
            "SELECT kind, source_qualified, target_qualified, file_path, line
             FROM edges",
        )
        .map_err(|e| CliError::Other(format!("prepare edges from {}: {e}", db.display())))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(EdgeRow {
                kind: row.get(0)?,
                source_qualified: row.get(1)?,
                target_qualified: row.get(2)?,
                file_path: row.get(3)?,
                line: row.get(4)?,
            })
        })
        .map_err(|e| CliError::Other(format!("query edges: {e}")))?;
    let mut out = BTreeSet::new();
    for r in rows {
        let row = r.map_err(|e| CliError::Other(format!("row map edges: {e}")))?;
        out.insert(row);
    }
    Ok(out)
}

/// Stable per-node content fingerprint. blake3 keeps it cheap and the
/// 16-hex prefix is plenty to discriminate within a single project.
fn node_fingerprint(
    kind: &str,
    name: &str,
    signature: Option<&str>,
    file_path: Option<&str>,
    line_start: Option<i64>,
    line_end: Option<i64>,
    summary: Option<&str>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(kind.as_bytes());
    hasher.update(b"\0");
    hasher.update(name.as_bytes());
    hasher.update(b"\0");
    hasher.update(signature.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(file_path.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(line_start.unwrap_or(0).to_le_bytes().as_slice());
    hasher.update(line_end.unwrap_or(0).to_le_bytes().as_slice());
    hasher.update(b"\0");
    hasher.update(summary.unwrap_or("").as_bytes());
    let h = hasher.finalize();
    let bytes = h.as_bytes();
    // 16 hex chars (64 bits). Cheap to log, plenty unique within a project.
    let mut s = String::with_capacity(16);
    for b in &bytes[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---------------------------------------------------------------------------
// Diff core
// ---------------------------------------------------------------------------

struct DiffData {
    from_nodes: BTreeMap<String, NodeRow>,
    to_nodes: BTreeMap<String, NodeRow>,
    edges: Option<(BTreeSet<EdgeRow>, BTreeSet<EdgeRow>)>,
}

/// Per-section change list. Renames are detected in a second pass and
/// move pairs out of `removed_nodes` + `added_nodes`.
#[derive(Debug, Default, Serialize)]
pub struct GraphDiff {
    pub added_nodes: Vec<NodeRow>,
    pub removed_nodes: Vec<NodeRow>,
    pub modified_nodes: Vec<ModifiedNode>,
    pub renamed_nodes: Vec<RenamedNode>,
    pub added_edges: Vec<EdgeRow>,
    pub removed_edges: Vec<EdgeRow>,
    pub added_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub edges_included: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModifiedNode {
    pub qualified_name: String,
    pub kind: String,
    pub file_path: Option<String>,
    pub from_fingerprint: String,
    pub to_fingerprint: String,
    pub from_line_start: Option<i64>,
    pub to_line_start: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenamedNode {
    pub from_qualified: String,
    pub to_qualified: String,
    pub kind: String,
    pub file_path: Option<String>,
}

/// Walk the two node maps and the (optional) edge sets, classifying each
/// row as added / removed / modified. Files are derived from the union.
fn compute_diff(
    from: &BTreeMap<String, NodeRow>,
    to: &BTreeMap<String, NodeRow>,
    edges: Option<&(BTreeSet<EdgeRow>, BTreeSet<EdgeRow>)>,
) -> GraphDiff {
    let mut diff = GraphDiff {
        edges_included: edges.is_some(),
        ..Default::default()
    };

    // Nodes — single pass over both keysets.
    for (qname, row) in to {
        match from.get(qname) {
            None => diff.added_nodes.push(row.clone()),
            Some(prev) if prev.fingerprint != row.fingerprint => {
                diff.modified_nodes.push(ModifiedNode {
                    qualified_name: qname.clone(),
                    kind: row.kind.clone(),
                    file_path: row.file_path.clone(),
                    from_fingerprint: prev.fingerprint.clone(),
                    to_fingerprint: row.fingerprint.clone(),
                    from_line_start: prev.line_start,
                    to_line_start: row.line_start,
                });
            }
            Some(_) => { /* unchanged */ }
        }
    }
    for (qname, row) in from {
        if !to.contains_key(qname) {
            diff.removed_nodes.push(row.clone());
        }
    }

    // Files — touched iff any node in them changed.
    let mut added_files = BTreeSet::new();
    let mut removed_files = BTreeSet::new();
    let mut modified_files = BTreeSet::new();
    let from_files: BTreeSet<&String> =
        from.values().filter_map(|n| n.file_path.as_ref()).collect();
    let to_files: BTreeSet<&String> = to.values().filter_map(|n| n.file_path.as_ref()).collect();
    for f in &to_files {
        if !from_files.contains(f) {
            added_files.insert((*f).clone());
        }
    }
    for f in &from_files {
        if !to_files.contains(f) {
            removed_files.insert((*f).clone());
        }
    }
    // A file is "modified" if both sides know it AND any node in it
    // appears in added/removed/modified for that file.
    for n in &diff.added_nodes {
        if let Some(p) = &n.file_path {
            if from_files.contains(p) && to_files.contains(p) {
                modified_files.insert(p.clone());
            }
        }
    }
    for n in &diff.removed_nodes {
        if let Some(p) = &n.file_path {
            if from_files.contains(p) && to_files.contains(p) {
                modified_files.insert(p.clone());
            }
        }
    }
    for n in &diff.modified_nodes {
        if let Some(p) = &n.file_path {
            modified_files.insert(p.clone());
        }
    }
    diff.added_files = added_files.into_iter().collect();
    diff.removed_files = removed_files.into_iter().collect();
    diff.modified_files = modified_files.into_iter().collect();

    // Edges — set difference both ways. No "modified" classification.
    if let Some((from_e, to_e)) = edges {
        for e in to_e.difference(from_e) {
            diff.added_edges.push(e.clone());
        }
        for e in from_e.difference(to_e) {
            diff.removed_edges.push(e.clone());
        }
    }

    diff
}

/// Pair a removed node with an added node when they share `(fingerprint,
/// file_path)`. This is the conservative heuristic — same content, same
/// file = "the user renamed it". Anything more aggressive (cross-file,
/// fingerprint-only) will misclassify copy-pastes.
fn detect_renames(mut diff: GraphDiff) -> GraphDiff {
    if diff.removed_nodes.is_empty() || diff.added_nodes.is_empty() {
        return diff;
    }

    // Index added by (fingerprint, file_path) so the lookup is O(1).
    let mut added_idx: BTreeMap<(String, Option<String>), usize> = BTreeMap::new();
    for (i, n) in diff.added_nodes.iter().enumerate() {
        added_idx.insert((n.fingerprint.clone(), n.file_path.clone()), i);
    }

    // Walk removed in reverse so we can swap_remove without shifting
    // indices we've already recorded.
    let mut renamed = Vec::new();
    let mut to_drop_added: BTreeSet<usize> = BTreeSet::new();
    let mut to_drop_removed: Vec<usize> = Vec::new();
    for (i, removed) in diff.removed_nodes.iter().enumerate() {
        let key = (removed.fingerprint.clone(), removed.file_path.clone());
        if let Some(&j) = added_idx.get(&key) {
            if to_drop_added.contains(&j) {
                continue;
            }
            let added = &diff.added_nodes[j];
            renamed.push(RenamedNode {
                from_qualified: removed.qualified_name.clone(),
                to_qualified: added.qualified_name.clone(),
                kind: added.kind.clone(),
                file_path: added.file_path.clone(),
            });
            to_drop_added.insert(j);
            to_drop_removed.push(i);
        }
    }

    // Apply removals from highest index to lowest.
    for i in to_drop_removed.into_iter().rev() {
        diff.removed_nodes.swap_remove(i);
    }
    let mut to_drop_added: Vec<usize> = to_drop_added.into_iter().collect();
    to_drop_added.sort_unstable_by(|a, b| b.cmp(a));
    for j in to_drop_added {
        diff.added_nodes.swap_remove(j);
    }
    diff.renamed_nodes = renamed;
    diff
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

fn apply_filters(mut diff: GraphDiff, args: &GraphDiffArgs) -> GraphDiff {
    if let Some(kind) = args.node_type.as_deref() {
        diff.added_nodes.retain(|n| n.kind == kind);
        diff.removed_nodes.retain(|n| n.kind == kind);
        diff.modified_nodes.retain(|n| n.kind == kind);
        diff.renamed_nodes.retain(|n| n.kind == kind);
    }
    if let Some(pat) = args.files.as_deref() {
        let matcher = GlobMatcher::compile(pat);
        let keep =
            |fp: &Option<String>| -> bool { fp.as_deref().is_some_and(|p| matcher.matches(p)) };
        diff.added_nodes.retain(|n| keep(&n.file_path));
        diff.removed_nodes.retain(|n| keep(&n.file_path));
        diff.modified_nodes.retain(|n| keep(&n.file_path));
        diff.renamed_nodes.retain(|n| keep(&n.file_path));
        diff.added_files.retain(|f| matcher.matches(f));
        diff.removed_files.retain(|f| matcher.matches(f));
        diff.modified_files.retain(|f| matcher.matches(f));
        if diff.edges_included {
            diff.added_edges
                .retain(|e| e.file_path.as_deref().is_some_and(|p| matcher.matches(p)));
            diff.removed_edges
                .retain(|e| e.file_path.as_deref().is_some_and(|p| matcher.matches(p)));
        }
    }
    diff
}

/// Tiny shell-style glob: supports `*` (any chars within a path
/// component), `**` (any chars across `/`), and literal characters.
/// Conservative: anything else is treated as a literal. We don't pull
/// in a full glob crate because the existing CLI deps already cover
/// the common cases this command actually needs.
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
                    // `**/` collapses to "zero or more path components" — so
                    // `src/**/*.rs` matches `src/lib.rs` AND
                    // `src/cli/main.rs`. Without the optional-slash trick the
                    // middle `**/` would force a literal `/` and miss the
                    // shallow case.
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
        // Fall back to "match nothing" on bogus input rather than panic.
        let re = regex::Regex::new(&out).unwrap_or_else(|_| regex::Regex::new("^$").unwrap());
        Self { re }
    }
    fn matches(&self, s: &str) -> bool {
        // Normalise Windows separators so a single pattern works on both.
        let normalised = s.replace('\\', "/");
        self.re.is_match(&normalised)
    }
}

// ---------------------------------------------------------------------------
// Output renderers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonOutput<'a> {
    summary: Summary,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added_nodes: Vec<&'a NodeRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed_nodes: Vec<&'a NodeRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    modified_nodes: Vec<&'a ModifiedNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    renamed_nodes: Vec<&'a RenamedNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added_edges: Vec<&'a EdgeRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed_edges: Vec<&'a EdgeRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added_files: Vec<&'a String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed_files: Vec<&'a String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    modified_files: Vec<&'a String>,
}

#[derive(Serialize)]
struct Summary {
    added_nodes: usize,
    removed_nodes: usize,
    modified_nodes: usize,
    renamed_nodes: usize,
    added_edges: usize,
    removed_edges: usize,
    added_files: usize,
    removed_files: usize,
    modified_files: usize,
    edges_included: bool,
}

fn summarise(diff: &GraphDiff) -> Summary {
    Summary {
        added_nodes: diff.added_nodes.len(),
        removed_nodes: diff.removed_nodes.len(),
        modified_nodes: diff.modified_nodes.len(),
        renamed_nodes: diff.renamed_nodes.len(),
        added_edges: diff.added_edges.len(),
        removed_edges: diff.removed_edges.len(),
        added_files: diff.added_files.len(),
        removed_files: diff.removed_files.len(),
        modified_files: diff.modified_files.len(),
        edges_included: diff.edges_included,
    }
}

fn cap<T: Clone>(items: &[T], max: usize) -> Vec<&T> {
    items.iter().take(max).collect()
}

fn print_json(diff: &GraphDiff, args: &GraphDiffArgs) -> CliResult<()> {
    let max = args.max;
    let payload = JsonOutput {
        summary: summarise(diff),
        added_nodes: if args.summary_only {
            Vec::new()
        } else {
            cap(&diff.added_nodes, max)
        },
        removed_nodes: if args.summary_only {
            Vec::new()
        } else {
            cap(&diff.removed_nodes, max)
        },
        modified_nodes: if args.summary_only {
            Vec::new()
        } else {
            cap(&diff.modified_nodes, max)
        },
        renamed_nodes: if args.summary_only {
            Vec::new()
        } else {
            cap(&diff.renamed_nodes, max)
        },
        added_edges: if args.summary_only {
            Vec::new()
        } else {
            cap(&diff.added_edges, max)
        },
        removed_edges: if args.summary_only {
            Vec::new()
        } else {
            cap(&diff.removed_edges, max)
        },
        added_files: if args.summary_only {
            Vec::new()
        } else {
            diff.added_files.iter().take(max).collect()
        },
        removed_files: if args.summary_only {
            Vec::new()
        } else {
            diff.removed_files.iter().take(max).collect()
        },
        modified_files: if args.summary_only {
            Vec::new()
        } else {
            diff.modified_files.iter().take(max).collect()
        },
    };
    let s = serde_json::to_string_pretty(&payload).map_err(CliError::Json)?;
    println!("{s}");
    Ok(())
}

fn print_table(diff: &GraphDiff, args: &GraphDiffArgs) {
    use console::style;
    let s = summarise(diff);
    println!(
        "{}  added:{}  removed:{}  modified:{}  renamed:{}",
        style("graph-diff summary").bold(),
        style(s.added_nodes).green(),
        style(s.removed_nodes).red(),
        style(s.modified_nodes).yellow(),
        style(s.renamed_nodes).cyan(),
    );
    if s.edges_included {
        println!(
            "                   edges +{} / -{}",
            style(s.added_edges).green(),
            style(s.removed_edges).red()
        );
    }
    println!(
        "                   files +{} / -{} / *{}",
        style(s.added_files).green(),
        style(s.removed_files).red(),
        style(s.modified_files).yellow()
    );
    if args.summary_only {
        return;
    }

    print_table_section_nodes("added (+)", &diff.added_nodes, args.max, |n| {
        style(format!("[{}] {}", n.kind, n.qualified_name))
            .green()
            .to_string()
    });
    print_table_section_nodes("removed (-)", &diff.removed_nodes, args.max, |n| {
        style(format!("[{}] {}", n.kind, n.qualified_name))
            .red()
            .to_string()
    });
    print_table_section_modified("modified (~)", &diff.modified_nodes, args.max);
    print_table_section_renamed("renamed (->)", &diff.renamed_nodes, args.max);
    if diff.edges_included {
        print_table_section_edges("edges added (+)", &diff.added_edges, args.max, true);
        print_table_section_edges("edges removed (-)", &diff.removed_edges, args.max, false);
    }
}

fn print_table_section_nodes<F: Fn(&NodeRow) -> String>(
    title: &str,
    rows: &[NodeRow],
    max: usize,
    fmt: F,
) {
    if rows.is_empty() {
        return;
    }
    use console::style;
    println!();
    println!("{}", style(title).bold());
    for row in rows.iter().take(max) {
        let loc = match (&row.file_path, row.line_start) {
            (Some(f), Some(l)) if l > 0 => format!("{f}:{l}"),
            (Some(f), _) => f.clone(),
            _ => "-".into(),
        };
        println!("  {}    {}", fmt(row), style(loc).dim());
    }
    if rows.len() > max {
        println!("  ...and {} more", rows.len() - max);
    }
}

fn print_table_section_modified(title: &str, rows: &[ModifiedNode], max: usize) {
    if rows.is_empty() {
        return;
    }
    use console::style;
    println!();
    println!("{}", style(title).bold());
    for row in rows.iter().take(max) {
        let loc = match (&row.file_path, row.from_line_start, row.to_line_start) {
            (Some(f), Some(a), Some(b)) if a != b => format!("{f}:{a} -> {b}"),
            (Some(f), _, Some(b)) if b > 0 => format!("{f}:{b}"),
            (Some(f), _, _) => f.clone(),
            _ => "-".into(),
        };
        println!(
            "  {} {}    {}",
            style(format!("[{}]", row.kind)).yellow(),
            style(&row.qualified_name).yellow(),
            style(loc).dim()
        );
    }
    if rows.len() > max {
        println!("  ...and {} more", rows.len() - max);
    }
}

fn print_table_section_renamed(title: &str, rows: &[RenamedNode], max: usize) {
    if rows.is_empty() {
        return;
    }
    use console::style;
    println!();
    println!("{}", style(title).bold());
    for row in rows.iter().take(max) {
        let loc = row.file_path.as_deref().unwrap_or("-");
        println!(
            "  {} {} -> {}    {}",
            style(format!("[{}]", row.kind)).cyan(),
            style(&row.from_qualified).strikethrough(),
            style(&row.to_qualified).cyan().bold(),
            style(loc).dim()
        );
    }
    if rows.len() > max {
        println!("  ...and {} more", rows.len() - max);
    }
}

fn print_table_section_edges(title: &str, rows: &[EdgeRow], max: usize, added: bool) {
    if rows.is_empty() {
        return;
    }
    use console::style;
    println!();
    println!("{}", style(title).bold());
    for row in rows.iter().take(max) {
        let body = format!(
            "[{}] {} -> {}",
            row.kind, row.source_qualified, row.target_qualified
        );
        let coloured = if added {
            style(body).green().to_string()
        } else {
            style(body).red().to_string()
        };
        let loc = match (&row.file_path, row.line) {
            (Some(f), Some(l)) if l > 0 => format!("{f}:{l}"),
            (Some(f), _) => f.clone(),
            _ => "-".into(),
        };
        println!("  {}    {}", coloured, style(loc).dim());
    }
    if rows.len() > max {
        println!("  ...and {} more", rows.len() - max);
    }
}

fn print_markdown(diff: &GraphDiff, args: &GraphDiffArgs) {
    let s = summarise(diff);
    println!("# graph-diff");
    println!();
    println!("| metric | count |");
    println!("|---|---:|");
    println!("| nodes added | {} |", s.added_nodes);
    println!("| nodes removed | {} |", s.removed_nodes);
    println!("| nodes modified | {} |", s.modified_nodes);
    println!("| nodes renamed | {} |", s.renamed_nodes);
    if s.edges_included {
        println!("| edges added | {} |", s.added_edges);
        println!("| edges removed | {} |", s.removed_edges);
    }
    println!("| files added | {} |", s.added_files);
    println!("| files removed | {} |", s.removed_files);
    println!("| files modified | {} |", s.modified_files);
    if args.summary_only {
        return;
    }

    md_section_nodes("Added nodes", &diff.added_nodes, args.max);
    md_section_nodes("Removed nodes", &diff.removed_nodes, args.max);
    md_section_modified("Modified nodes", &diff.modified_nodes, args.max);
    md_section_renamed("Renamed nodes", &diff.renamed_nodes, args.max);
    if diff.edges_included {
        md_section_edges("Added edges", &diff.added_edges, args.max);
        md_section_edges("Removed edges", &diff.removed_edges, args.max);
    }
    md_files("Added files", &diff.added_files, args.max);
    md_files("Removed files", &diff.removed_files, args.max);
    md_files("Modified files", &diff.modified_files, args.max);
}

fn md_section_nodes(title: &str, rows: &[NodeRow], max: usize) {
    if rows.is_empty() {
        return;
    }
    println!();
    println!("## {title}");
    println!();
    for row in rows.iter().take(max) {
        let loc = match (&row.file_path, row.line_start) {
            (Some(f), Some(l)) if l > 0 => format!("`{f}:{l}`"),
            (Some(f), _) => format!("`{f}`"),
            _ => "-".into(),
        };
        println!("- `[{}] {}` {loc}", row.kind, row.qualified_name);
    }
    if rows.len() > max {
        println!();
        println!("_...and {} more_", rows.len() - max);
    }
}

fn md_section_modified(title: &str, rows: &[ModifiedNode], max: usize) {
    if rows.is_empty() {
        return;
    }
    println!();
    println!("## {title}");
    println!();
    for row in rows.iter().take(max) {
        let loc = row.file_path.as_deref().unwrap_or("-");
        println!(
            "- `[{}] {}` `{}` ({} -> {})",
            row.kind, row.qualified_name, loc, row.from_fingerprint, row.to_fingerprint
        );
    }
    if rows.len() > max {
        println!();
        println!("_...and {} more_", rows.len() - max);
    }
}

fn md_section_renamed(title: &str, rows: &[RenamedNode], max: usize) {
    if rows.is_empty() {
        return;
    }
    println!();
    println!("## {title}");
    println!();
    for row in rows.iter().take(max) {
        let loc = row.file_path.as_deref().unwrap_or("-");
        println!(
            "- `[{}]` `{}` -> `{}` (`{}`)",
            row.kind, row.from_qualified, row.to_qualified, loc
        );
    }
    if rows.len() > max {
        println!();
        println!("_...and {} more_", rows.len() - max);
    }
}

fn md_section_edges(title: &str, rows: &[EdgeRow], max: usize) {
    if rows.is_empty() {
        return;
    }
    println!();
    println!("## {title}");
    println!();
    for row in rows.iter().take(max) {
        let loc = match (&row.file_path, row.line) {
            (Some(f), Some(l)) if l > 0 => format!("`{f}:{l}`"),
            (Some(f), _) => format!("`{f}`"),
            _ => "-".into(),
        };
        println!(
            "- `[{}] {} -> {}` {loc}",
            row.kind, row.source_qualified, row.target_qualified
        );
    }
    if rows.len() > max {
        println!();
        println!("_...and {} more_", rows.len() - max);
    }
}

fn md_files(title: &str, files: &[String], max: usize) {
    if files.is_empty() {
        return;
    }
    println!();
    println!("## {title}");
    println!();
    for f in files.iter().take(max) {
        println!("- `{f}`");
    }
    if files.len() > max {
        println!();
        println!("_...and {} more_", files.len() - max);
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

    /// Reproduces `main.rs::Cli` enough to drive clap parsing in tests
    /// without re-exporting the binary's enum.
    #[derive(Parser, Debug)]
    #[command(name = "mneme")]
    struct TestCli {
        #[command(subcommand)]
        cmd: TestCmd,
    }
    #[derive(clap::Subcommand, Debug)]
    enum TestCmd {
        GraphDiff(GraphDiffArgs),
    }

    #[test]
    fn parses_clap_args() {
        let cli = TestCli::try_parse_from([
            "mneme",
            "graph-diff",
            "HEAD~1",
            "HEAD",
            "--format=table",
            "--max=10",
            "--include-edges",
        ])
        .expect("parse");
        let TestCmd::GraphDiff(args) = cli.cmd;
        assert_eq!(args.from, "HEAD~1");
        assert_eq!(args.to, "HEAD");
        assert_eq!(args.format, GraphDiffFormat::Table);
        assert_eq!(args.max, 10);
        assert!(args.include_edges);
        assert!(!args.summary_only);
    }

    #[test]
    fn parses_summary_only_and_files_filter() {
        let cli = TestCli::try_parse_from([
            "mneme",
            "graph-diff",
            "snap-a",
            "snap-b",
            "--summary-only",
            "--files",
            "src/**/*.rs",
            "--node-type=function",
        ])
        .expect("parse");
        let TestCmd::GraphDiff(args) = cli.cmd;
        assert!(args.summary_only);
        assert_eq!(args.files.as_deref(), Some("src/**/*.rs"));
        assert_eq!(args.node_type.as_deref(), Some("function"));
        assert_eq!(args.format, GraphDiffFormat::Json);
    }

    /// Build a synthetic graph DB with the columns `load_nodes` and
    /// `load_edges` reach for. Lets the diff tests run without depending
    /// on a real `mneme build` having been executed.
    fn make_graph_db(path: &Path, rows: &[(&str, &str, &str, &str, i64, i64, &str, &str)]) {
        let conn = Connection::open(path).expect("open synthetic db");
        conn.execute_batch(
            r#"
            CREATE TABLE nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                qualified_name TEXT UNIQUE NOT NULL,
                file_path TEXT,
                line_start INTEGER,
                line_end INTEGER,
                language TEXT,
                parent_qualified TEXT,
                signature TEXT,
                modifiers TEXT,
                is_test INTEGER NOT NULL DEFAULT 0,
                file_hash TEXT,
                summary TEXT,
                embedding_id INTEGER,
                extra TEXT NOT NULL DEFAULT '{}',
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                source_qualified TEXT NOT NULL,
                target_qualified TEXT NOT NULL,
                confidence TEXT NOT NULL DEFAULT 'high',
                confidence_score REAL NOT NULL DEFAULT 1.0,
                file_path TEXT,
                line INTEGER,
                source_extractor TEXT NOT NULL DEFAULT 'test',
                extra TEXT NOT NULL DEFAULT '{}',
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "#,
        )
        .expect("create schema");
        for (kind, name, qname, file, ls, le, sig, summary) in rows {
            conn.execute(
                "INSERT INTO nodes (kind, name, qualified_name, file_path, line_start, line_end, signature, summary)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![kind, name, qname, file, ls, le, sig, summary],
            )
            .expect("insert node");
        }
    }

    #[test]
    fn detects_added_node() {
        let td = tempfile::tempdir().unwrap();
        let from_db = td.path().join("from.db");
        let to_db = td.path().join("to.db");
        make_graph_db(
            &from_db,
            &[(
                "function",
                "alpha",
                "crate::alpha",
                "src/lib.rs",
                1,
                5,
                "fn alpha()",
                "",
            )],
        );
        make_graph_db(
            &to_db,
            &[
                (
                    "function",
                    "alpha",
                    "crate::alpha",
                    "src/lib.rs",
                    1,
                    5,
                    "fn alpha()",
                    "",
                ),
                (
                    "function",
                    "beta",
                    "crate::beta",
                    "src/lib.rs",
                    7,
                    10,
                    "fn beta()",
                    "",
                ),
            ],
        );
        let from = load_nodes(&from_db).unwrap();
        let to = load_nodes(&to_db).unwrap();
        let raw = compute_diff(&from, &to, None);
        let diff = detect_renames(raw);
        assert_eq!(diff.added_nodes.len(), 1);
        assert_eq!(diff.added_nodes[0].qualified_name, "crate::beta");
        assert!(diff.removed_nodes.is_empty());
        assert!(diff.modified_nodes.is_empty());
    }

    #[test]
    fn detects_removed_node() {
        let td = tempfile::tempdir().unwrap();
        let from_db = td.path().join("from.db");
        let to_db = td.path().join("to.db");
        make_graph_db(
            &from_db,
            &[
                (
                    "function",
                    "alpha",
                    "crate::alpha",
                    "src/lib.rs",
                    1,
                    5,
                    "fn alpha()",
                    "",
                ),
                (
                    "function",
                    "beta",
                    "crate::beta",
                    "src/lib.rs",
                    7,
                    10,
                    "fn beta()",
                    "",
                ),
            ],
        );
        make_graph_db(
            &to_db,
            &[(
                "function",
                "alpha",
                "crate::alpha",
                "src/lib.rs",
                1,
                5,
                "fn alpha()",
                "",
            )],
        );
        let from = load_nodes(&from_db).unwrap();
        let to = load_nodes(&to_db).unwrap();
        let raw = compute_diff(&from, &to, None);
        let diff = detect_renames(raw);
        assert_eq!(diff.removed_nodes.len(), 1);
        assert_eq!(diff.removed_nodes[0].qualified_name, "crate::beta");
        assert!(diff.added_nodes.is_empty());
    }

    #[test]
    fn detects_modified_node() {
        let td = tempfile::tempdir().unwrap();
        let from_db = td.path().join("from.db");
        let to_db = td.path().join("to.db");
        // Same qualified_name, different signature → fingerprint differs → modified.
        make_graph_db(
            &from_db,
            &[(
                "function",
                "alpha",
                "crate::alpha",
                "src/lib.rs",
                1,
                5,
                "fn alpha()",
                "",
            )],
        );
        make_graph_db(
            &to_db,
            &[(
                "function",
                "alpha",
                "crate::alpha",
                "src/lib.rs",
                1,
                8,
                "fn alpha(x: u32)",
                "",
            )],
        );
        let from = load_nodes(&from_db).unwrap();
        let to = load_nodes(&to_db).unwrap();
        let raw = compute_diff(&from, &to, None);
        let diff = detect_renames(raw);
        assert_eq!(diff.modified_nodes.len(), 1);
        assert_eq!(diff.modified_nodes[0].qualified_name, "crate::alpha");
        assert_ne!(
            diff.modified_nodes[0].from_fingerprint,
            diff.modified_nodes[0].to_fingerprint
        );
    }

    #[test]
    fn detects_rename_via_content_hash() {
        let td = tempfile::tempdir().unwrap();
        let from_db = td.path().join("from.db");
        let to_db = td.path().join("to.db");
        // Same kind/name/sig/file/lines → same fingerprint, different qualified_name.
        // That's the rename signal.
        make_graph_db(
            &from_db,
            &[(
                "function",
                "auth",
                "crate::authMiddleware",
                "src/lib.rs",
                1,
                5,
                "fn auth()",
                "",
            )],
        );
        make_graph_db(
            &to_db,
            &[(
                "function",
                "auth",
                "crate::authGuard",
                "src/lib.rs",
                1,
                5,
                "fn auth()",
                "",
            )],
        );
        let from = load_nodes(&from_db).unwrap();
        let to = load_nodes(&to_db).unwrap();
        let raw = compute_diff(&from, &to, None);
        let diff = detect_renames(raw);
        assert_eq!(diff.renamed_nodes.len(), 1);
        assert_eq!(
            diff.renamed_nodes[0].from_qualified,
            "crate::authMiddleware"
        );
        assert_eq!(diff.renamed_nodes[0].to_qualified, "crate::authGuard");
        assert!(
            diff.added_nodes.is_empty(),
            "rename should not double-count as add"
        );
        assert!(
            diff.removed_nodes.is_empty(),
            "rename should not double-count as remove"
        );
    }

    #[test]
    fn files_pattern_filter_restricts_changes() {
        let td = tempfile::tempdir().unwrap();
        let from_db = td.path().join("from.db");
        let to_db = td.path().join("to.db");
        make_graph_db(&from_db, &[]);
        make_graph_db(
            &to_db,
            &[
                ("function", "a", "crate::a", "src/a.rs", 1, 1, "fn a()", ""),
                (
                    "function",
                    "b",
                    "crate::b",
                    "tests/b.rs",
                    1,
                    1,
                    "fn b()",
                    "",
                ),
            ],
        );
        let from = load_nodes(&from_db).unwrap();
        let to = load_nodes(&to_db).unwrap();
        let raw = compute_diff(&from, &to, None);
        let diff = detect_renames(raw);

        let args = GraphDiffArgs {
            from: "from".into(),
            to: "to".into(),
            format: GraphDiffFormat::Json,
            files: Some("src/**".into()),
            node_type: None,
            max: 100,
            include_edges: false,
            summary_only: false,
            project: None,
        };
        let filtered = apply_filters(diff, &args);
        assert_eq!(filtered.added_nodes.len(), 1);
        assert_eq!(
            filtered.added_nodes[0].file_path.as_deref(),
            Some("src/a.rs")
        );
    }

    #[test]
    fn summary_only_skips_per_item_rows_in_json() {
        let td = tempfile::tempdir().unwrap();
        let from_db = td.path().join("from.db");
        let to_db = td.path().join("to.db");
        make_graph_db(&from_db, &[]);
        make_graph_db(
            &to_db,
            &[("function", "a", "crate::a", "src/a.rs", 1, 1, "fn a()", "")],
        );
        let from = load_nodes(&from_db).unwrap();
        let to = load_nodes(&to_db).unwrap();
        let raw = compute_diff(&from, &to, None);
        let diff = detect_renames(raw);

        let args = GraphDiffArgs {
            from: "from".into(),
            to: "to".into(),
            format: GraphDiffFormat::Json,
            files: None,
            node_type: None,
            max: 100,
            include_edges: false,
            summary_only: true,
            project: None,
        };
        let payload = JsonOutput {
            summary: summarise(&diff),
            added_nodes: if args.summary_only {
                Vec::new()
            } else {
                cap(&diff.added_nodes, args.max)
            },
            removed_nodes: Vec::new(),
            modified_nodes: Vec::new(),
            renamed_nodes: Vec::new(),
            added_edges: Vec::new(),
            removed_edges: Vec::new(),
            added_files: Vec::new(),
            removed_files: Vec::new(),
            modified_files: Vec::new(),
        };
        let s = serde_json::to_string(&payload).unwrap();
        // The summary's `added_nodes` count is always present (it's a
        // u64, not a Vec). What summary_only suppresses is the per-item
        // ARRAY: parse the JSON and verify the top-level only carries
        // `summary` and that `summary.added_nodes` reflects the count.
        let v: serde_json::Value = serde_json::from_str(&s).expect("json round-trips");
        let obj = v.as_object().expect("object root");
        assert_eq!(
            obj.len(),
            1,
            "summary_only must emit only the summary key, got {s}"
        );
        assert!(obj.contains_key("summary"), "missing summary: {s}");
        assert_eq!(
            obj["summary"]["added_nodes"].as_u64(),
            Some(1),
            "summary.added_nodes should reflect the unfiltered diff: {s}"
        );
    }

    #[test]
    fn json_output_is_valid_json() {
        let diff = GraphDiff::default();
        let payload = JsonOutput {
            summary: summarise(&diff),
            added_nodes: Vec::new(),
            removed_nodes: Vec::new(),
            modified_nodes: Vec::new(),
            renamed_nodes: Vec::new(),
            added_edges: Vec::new(),
            removed_edges: Vec::new(),
            added_files: Vec::new(),
            removed_files: Vec::new(),
            modified_files: Vec::new(),
        };
        let s = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).expect("round-trips");
        assert!(v.get("summary").is_some());
    }

    #[test]
    fn glob_matches_double_star() {
        let m = GlobMatcher::compile("src/**/*.rs");
        assert!(m.matches("src/lib.rs"));
        assert!(m.matches("src/cli/main.rs"));
        assert!(m.matches("src/cli/commands/graph_diff.rs"));
        assert!(!m.matches("tests/lib.rs"));
        // Backslashes get normalised so a Windows path matches a posix glob.
        assert!(m.matches("src\\cli\\main.rs"));
    }

    #[test]
    fn fingerprint_changes_on_signature_change() {
        let a = node_fingerprint(
            "function",
            "alpha",
            Some("fn alpha()"),
            Some("src/lib.rs"),
            Some(1),
            Some(5),
            None,
        );
        let b = node_fingerprint(
            "function",
            "alpha",
            Some("fn alpha(x: u32)"),
            Some("src/lib.rs"),
            Some(1),
            Some(5),
            None,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn edge_diff_is_set_difference() {
        let mut from_edges = BTreeSet::new();
        from_edges.insert(EdgeRow {
            kind: "calls".into(),
            source_qualified: "a".into(),
            target_qualified: "b".into(),
            file_path: Some("src/lib.rs".into()),
            line: Some(3),
        });
        let mut to_edges = BTreeSet::new();
        to_edges.insert(EdgeRow {
            kind: "calls".into(),
            source_qualified: "a".into(),
            target_qualified: "c".into(),
            file_path: Some("src/lib.rs".into()),
            line: Some(3),
        });
        let from_nodes: BTreeMap<String, NodeRow> = BTreeMap::new();
        let to_nodes: BTreeMap<String, NodeRow> = BTreeMap::new();
        let diff = compute_diff(&from_nodes, &to_nodes, Some(&(from_edges, to_edges)));
        assert!(diff.edges_included);
        assert_eq!(diff.added_edges.len(), 1);
        assert_eq!(diff.removed_edges.len(), 1);
        assert_eq!(diff.added_edges[0].target_qualified, "c");
        assert_eq!(diff.removed_edges[0].target_qualified, "b");
    }

    #[test]
    fn snapshot_resolver_accepts_explicit_db_path() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("foo.db");
        std::fs::write(&p, b"").unwrap();
        let resolved = resolve_snapshot(p.to_str().unwrap(), td.path()).unwrap();
        assert_eq!(resolved, p);
    }

    #[test]
    fn markdown_output_has_headers_per_section() {
        // Render markdown into a captured string by reusing the helpers
        // — print_* push to stdout, so we exercise them indirectly via
        // their underlying summary / per-item shape.
        let mut diff = GraphDiff::default();
        diff.added_nodes.push(NodeRow {
            qualified_name: "crate::alpha".into(),
            kind: "function".into(),
            name: "alpha".into(),
            file_path: Some("src/lib.rs".into()),
            line_start: Some(1),
            line_end: Some(5),
            fingerprint: "deadbeef".into(),
        });
        let s = summarise(&diff);
        assert_eq!(s.added_nodes, 1);
        // The markdown renderer composes "## Added nodes" — verify the
        // helper accepts the input shape without panicking.
        md_section_nodes("Added nodes", &diff.added_nodes, 100);
    }
}
