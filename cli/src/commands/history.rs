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

    /// Unix ms lower bound (or 0 to skip).
    #[arg(long)]
    pub since: Option<i64>,

    /// Max results to return. Clamped at parse-time to 1..=10000 (REG-022).
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u64).range(1..=10000))]
    pub limit: u64,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

#[derive(Debug)]
struct HistoryRow {
    timestamp_ms: i64,
    kind: String,
    summary: String,
    rationale: Option<String>,
}

/// Entry point used by `main.rs`.
///
/// WIRE-012: history is a direct-DB-only command — there is no IPC verb
/// for ledger search yet (slated for v0.4 supervisor work). The
/// `_socket_override` parameter has been removed; if/when the supervisor
/// gains a `History` request, re-add it (and threadthrough from main.rs).
pub async fn run(args: HistoryArgs) -> CliResult<()> {
    let project = args
        .project
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
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

    // Schema (store/src/schema.rs §ledger_entries):
    //   id, session_id, timestamp (INTEGER unix ms), kind, summary, rationale, …
    let like = format!("%{}%", args.query.replace('%', r"\%").replace('_', r"\_"));
    let base = "SELECT timestamp, kind, summary, rationale FROM ledger_entries \
         WHERE (summary LIKE ?1 ESCAPE '\\' OR rationale LIKE ?1 ESCAPE '\\' OR kind LIKE ?1 ESCAPE '\\')";
    let rows_iter: Box<dyn Iterator<Item = _>> = if let Some(since) = args.since {
        let sql = format!("{base} AND timestamp >= ?2 ORDER BY timestamp DESC LIMIT ?3");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| CliError::Other(format!("prep: {e}")))?;
        let mapped = stmt
            .query_map(rusqlite::params![like, since, args.limit as i64], |row| {
                Ok(HistoryRow {
                    timestamp_ms: row.get::<_, i64>(0)?,
                    kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    summary: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    rationale: row.get::<_, Option<String>>(3)?,
                })
            })
            .map_err(|e| CliError::Other(format!("exec: {e}")))?;
        Box::new(
            mapped
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>()
                .into_iter(),
        )
    } else {
        let sql = format!("{base} ORDER BY timestamp DESC LIMIT ?2");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| CliError::Other(format!("prep: {e}")))?;
        let mapped = stmt
            .query_map(rusqlite::params![like, args.limit as i64], |row| {
                Ok(HistoryRow {
                    timestamp_ms: row.get::<_, i64>(0)?,
                    kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    summary: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    rationale: row.get::<_, Option<String>>(3)?,
                })
            })
            .map_err(|e| CliError::Other(format!("exec: {e}")))?;
        Box::new(
            mapped
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>()
                .into_iter(),
        )
    };

    let mut shown = 0usize;
    for r in rows_iter {
        if shown == 0 {
            println!("history hits for `{}`:", args.query);
            println!();
        }
        println!("  [{}] {}", r.kind, format_ms_utc(r.timestamp_ms));
        let snippet: String = r.summary.chars().take(140).collect();
        println!("    {snippet}");
        if let Some(rat) = r.rationale.as_deref() {
            let rat_snip: String = rat.chars().take(140).collect();
            if !rat_snip.is_empty() {
                println!("    (rationale: {rat_snip})");
            }
        }
        println!();
        shown += 1;
    }
    if shown == 0 {
        println!("no history entries match `{}`", args.query);
    }
    Ok(())
}

/// Format a unix-millis timestamp as `YYYY-MM-DD HH:MM:SS UTC`.
fn format_ms_utc(ms: i64) -> String {
    let secs = (ms / 1000).max(0) as u64;
    let days = (secs / 86_400) as i64;
    let s = secs % 86_400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    let (y, mo, d) = ymd(days);
    format!("{y:04}-{mo:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}
fn ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 {
        (mp + 3) as u32
    } else {
        (mp - 9) as u32
    };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_ms_utc_epoch_zero() {
        assert_eq!(format_ms_utc(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn format_ms_utc_known_timestamp() {
        // 2021-01-01 00:00:00 UTC = 1_609_459_200_000 ms
        assert_eq!(format_ms_utc(1_609_459_200_000), "2021-01-01 00:00:00 UTC");
    }

    #[tokio::test]
    async fn run_with_missing_db_prints_clean_message() {
        // Use a tempdir where the shard cannot exist; run() must NOT
        // panic and should return Ok with the "no history recorded yet"
        // path. (We can't intercept stdout from this test harness, but
        // we can verify the function exits 0.)
        let td = tempfile::tempdir().unwrap();
        let args = HistoryArgs {
            query: "anything".into(),
            since: None,
            limit: 10,
            project: Some(td.path().to_path_buf()),
        };
        let r = run(args).await;
        assert!(r.is_ok(), "expected Ok, got {r:?}");
    }
}
