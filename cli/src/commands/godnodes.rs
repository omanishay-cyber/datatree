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
use crate::commands::ipc_helpers::{
    graph_db_path, resolve_project_root, try_ipc_dispatch, IpcDispatch,
};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::query::GodNode;

/// CLI args for `mneme godnodes`.
#[derive(Debug, Args)]
pub struct GodNodesArgs {
    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// How many to return. Clamped at parse-time to 1..=10000 (REG-022).
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=10000))]
    pub n: u64,
}

/// Entry point used by `main.rs`.
///
/// Dispatch order matches `recall`/`blast`. IPC first, direct-DB fallback.
pub async fn run(args: GodNodesArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project_root = resolve_project_root(args.project.clone());

    // HIGH-48 (2026-05-06, 2026-05-05 audit): consolidated IPC dispatch via
    // cli::ipc_helpers::try_ipc_dispatch. Error arms are shared; success arm
    // is inline here because it is specific to godnodes (GodNodesResults variant).
    let client = make_client(socket_override);
    let req = IpcRequest::GodNodes {
        project: project_root.clone(),
        n: args.n as usize,
    };
    let outcome = try_ipc_dispatch(
        &client,
        req,
        |resp| match resp {
            IpcResponse::GodNodesResults { nodes } => {
                info!(
                    source = "supervisor",
                    count = nodes.len(),
                    "godnodes served"
                );
                print_gods(&nodes);
                Some(Ok(()))
            }
            _ => None,
        },
        |_| false,
    )
    .await?;
    if outcome == IpcDispatch::Done {
        return Ok(());
    }

    // Direct-DB fallback.
    info!(source = "direct-db", "godnodes served");
    let graph_db = graph_db_path(&project_root)?; // HIGH-47 (2026-05-06, 2026-05-05 audit): consolidated to cli::ipc_helpers::graph_db_path
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
    for g in rows.flatten() {
        gods.push(g);
    }

    print_gods(&gods);
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
        args: GodNodesArgs,
    }

    #[test]
    fn print_gods_smoke_empty() {
        // Should not panic on empty input.
        print_gods(&[]);
    }

    #[test]
    fn print_gods_smoke_with_data() {
        let g = GodNode {
            qualified_name: "foo::bar".into(),
            kind: "function".into(),
            name: "bar".into(),
            file_path: Some("src/foo.rs".into()),
            degree: 7,
            fan_in: 3,
            fan_out: 4,
        };
        print_gods(&[g]);
    }

    #[test]
    fn godnodes_args_parse_with_default_n() {
        // Default N is 10 per the clap default_value_t.
        let h = Harness::try_parse_from(["x"]).unwrap();
        assert_eq!(h.args.n, 10);
        assert!(h.args.project.is_none());
    }

    #[test]
    fn godnodes_args_parse_with_explicit_n() {
        let h = Harness::try_parse_from(["x", "--n", "25"]).unwrap();
        assert_eq!(h.args.n, 25);
    }

    #[test]
    fn godnodes_args_parser_rejects_n_zero() {
        // REG-022: n is clamped to 1..=10000; 0 must fail at parse time.
        let r = Harness::try_parse_from(["x", "--n", "0"]);
        assert!(r.is_err(), "n=0 must be rejected at parse time");
    }

    #[test]
    fn godnodes_args_parser_rejects_n_over_max() {
        // REG-022: n>10000 must fail at parse time.
        let r = Harness::try_parse_from(["x", "--n", "10001"]);
        assert!(r.is_err(), "n>10000 must be rejected at parse time");
    }

    #[test]
    fn print_gods_uses_qualified_name_when_name_empty() {
        // When `name` is empty, display_name falls back to qualified_name.
        // We can't capture stdout, but we can exercise the code path.
        let g = GodNode {
            qualified_name: "x::y::z".into(),
            kind: "module".into(),
            name: String::new(),
            file_path: None,
            degree: 2,
            fan_in: 1,
            fan_out: 1,
        };
        print_gods(&[g]);
    }

    #[test]
    fn paths_graph_db_returns_a_pathbuf_for_temp_root() {
        // Helper smoke: graph_db_path (formerly paths_graph_db) should compute
        // a graph.db path for any project root without panicking.
        // HIGH-47 (2026-05-06, 2026-05-05 audit): consolidated to cli::ipc_helpers::graph_db_path
        use crate::commands::ipc_helpers::graph_db_path;
        let td = tempfile::tempdir().unwrap();
        let r = graph_db_path(td.path());
        assert!(r.is_ok(), "graph_db_path unexpectedly errored: {r:?}");
        assert!(
            r.unwrap().to_string_lossy().ends_with("graph.db"),
            "result should end with graph.db"
        );
    }
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
