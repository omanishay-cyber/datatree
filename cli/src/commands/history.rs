//! `mneme history <query>` — search the conversation / ledger history.
//!
//! v0.3.1: direct-DB path. Queries the step ledger (`tasks.db`) and
//! any session transcripts recorded under the project's shard. When
//! the shard doesn't yet have history rows (fresh `mneme build`) we
//! print a clean empty result instead of failing.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

use crate::error::{CliError, CliResult};
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme history`.
#[derive(Debug, Args)]
pub struct HistoryArgs {
    /// Free-form query.
    pub query: String,

    /// ISO-8601 lower bound (e.g. `2026-04-01T00:00:00Z`).
    #[arg(long)]
    pub since: Option<String>,

    /// Max results to return.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

#[derive(Debug)]
struct HistoryRow {
    created_at: String,
    kind: String,
    body: String,
}

/// Entry point used by `main.rs`.
pub async fn run(args: HistoryArgs, _socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = args.project.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    let project = std::fs::canonicalize(&project).unwrap_or(project);

    let id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let tasks_db = paths.project_root(&id).join("tasks.db");

    if !tasks_db.exists() {
        println!("no history recorded yet for this project");
        println!("  (tasks.db not found at {})", tasks_db.display());
        return Ok(());
    }

    let conn = Connection::open_with_flags(
        &tasks_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", tasks_db.display())))?;

    // Graceful empty result if the ledger table doesn't exist (fresh shard).
    let table_ok: Option<i64> = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='ledger_entries' LIMIT 1")
        .and_then(|mut s| s.query_row([], |row| row.get(0)))
        .ok();
    if table_ok.is_none() {
        println!("no history entries yet");
        return Ok(());
    }

    let like = format!("%{}%", args.query.replace('%', r"\%").replace('_', r"\_"));
    let since_clause = match args.since.as_deref() {
        Some(_) => "AND created_at >= ?2",
        None => "",
    };
    let sql = format!(
        "SELECT created_at, kind, body FROM ledger_entries \
         WHERE (body LIKE ?1 ESCAPE '\\' OR kind LIKE ?1 ESCAPE '\\') {since_clause} \
         ORDER BY created_at DESC LIMIT ?3"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CliError::Other(format!("prep: {e}")))?;
    let rows_iter: Box<dyn Iterator<Item = _>> = if let Some(since) = args.since.as_deref() {
        let mapped = stmt
            .query_map(rusqlite::params![like, since, args.limit as i64], |row| {
                Ok(HistoryRow {
                    created_at: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    body: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            })
            .map_err(|e| CliError::Other(format!("exec: {e}")))?;
        Box::new(mapped.filter_map(|r| r.ok()).collect::<Vec<_>>().into_iter())
    } else {
        let mapped = stmt
            .query_map(rusqlite::params![like, args.limit as i64], |row| {
                Ok(HistoryRow {
                    created_at: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    body: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            })
            .map_err(|e| CliError::Other(format!("exec: {e}")))?;
        Box::new(mapped.filter_map(|r| r.ok()).collect::<Vec<_>>().into_iter())
    };

    let mut shown = 0usize;
    for r in rows_iter {
        if shown == 0 {
            println!("history hits for `{}`:", args.query);
            println!();
        }
        println!("  [{}] {}", r.kind, r.created_at);
        let snippet: String = r.body.chars().take(140).collect();
        println!("    {snippet}");
        println!();
        shown += 1;
    }
    if shown == 0 {
        println!("no history entries match `{}`", args.query);
    }
    Ok(())
}
