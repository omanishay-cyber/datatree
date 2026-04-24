//! `mneme blast <target> [--depth=N]` — blast radius lookup.
//!
//! v0.3.1: dual-path dispatch, same shape as `recall`. Supervisor-up
//! prefers the IPC hop; supervisor-down falls back to the in-process BFS
//! over `graph.db`. Both paths operate on the same shard path derived
//! from `PathManager` so results are identical.
//!
//! Algorithm (direct-DB path): BFS over the `edges` table, starting from
//! any node whose `qualified_name` or `name` matches `target`. Returns
//! the set of reachable node qualified_names up to `depth` hops.
//! Direction is reverse ("who depends on me") because that's the
//! blast-radius question users actually ask.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use tracing::info;

use crate::commands::build::make_client;
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::query::BlastItem;
use common::{ids::ProjectId, paths::PathManager};

/// CLI args for `mneme blast`.
#[derive(Debug, Args)]
pub struct BlastArgs {
    /// File path or fully-qualified function name (e.g. `src/auth.ts:login`
    /// or just a bare name like `authenticate`).
    pub target: String,

    /// Max traversal depth. 1 = direct dependents only.
    #[arg(long, default_value_t = 2)]
    pub depth: usize,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
///
/// Dispatch order matches `recall`: IPC when the supervisor is up, with
/// automatic fallback to the in-process BFS when IPC is unreachable or
/// returns an IO-level error.
pub async fn run(args: BlastArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project_root = resolve_project_root(args.project.clone());

    let client = make_client(socket_override);
    if client.is_running().await {
        let req = IpcRequest::Blast {
            project: project_root.clone(),
            target: args.target.clone(),
            depth: args.depth,
        };
        match client.request(req).await {
            Ok(IpcResponse::BlastResults { impacted }) => {
                info!(source = "supervisor", count = impacted.len(), "blast served");
                print_layers_from_items(&args.target, &impacted, args.depth);
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

    // Direct-DB fallback — identical to the v0.3.1 behaviour.
    info!(source = "direct-db", "blast served");
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

    // Resolve target to one or more starting node qualified_names.
    let starts = resolve_target(&conn, &args.target)?;
    if starts.is_empty() {
        println!("no node matches target `{}`", args.target);
        return Ok(());
    }

    // BFS in the reverse direction: who points AT me.
    let mut visited: HashSet<String> = starts.iter().cloned().collect();
    let mut frontier: VecDeque<(String, usize)> =
        starts.iter().map(|s| (s.clone(), 0)).collect();
    let mut layers: Vec<Vec<String>> = vec![starts.clone()];
    for _ in 0..args.depth {
        layers.push(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT source_qualified FROM edges WHERE target_qualified = ?1
             UNION
             SELECT source_qualified FROM edges WHERE target_qualified IN (
               SELECT qualified_name FROM nodes WHERE name = ?1
             )",
        )
        .map_err(|e| CliError::Other(format!("prep blast query: {e}")))?;

    while let Some((node, d)) = frontier.pop_front() {
        if d >= args.depth {
            continue;
        }
        let rows = stmt
            .query_map(rusqlite::params![node], |row| {
                row.get::<_, Option<String>>(0)
            })
            .map_err(|e| CliError::Other(format!("exec blast query: {e}")))?;

        for r in rows {
            if let Ok(Some(src)) = r {
                if visited.insert(src.clone()) {
                    frontier.push_back((src.clone(), d + 1));
                    if let Some(layer) = layers.get_mut(d + 1) {
                        layer.push(src);
                    }
                }
            }
        }
    }

    print_layers(&args.target, &layers);
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

/// Map a resolved project root to its `graph.db` path. Uses the same
/// `PathManager`/`ProjectId` chain the CLI always has — supervisor path
/// derives the same location so shard selection cannot drift.
fn paths_graph_db(root: &std::path::Path) -> CliResult<PathBuf> {
    let id = ProjectId::from_path(root)
        .map_err(|e| CliError::Other(format!("cannot hash project path {}: {e}", root.display())))?;
    let paths = PathManager::default_root();
    Ok(paths.project_root(&id).join("graph.db"))
}

/// Render an IPC `BlastResults` into the same layered output the direct-DB
/// path produces. Supervisor-side BFS emits a flat `Vec<BlastItem>` with
/// per-item `depth` so we can reconstruct the layered presentation here
/// without the CLI having to track BFS state itself.
fn print_layers_from_items(target: &str, items: &[BlastItem], max_depth: usize) {
    let mut layers: Vec<Vec<String>> = vec![Vec::new()];
    for _ in 0..max_depth {
        layers.push(Vec::new());
    }
    for it in items {
        if let Some(layer) = layers.get_mut(it.depth) {
            layer.push(it.qualified_name.clone());
        }
    }
    print_layers(target, &layers);
}

/// Resolve a target string to one or more starting node qualified_names.
/// Matches both `qualified_name` (exact) and `name` (exact, case-sensitive).
fn resolve_target(conn: &Connection, target: &str) -> CliResult<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT qualified_name FROM nodes
             WHERE qualified_name = ?1 OR name = ?1 OR file_path = ?1
             ORDER BY CASE
               WHEN qualified_name = ?1 THEN 0
               WHEN name = ?1 THEN 1
               ELSE 2
             END
             LIMIT 10",
        )
        .map_err(|e| CliError::Other(format!("prep target resolve: {e}")))?;
    let rows = stmt
        .query_map(rusqlite::params![target], |row| {
            row.get::<_, Option<String>>(0)
        })
        .map_err(|e| CliError::Other(format!("exec target resolve: {e}")))?;

    let mut out = Vec::new();
    for r in rows {
        if let Ok(Some(q)) = r {
            out.push(q);
        }
    }
    Ok(out)
}

fn print_layers(target: &str, layers: &[Vec<String>]) {
    let total: usize = layers.iter().skip(1).map(|l| l.len()).sum();
    println!("blast radius for `{target}` — {total} dependent(s)");
    println!();
    for (depth, layer) in layers.iter().enumerate() {
        if depth == 0 {
            // Depth 0 is the target itself — already shown in header.
            continue;
        }
        if layer.is_empty() {
            continue;
        }
        println!("depth {depth}: {} dependent(s)", layer.len());
        for q in layer {
            println!("  {q}");
        }
        println!();
    }
}
