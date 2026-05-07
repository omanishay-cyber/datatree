//! `mneme call-graph <function>` — direct + transitive call graph for a function.
//!
//! v0.4.0 (BENCH-FIX-2.5, 2026-05-07): the MCP tool `call_graph` has
//! existed since v0.1, but there was no CLI counterpart. This module
//! ports the BFS logic from `mcp/src/store.ts::callGraphBfs` into a
//! direct-DB CLI command. Direction picks which edges to walk:
//!
//! - `callees` (default): walk forward — what does this function call?
//! - `callers`: walk backward — who calls this function?
//! - `both`: union of both walks.
//!
//! Direct-DB only — no IPC, no daemon. Read-only against `graph.db`.

use clap::{Args, ValueEnum};
use rusqlite::{params_from_iter, Connection, OpenFlags};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use tracing::info;

use crate::commands::ipc_helpers::{graph_db_path, resolve_project_root};
use crate::error::{CliError, CliResult};

/// Which direction to walk on `kind='calls'` edges.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Direction {
    /// What does this function call? (forward edge walk)
    Callees,
    /// Who calls this function? (reverse edge walk)
    Callers,
    /// Union of both walks.
    Both,
}

impl Direction {
    fn pick_callees(self) -> bool {
        matches!(self, Direction::Callees | Direction::Both)
    }
    fn pick_callers(self) -> bool {
        matches!(self, Direction::Callers | Direction::Both)
    }
}

/// CLI args for `mneme call-graph`.
#[derive(Debug, Args)]
pub struct CallGraphArgs {
    /// Function name to expand. Bare names (`build_or_migrate`) and
    /// fully-qualified paths (`crate::store::DbBuilder::build_or_migrate`)
    /// both work.
    pub function: String,

    /// Walk depth (1 = direct neighbours only). Clamped 1..=32 to keep
    /// large projects' BFS bounded.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u64).range(1..=32))]
    pub depth: u64,

