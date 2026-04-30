//! `mneme status` — graph stats, drift findings count, last build time.
//!
//! ## WIRE-010: direct-DB fallback
//!
//! When the supervisor is reachable we delegate (it has the freshest
//! per-child snapshots). When it is NOT reachable we fall back to a
//! direct read of `meta.db::projects` so `mneme status` still tells
//! the user something useful (project list + last_indexed_at), in the
//! same spirit as `history.rs`.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::paths::PathManager;

/// CLI args for `mneme status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Optional project path. Defaults to CWD.
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: StatusArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let attempt = client
        .request(IpcRequest::Status {
            project: args.project.clone(),
        })
        .await;

    match attempt {
        Ok(IpcResponse::Error { message }) => {
            tracing::warn!(
                error = %message,
                "supervisor returned error on status; falling back to direct-db"
            );
        }
        Ok(resp) => return handle_response(resp),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "supervisor unreachable on status; falling back to direct-db"
            );
        }
    }

    direct_db_fallback()
}

fn direct_db_fallback() -> CliResult<()> {
    let paths = PathManager::default_root();
    let meta_db = paths.meta_db();
    if !meta_db.exists() {
        println!("status: supervisor unreachable + meta.db not found");
        println!("  (no projects have been built yet — run `mneme build .`)");
        return Ok(());
    }

    let conn = Connection::open_with_flags(
        &meta_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", meta_db.display())))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, root, last_indexed_at FROM projects ORDER BY last_indexed_at DESC NULLS LAST",
        )
        .map_err(|e| CliError::Other(format!("prep status: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| CliError::Other(format!("exec status: {e}")))?;

    let mut shown = 0usize;
    println!("status: supervisor unreachable — direct-db summary from {}:", meta_db.display());
    println!();
    for (id, name, root, last_indexed_at) in rows.flatten() {
        shown += 1;
        let id_short: String = id.chars().take(12).collect();
        println!("  [{id_short}] {}", name.unwrap_or_else(|| "<unnamed>".into()));
        if let Some(p) = root {
            println!("      root: {p}");
        }
        println!(
            "      last_indexed_at: {}",
            last_indexed_at.unwrap_or_else(|| "<never>".into())
        );
        println!();
    }
    if shown == 0 {
        println!("  (no projects in meta.db)");
    } else {
        println!("{shown} project(s) tracked.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Smoke clap harness — verify args parser without spinning up the
    /// full binary.
    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: StatusArgs,
    }

    #[tokio::test]
    async fn status_with_no_supervisor_falls_through_cleanly() {
        // Point at a socket that absolutely doesn't exist; the fallback
        // path should run, not error. (`meta.db` may or may not exist
        // on this machine — both branches are valid.)
        let args = StatusArgs { project: None };
        let r = run(args, Some(PathBuf::from("/nope-mneme-supervisor.sock"))).await;
        // We accept Ok in both cases (db missing OR present).
        assert!(r.is_ok(), "expected Ok from fallback path, got: {r:?}");
    }

    #[test]
    fn status_args_parse_with_no_args() {
        // `mneme status` with no args — project field defaults to None
        // (resolved at runtime to CWD by PathManager).
        let h = Harness::try_parse_from(["x"]).unwrap();
        assert!(h.args.project.is_none());
    }

    #[test]
    fn status_args_parse_with_explicit_project() {
        // `mneme status /tmp/proj` — positional argument is captured.
        let h = Harness::try_parse_from(["x", "/tmp/proj"]).unwrap();
        assert!(h.args.project.is_some());
        assert_eq!(
            h.args.project.as_ref().unwrap(),
            &PathBuf::from("/tmp/proj")
        );
    }
}
