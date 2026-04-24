//! `mneme snap` — manual snapshot of the active shard.
//!
//! v0.3.1: direct-DB path. Uses SQLite's `VACUUM INTO` to produce a
//! consistent point-in-time copy of `graph.db` into
//! `~/.mneme/snapshots/<project>/<YYYYMMDD-HHMMSS>.db`. No supervisor
//! round-trip; works even when the daemon is down. Idempotent — each
//! call creates a new timestamped snapshot, never overwrites an
//! existing one.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

use crate::error::{CliError, CliResult};
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme snap`.
#[derive(Debug, Args)]
pub struct SnapArgs {
    /// Optional project path. Defaults to CWD.
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: SnapArgs, _socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = args.project.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    let project = std::fs::canonicalize(&project).unwrap_or(project);

    let id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let graph_db = paths.project_root(&id).join("graph.db");
    if !graph_db.exists() {
        return Err(CliError::Other(format!(
            "graph.db not found at {}. Run `mneme build .` first.",
            graph_db.display()
        )));
    }

    let home = dirs::home_dir()
        .ok_or_else(|| CliError::Other("cannot locate home dir".into()))?;
    let snap_dir = home.join(".mneme").join("snapshots").join(id.to_string());
    std::fs::create_dir_all(&snap_dir).map_err(|e| CliError::io(&snap_dir, e))?;

    let stamp = utc_stamp();
    let snap_path = snap_dir.join(format!("{stamp}.db"));

    let conn = Connection::open_with_flags(
        &graph_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", graph_db.display())))?;

    // VACUUM INTO runs atomically against the source; safe to run
    // concurrently with an active daemon since we opened read-only.
    conn.execute(
        &format!(
            "VACUUM INTO '{}'",
            snap_path.display().to_string().replace('\'', "''")
        ),
        [],
    )
    .map_err(|e| CliError::Other(format!("VACUUM INTO: {e}")))?;

    let size = std::fs::metadata(&snap_path)
        .map(|m| m.len())
        .unwrap_or(0);
    println!("snapshot created:");
    println!("  source: {}", graph_db.display());
    println!("  target: {}", snap_path.display());
    println!("  size:   {:.1} MB", size as f64 / 1_048_576.0);
    Ok(())
}

/// `YYYYMMDD-HHMMSS` UTC without pulling chrono.
fn utc_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let s = secs % 86_400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    let (y, m, d) = ymd(days);
    format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}")
}
fn ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z / 146_097 } else { (z - 146_096) / 146_097 };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