    /// Edge direction. Default both — get the full neighborhood.
    #[arg(long, value_enum, default_value_t = Direction::Both)]
    pub direction: Direction,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// One reachable callable in the graph.
#[derive(Debug, Clone)]
struct CallNode {
    qualified_name: String,
    name: String,
    file: Option<String>,
    line: Option<i64>,
    /// 0 = the seed itself, 1 = direct neighbour, ...
    depth: usize,
    /// Whether this row was reached via callees (forward) or callers (reverse).
    via: &'static str,
}

pub async fn run(args: CallGraphArgs) -> CliResult<()> {
    if args.function.trim().is_empty() {
        return Err(CliError::Other("function must not be empty".to_string()));
    }
    if args.function.contains('\0') {
        return Err(CliError::Other(
            "function contains NUL byte (\\0); remove it and retry".to_string(),
        ));
    }

    let project_root = resolve_project_root(args.project.clone());
    let graph_db = graph_db_path(&project_root)?;
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

    info!(function = %args.function, depth = args.depth, "call-graph: served from direct-DB");

    let nodes = bfs_call_graph(&conn, &args.function, args.direction, args.depth as usize)?;
    print_graph(&args.function, args.direction, &nodes);
    Ok(())
}

/// Resolve `fn_query` to seed qualified_names, then BFS over `edges`
/// (kind='calls') in the requested direction up to `depth` hops.
fn bfs_call_graph(
    conn: &Connection,
    fn_query: &str,
    direction: Direction,
    depth: usize,
) -> CliResult<Vec<CallNode>> {
    // 1. Seed resolution. Same shape as find_references — try every
    //    plausible qualified_name match.
    let seeds: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT qualified_name FROM nodes \
                 WHERE qualified_name = ?1 \
                    OR name = ?1 \
                    OR qualified_name LIKE '%::' || ?1 \
                    OR qualified_name LIKE '%.' || ?1 \
                 LIMIT 50",
            )
            .map_err(|e| CliError::Other(format!("prep seed resolve: {e}")))?;
        let rows = stmt
            .query_map([fn_query], |r| r.get::<_, String>(0))
            .map_err(|e| CliError::Other(format!("exec seed resolve: {e}")))?;
        let mut out: Vec<String> = rows.flatten().collect();
        // If nothing matched, seed with the raw input — BFS will return
        // empty but at least the user knows it tried.
        if out.is_empty() {
            out.push(fn_query.to_string());
        }
        out
    };

    let mut visited: HashSet<String> = seeds.iter().cloned().collect();
    let mut frontier: VecDeque<(String, usize)> =
        seeds.iter().map(|s| (s.clone(), 0_usize)).collect();
    // Track every visited node + its first-reach depth + via-direction
    // for printing.
    let mut visited_meta: Vec<CallNode> = Vec::with_capacity(seeds.len());
    for s in &seeds {
        visited_meta.push(CallNode {
            qualified_name: s.clone(),
            name: s.clone(),
            file: None,
            line: None,
            depth: 0,
            via: "seed",
        });
    }

    // Prepared statements outlive the loop so the SQLite parser only
    // runs once per direction.
    let mut callees_stmt = conn
        .prepare(
            "SELECT DISTINCT target_qualified \
             FROM edges WHERE kind = 'calls' AND source_qualified = ?1",
        )
        .map_err(|e| CliError::Other(format!("prep callees: {e}")))?;
    let mut callers_stmt = conn
        .prepare(
            "SELECT DISTINCT source_qualified \
             FROM edges WHERE kind = 'calls' AND target_qualified = ?1",
        )
        .map_err(|e| CliError::Other(format!("prep callers: {e}")))?;

    while let Some((node, d)) = frontier.pop_front() {
        if d >= depth {
            continue;
        }
        if direction.pick_callees() {
            let rows = callees_stmt
                .query_map([&node], |r| r.get::<_, String>(0))
                .map_err(|e| CliError::Other(format!("exec callees: {e}")))?;
            for r in rows.flatten() {
                if visited.insert(r.clone()) {
                    visited_meta.push(CallNode {
                        qualified_name: r.clone(),
                        name: r.clone(),
                        file: None,
                        line: None,
                        depth: d + 1,
                        via: "callee",
                    });
                    frontier.push_back((r, d + 1));
                }
            }
        }
        if direction.pick_callers() {
            let rows = callers_stmt
                .query_map([&node], |r| r.get::<_, String>(0))
                .map_err(|e| CliError::Other(format!("exec callers: {e}")))?;
            for r in rows.flatten() {
                if visited.insert(r.clone()) {
                    visited_meta.push(CallNode {
                        qualified_name: r.clone(),
                        name: r.clone(),
                        file: None,
                        line: None,
                        depth: d + 1,
                        via: "caller",
                    });
                    frontier.push_back((r, d + 1));
                }
            }
        }
    }

    // 2. Enrich each node with name + file + line from the nodes table.
    //    Single-row lookup — graph.db has UNIQUE on qualified_name so
    //    each is O(log N).
    drop(callees_stmt);
    drop(callers_stmt);
    let mut enrich_stmt = conn
        .prepare(
            "SELECT name, file_path, line_start FROM nodes \
             WHERE qualified_name = ?1 LIMIT 1",
        )
        .map_err(|e| CliError::Other(format!("prep enrich: {e}")))?;

    // Avoid mutating visited_meta in place during iteration. Build new
    // vec instead.
    let mut enriched: Vec<CallNode> = Vec::with_capacity(visited_meta.len());
    for n in visited_meta {
        let row: rusqlite::Result<(String, Option<String>, Option<i64>)> =
            enrich_stmt.query_row(params_from_iter([&n.qualified_name]), |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get(1)?,
                    r.get(2)?,
                ))
            });
        match row {
            Ok((name, file, line)) => enriched.push(CallNode {
                name: if name.is_empty() {
                    n.name.clone()
                } else {
                    name
                },
                file,
                line,
                ..n
            }),
            Err(_) => enriched.push(n),
        }
    }
    Ok(enriched)
}

