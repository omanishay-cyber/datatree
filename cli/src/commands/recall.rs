//! `mneme recall <query>` — semantic search across the project graph.
//!
//! v0.3.1 change: queries `graph.db` directly instead of going through the
//! supervisor IPC. The supervisor's `ControlCommand` enum doesn't route a
//! `Recall` verb today (F-009 in the v0.3.0 install report), so any
//! CLI-originated recall hit "unknown variant recall" at the supervisor
//! and failed. The MCP server has always read graph.db directly via
//! `bun:sqlite` — we now do the same from the CLI for parity.
//!
//! Search strategy: prefer FTS5 (`nodes_fts` virtual table, added in v0.3)
//! for speed, fall back to a LIKE scan when the FTS5 table isn't present
//! (older shards). Both paths read-only; no write lock is taken so this
//! is safe to run concurrently with `mneme build`.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

use crate::error::{CliError, CliResult};
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme recall`.
#[derive(Debug, Args)]
pub struct RecallArgs {
    /// Free-form query string. Required.
    pub query: String,

    /// Restrict to one source. For v0.3.1 the only indexed source is
    /// code concepts (nodes) — future layers (decisions, conversation,
    /// todo) will accept this filter. Currently used only to suppress
    /// the default column.
    #[arg(long = "type")]
    pub kind: Option<String>,

    /// Max results to return.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

#[derive(Debug)]
struct Hit {
    kind: String,
    name: String,
    qualified_name: String,
    file_path: Option<String>,
    line_start: Option<i64>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: RecallArgs, _socket_override: Option<PathBuf>) -> CliResult<()> {
    let graph_db = resolve_graph_db(args.project.clone())?;
    if !graph_db.exists() {
        return Err(CliError::Other(format!(
            "graph.db not found at {}. Run `mneme build .` first.",
            graph_db.display()
        )));
    }

    let conn = Connection::open_with_flags(
        &graph_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", graph_db.display())))?;

    // Prefer FTS5 if the virtual table exists; otherwise fall back to LIKE.
    let hits = if has_nodes_fts(&conn)? {
        recall_fts(&conn, &args.query, args.limit)?
    } else {
        recall_like(&conn, &args.query, args.limit)?
    };

    print_hits(&hits, &args.query);
    Ok(())
}

fn resolve_graph_db(project: Option<PathBuf>) -> CliResult<PathBuf> {
    let root = project
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
    let id = ProjectId::from_path(&root)
        .map_err(|e| CliError::Other(format!("cannot hash project path {}: {e}", root.display())))?;
    let paths = PathManager::default_root();
    Ok(paths.project_root(&id).join("graph.db"))
}

fn has_nodes_fts(conn: &Connection) -> CliResult<bool> {
    let mut stmt = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'nodes_fts' LIMIT 1")
        .map_err(|e| CliError::Other(format!("prep fts check: {e}")))?;
    let exists: Option<i64> = stmt
        .query_row([], |row| row.get(0))
        .ok();
    Ok(exists.is_some())
}

/// FTS5 path — fast, ranked by MATCH relevance.
fn recall_fts(conn: &Connection, raw: &str, limit: usize) -> CliResult<Vec<Hit>> {
    // FTS5 is sensitive to punctuation/reserved chars. Sanitize the query
    // by keeping only word characters + spaces; if nothing survives, fall
    // back to LIKE. This mirrors mcp/src/store.ts::fts5Sanitize().
    let sanitized = fts5_sanitize(raw);
    if sanitized.is_empty() {
        return recall_like(conn, raw, limit);
    }

    let sql = "
        SELECT n.kind, n.name, n.qualified_name, n.file_path, n.line_start
        FROM nodes_fts
        JOIN nodes n ON n.rowid = nodes_fts.rowid
        WHERE nodes_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
    ";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep fts recall: {e}")))?;
    let rows = stmt
        .query_map(
            rusqlite::params![sanitized, limit as i64],
            |row| {
                Ok(Hit {
                    kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    file_path: row.get::<_, Option<String>>(3)?,
                    line_start: row.get::<_, Option<i64>>(4)?,
                })
            },
        )
        .map_err(|e| CliError::Other(format!("exec fts recall: {e}")))?;

    let mut hits = Vec::new();
    for r in rows {
        match r {
            Ok(h) => hits.push(h),
            Err(e) => return Err(CliError::Other(format!("row map: {e}"))),
        }
    }
    // If FTS5 returned zero (sanitized query too aggressive, no match),
    // fall back to LIKE so users don't see empty results when a simple
    // substring would match.
    if hits.is_empty() {
        return recall_like(conn, raw, limit);
    }
    Ok(hits)
}

/// LIKE fallback — slow but always works.
fn recall_like(conn: &Connection, query: &str, limit: usize) -> CliResult<Vec<Hit>> {
    let pattern = format!("%{}%", query.replace('%', r"\%").replace('_', r"\_"));
    let sql = "
        SELECT kind, name, qualified_name, file_path, line_start
        FROM nodes
        WHERE name LIKE ?1 ESCAPE '\\' OR qualified_name LIKE ?1 ESCAPE '\\'
        ORDER BY LENGTH(qualified_name) ASC
        LIMIT ?2
    ";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep like recall: {e}")))?;
    let rows = stmt
        .query_map(
            rusqlite::params![pattern, limit as i64],
            |row| {
                Ok(Hit {
                    kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    file_path: row.get::<_, Option<String>>(3)?,
                    line_start: row.get::<_, Option<i64>>(4)?,
                })
            },
        )
        .map_err(|e| CliError::Other(format!("exec like recall: {e}")))?;
    let mut hits = Vec::new();
    for r in rows {
        if let Ok(h) = r {
            hits.push(h);
        }
    }
    Ok(hits)
}

/// Strip anything FTS5 would choke on. Keep alphanumerics + space. Collapse
/// whitespace. Mirrors mcp/src/store.ts `fts5Sanitize` for parity.
fn fts5_sanitize(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    let mut last_was_space = true;
    for c in q.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

fn print_hits(hits: &[Hit], query: &str) {
    if hits.is_empty() {
        println!("no results for `{query}`");
        return;
    }
    println!("{} hit(s) for `{}`:", hits.len(), query);
    println!();
    for h in hits {
        let loc = match (&h.file_path, h.line_start) {
            (Some(f), Some(l)) if l > 0 => format!("{}:{}", f, l),
            (Some(f), _) => f.clone(),
            _ => "-".into(),
        };
        println!("  [{}] {}", h.kind, h.qualified_name);
        if h.name != h.qualified_name {
            println!("      name: {}", h.name);
        }
        println!("      {}", loc);
        println!();
    }
}
