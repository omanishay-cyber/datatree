//! `mneme godnodes [--n=N]` — top-N most-connected concepts.
//!
//! v0.3.1: dual-path dispatch, same shape as `recall` and `blast`. IPC
//! first (supervisor pools the read connection + prepared statement);
//! direct-DB fallback preserved verbatim when the daemon is down.
//!
//! "Most connected" = highest in-degree + out-degree across the `edges`
//! table. These are the project's god-objects, central utilities, and
//! structural chokepoints. Useful for: architectural review, refactor
//! risk assessment, and understanding what load-bearing code looks like.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use tracing::info;

use crate::commands::build::make_client;
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::query::GodNode;
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme godnodes`.
#[derive(Debug, Args)]
pub struct GodNodesArgs {
    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// How many to return.
    #[arg(long, default_value_t = 10)]
    pub n: usize,
}

/// Entry point used by `main.rs`.
///
/// Dispatch order matches `recall`/`blast`. IPC first, direct-DB fallback.
pub async fn run(args: GodNodesArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project_root = resolve_project_root(args.project.clone());

    let client = make_client(socket_override);
    if client.is_running().await {
        let req = IpcRequest::GodNodes {
            project: project_root.clone(),
            n: args.n,
        };
        match client.request(req).await {
            Ok(IpcResponse::GodNodesResults { nodes }) => {
                info!(source = "supervisor", count = nodes.len(), "godnodes served");
                print_gods(&nodes);
                return Ok(());
            }
            Ok(IpcResponse::Error { message }) => {
                return Err(CliError::Supervisor(message));
            }
            Ok(other) => {
                tracing::warn!(?other, "unexpected IPC response; falling back to direct-db");
            }
            Err(CliError::Ipc(msg)) => {
                tracing::warn!(error = %msg, "supervisor IPC failed; falling back to direct-db");
            }
            Err(e) => return Err(e),
        }
    }

    // Direct-DB fallback.
    info!(source = "direct-db", "godnodes served");
    let graph_db = paths_graph_db(&project_root)?;
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

    let sql = "
        WITH degrees AS (
            SELECT qn, SUM(fan_in) AS fan_in, SUM(fan_out) AS fan_out
            FROM (
                SELECT target_qualified AS qn, 1 AS fan_in, 0 AS fan_out FROM edges
                UNION ALL
                SELECT source_qualified AS qn, 0 AS fan_in, 1 AS fan_out FROM edges
            )
            GROUP BY qn
        )
        SELECT n.qualified_name, n.kind, n.name, n.file_path,
               (d.fan_in + d.fan_out) AS degree, d.fan_in, d.fan_out
        FROM degrees d
        JOIN nodes n ON n.qualified_name = d.qn
        ORDER BY degree DESC, n.qualified_name ASC
        LIMIT ?1
    ";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep godnodes: {e}")))?;
    let rows = stmt
        .query_map(rusqlite::params![args.n as i64], |row| {
            Ok(GodNode {
                qualified_name: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                file_path: row.get::<_, Option<String>>(3)?,
                degree: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                fan_in: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                fan_out: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            })
        })
        .map_err(|e| CliError::Other(format!("exec godnodes: {e}")))?;

    let mut gods = Vec::new();
    for r in rows {
        if let Ok(g) = r {
            gods.push(g);
        }
    }

    print_gods(&gods);
    Ok(())
}

/// Canonicalise the user's `--project` flag (or CWD) to an absolute path.
fn resolve_project_root(project: Option<PathBuf>) -> PathBuf {
    project
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
}

/// Map a resolved project root to its `graph.db` path.
fn paths_graph_db(root: &std::path::Path) -> CliResult<PathBuf> {
    let id = ProjectId::from_path(root)
        .map_err(|e| CliError::Other(format!("cannot hash project path {}: {e}", root.display())))?;
    let paths = PathManager::default_root();
    Ok(paths.project_root(&id).join("graph.db"))
}

fn print_gods(gods: &[GodNode]) {
    if gods.is_empty() {
        println!("no edges in graph.db — run `mneme build .` first");
        return;
    }
    println!("top {} most-connected concept(s):", gods.len());
    println!();
    for (i, g) in gods.iter().enumerate() {
        let loc = g.file_path.clone().unwrap_or_else(|| "-".into());
        let display_name = if g.name.is_empty() {
            g.qualified_name.clone()
        } else {
            g.name.clone()
        };
        println!(
            "  {rank:>2}. [{kind}] {name}",
            rank = i + 1,
            kind = g.kind,
            name = display_name,
        );
        if display_name != g.qualified_name {
            println!("      qn:     {}", g.qualified_name);
        }
        println!(
            "      degree: {}  (in={} out={})",
            g.degree, g.fan_in, g.fan_out
        );
        println!("      file:   {}", loc);
        println!();
    }
}