fn print_graph(query: &str, direction: Direction, nodes: &[CallNode]) {
    let dir_label = match direction {
        Direction::Callees => "callees",
        Direction::Callers => "callers",
        Direction::Both => "both directions",
    };
    let total = nodes.len().saturating_sub(1); // subtract the seed itself
    if total == 0 {
        println!("call graph for `{query}` — no calls found ({dir_label})");
        return;
    }
    println!("call graph for `{query}` — {total} reachable callable(s) ({dir_label})");
    println!();
    // Group by depth so the BFS shape is visible.
    let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    for d in 0..=max_depth {
        let layer: Vec<&CallNode> = nodes.iter().filter(|n| n.depth == d).collect();
        if layer.is_empty() {
            continue;
        }
        if d == 0 {
            println!("seed:");
        } else {
            println!("depth {d}:");
        }
        for n in layer {
            // Bug #38: strip Windows long-path prefix at display boundary.
            let loc = match (&n.file, n.line) {
                (Some(f), Some(l)) if l > 0 => format!("{}:{l}", super::display_path(f)),
                (Some(f), _) => super::display_path(f).to_string(),
                _ => "-".into(),
            };
            let label = if n.name == n.qualified_name {
                n.qualified_name.clone()
            } else {
                format!("{} ({})", n.name, n.qualified_name)
            };
            if n.via == "seed" {
                println!("  {label} @ {loc}");
            } else {
                println!("  [{}] {} @ {}", n.via, label, loc);
            }
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: CallGraphArgs,
    }

    #[tokio::test]
    async fn empty_function_is_rejected() {
        let args = CallGraphArgs {
            function: "  ".to_string(),
            depth: 3,
            direction: Direction::Both,
            project: None,
        };
        let r = run(args).await;
        match r {
            Err(CliError::Other(msg)) => {
                assert!(msg.contains("function must not be empty"), "wrong: {msg}")
            }
            other => panic!("expected Err(empty), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn nul_byte_in_function_is_rejected() {
        let args = CallGraphArgs {
            function: "foo\0bar".to_string(),
            depth: 3,
            direction: Direction::Both,
            project: None,
        };
        let r = run(args).await;
        match r {
            Err(CliError::Other(msg)) => assert!(msg.contains("NUL"), "wrong: {msg}"),
            other => panic!("expected Err(NUL), got {other:?}"),
        }
    }

    #[test]
    fn args_parse_with_just_function() {
        let h = Harness::try_parse_from(["x", "build_or_migrate"]).unwrap();
        assert_eq!(h.args.function, "build_or_migrate");
        assert_eq!(h.args.depth, 3);
    }

    #[test]
    fn args_parse_with_full_flags() {
        let h = Harness::try_parse_from([
            "x",
            "foo",
            "--depth",
            "5",
            "--direction",
            "callees",
            "--project",
            "/tmp/p",
        ])
        .unwrap();
        assert_eq!(h.args.depth, 5);
        assert!(matches!(h.args.direction, Direction::Callees));
        assert_eq!(h.args.project, Some(PathBuf::from("/tmp/p")));
    }

    #[test]
    fn args_parser_rejects_zero_depth() {
        assert!(Harness::try_parse_from(["x", "foo", "--depth", "0"]).is_err());
    }

    #[test]
    fn args_parser_rejects_huge_depth() {
        assert!(Harness::try_parse_from(["x", "foo", "--depth", "33"]).is_err());
    }

    #[test]
    fn direction_pickers() {
        assert!(Direction::Callees.pick_callees());
        assert!(!Direction::Callees.pick_callers());
        assert!(Direction::Callers.pick_callers());
        assert!(!Direction::Callers.pick_callees());
        assert!(Direction::Both.pick_callees());
        assert!(Direction::Both.pick_callers());
    }

    #[test]
    fn print_graph_handles_empty_seed_only() {
        // smoke: node with only a seed should print "no calls found"
        let nodes = vec![CallNode {
            qualified_name: "foo".to_string(),
            name: "foo".to_string(),
            file: None,
            line: None,
            depth: 0,
            via: "seed",
        }];
        print_graph("foo", Direction::Both, &nodes);
    }
}
