//! `mneme blast <target> [--depth=N]` — blast radius lookup.
//!
//! v0.3.1 change: queries `graph.db` directly (same rationale as recall.rs
//! — the supervisor IPC doesn't route a Blast verb today, F-009).
//!
//! Algorithm: BFS over the `edges` table, starting from any node whose
//! `qualified_name` or `name` matches `target`. Returns the set of
//! reachable node qualified_names up to `depth` hops. Direction is
//! reverse ("who depends on me") because that's the blast-radius
//! question users actually ask.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

use crate::error::{CliError, CliResult};
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
pub async fn run(args: BlastArgs, _socket_override: Option<PathBuf>) -> CliResult<()> {
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
