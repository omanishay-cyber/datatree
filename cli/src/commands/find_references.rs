//! `mneme find-references <symbol>` — list every reference (def + callers + imports)
//! for a symbol across the project.
//!
//! v0.4.0 (BENCH-FIX-2.5, 2026-05-07): the MCP tool `find_references` has
//! existed since v0.1, but there was no CLI counterpart — users typing
//! `mneme find_references X` got `unrecognized subcommand`. This module
//! ports the same query shape from `mcp/src/store.ts::findReferences`
//! into a direct-DB CLI command so terminal users get the same surface.
//!
//! Direct-DB only: no IPC roundtrip, no daemon dependency. Read-only
//! against `graph.db`. Mirrors `blast.rs` and `recall.rs` for path
//! resolution and output formatting.

use clap::Args;
use rusqlite::{params_from_iter, Connection, OpenFlags};
use std::path::PathBuf;
use tracing::info;

use crate::commands::ipc_helpers::{graph_db_path, resolve_project_root};
use crate::error::{CliError, CliResult};

/// CLI args for `mneme find-references`.
#[derive(Debug, Args)]
pub struct FindReferencesArgs {
    /// Symbol name to look up. Accepts bare names (`PathManager`),
    /// fully-qualified paths (`crate::manager::PathManager`), or any
    /// suffix that ends in `::Foo` / `.Foo` — the resolver tries all
    /// shapes and unions the matches.
    pub symbol: String,

    /// Max results to return. Default 50; clamped 1..=10000.
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u64).range(1..=10000))]
    pub limit: u64,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// One reference hit returned to the caller.
#[derive(Debug, Clone)]
struct RefHit {
    /// `definition` | `call` | `import` | `usage`
    kind: String,
    /// The qualified_name of the source (where the reference lives).
    source: String,
    /// Source file path.
    file: Option<String>,
    /// Source line number.
    line: Option<i64>,
    /// Best-effort context line — signature for definitions, name for usages.
    context: String,
}

/// Map raw edge `kind` strings to user-friendly labels matching the MCP tool.
fn map_kind(k: &str) -> &'static str {
    match k {
        "definition" => "definition",
        "calls" | "call" => "call",
        "imports" | "import" => "import",
        _ => "usage",
    }
}

pub async fn run(args: FindReferencesArgs) -> CliResult<()> {
    if args.symbol.trim().is_empty() {
        return Err(CliError::Other("symbol must not be empty".to_string()));
    }
    if args.symbol.contains('\0') {
        return Err(CliError::Other(
            "symbol contains NUL byte (\\0) — SQLite would truncate the search; remove the NUL"
                .to_string(),
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

    info!(symbol = %args.symbol, "find-references: served from direct-DB");

    let hits = find_references(&conn, &args.symbol, args.limit as usize)?;
    print_hits(&args.symbol, &hits);
    Ok(())
}

/// Resolve the symbol to one or more qualified_name targets, then union
/// the edge-target lookup with the node-definition lookup. Mirrors
/// `mcp/src/store.ts::findReferences` query-by-query.
fn find_references(conn: &Connection, symbol: &str, limit: usize) -> CliResult<Vec<RefHit>> {
    // 1. Resolve all qualified_name candidates. Bare names ("Store") are
    //    rare in graph.db where everything is fully-qualified
    //    ("crate::manager::Store"), so we try every plausible shape.
    let target_rows: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT qualified_name FROM nodes \
                 WHERE qualified_name = ?1 \
                    OR name = ?1 \
                    OR qualified_name LIKE '%::' || ?1 \
                    OR qualified_name LIKE '%.' || ?1 \
                 LIMIT 200",
            )
            .map_err(|e| CliError::Other(format!("prep target resolve: {e}")))?;
        let rows = stmt
            .query_map([symbol], |r| r.get::<_, String>(0))
            .map_err(|e| CliError::Other(format!("exec target resolve: {e}")))?;
        let mut out = Vec::new();
        for r in rows.flatten() {
            out.push(r);
        }
        out
    };

    let mut targets: Vec<String> = target_rows;
    // Always include the raw input — covers references to symbols not
    // present as their own node row (extern crate, missed by indexer).
    if !targets.iter().any(|t| t == symbol) {
        targets.push(symbol.to_string());
    }
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Edges where target_qualified IN (...) → "usages" / "calls" / "imports"
    let placeholders = vec!["?"; targets.len()].join(",");
    let usage_sql = format!(
        "SELECT e.source_qualified, e.kind, \
                COALESCE(n.file_path, '') AS file, \
                COALESCE(n.line_start, 0) AS line, \
                COALESCE(n.signature, '') AS signature \
         FROM edges e \
         LEFT JOIN nodes n ON n.qualified_name = e.source_qualified \
         WHERE e.target_qualified IN ({placeholders}) \
         ORDER BY e.kind, file \
         LIMIT 500"
    );
    let mut usage_hits: Vec<RefHit> = Vec::new();
    {
        let mut stmt = conn
            .prepare(&usage_sql)
            .map_err(|e| CliError::Other(format!("prep usage query: {e}")))?;
        let rows = stmt
            .query_map(params_from_iter(targets.iter()), |row| {
                Ok(RefHit {
                    source: row.get::<_, String>(0)?,
                    kind: map_kind(&row.get::<_, String>(1)?).to_string(),
                    file: {
                        let f: String = row.get(2)?;
                        if f.is_empty() {
                            None
                        } else {
                            Some(f)
                        }
                    },
                    line: {
                        let l: i64 = row.get(3)?;
                        if l > 0 {
                            Some(l)
                        } else {
                            None
                        }
                    },
                    context: row.get::<_, String>(4)?,
                })
            })
            .map_err(|e| CliError::Other(format!("exec usage query: {e}")))?;
        for r in rows.flatten() {
            usage_hits.push(r);
        }
    }

    // 3. Definitions: nodes whose qualified_name matches any target OR
    //    whose name matches the raw input.
    let def_sql = format!(
        "SELECT file_path, line_start, COALESCE(signature, ''), qualified_name, kind \
         FROM nodes \
         WHERE qualified_name IN ({placeholders}) \
            OR name = ? \
         LIMIT 100"
    );
    let mut def_hits: Vec<RefHit> = Vec::new();
    {
        let mut stmt = conn
            .prepare(&def_sql)
            .map_err(|e| CliError::Other(format!("prep def query: {e}")))?;
        let mut all_params: Vec<&str> = targets.iter().map(String::as_str).collect();
        all_params.push(symbol);
        let rows = stmt
            .query_map(params_from_iter(all_params.iter()), |row| {
                Ok(RefHit {
                    file: row.get::<_, Option<String>>(0)?,
                    line: row.get::<_, Option<i64>>(1)?,
                    context: row.get::<_, String>(2)?,
                    source: row.get::<_, String>(3)?,
                    kind: format!(
                        "definition[{}]",
                        row.get::<_, Option<String>>(4)?.unwrap_or_default()
                    ),
                })
            })
            .map_err(|e| CliError::Other(format!("exec def query: {e}")))?;
        for r in rows.flatten() {
            def_hits.push(r);
        }
    }

    // 4. Merge defs first, then usages, capped at `limit`.
    let mut out: Vec<RefHit> = Vec::with_capacity(def_hits.len() + usage_hits.len());
    out.extend(def_hits);
    out.extend(usage_hits);
    out.truncate(limit);
    Ok(out)
}

fn print_hits(symbol: &str, hits: &[RefHit]) {
    if hits.is_empty() {
        println!("no references found for `{symbol}`");
        println!();
        println!(
            "tip: try the bare symbol name (e.g. `Store` not `crate::manager::Store`),\n     or run `mneme recall \"{symbol}\"` for a fuzzy text search."
        );
        return;
    }
    println!("{} reference(s) to `{}`:", hits.len(), symbol);
    println!();
    for h in hits {
        // Bug #38: strip Windows long-path prefix at display boundary.
        let loc = match (&h.file, h.line) {
            (Some(f), Some(l)) => format!("{}:{l}", super::display_path(f)),
            (Some(f), None) => super::display_path(f).to_string(),
            _ => "-".into(),
        };
        println!("  [{}] {} @ {}", h.kind, h.source, loc);
        if !h.context.is_empty() && h.context != h.source {
            println!("      {}", h.context);
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: FindReferencesArgs,
    }

    #[tokio::test]
    async fn empty_symbol_is_rejected() {
        let args = FindReferencesArgs {
            symbol: "   ".to_string(),
            limit: 50,
            project: None,
        };
        let r = run(args).await;
        match r {
            Err(CliError::Other(msg)) => {
                assert!(msg.contains("symbol must not be empty"), "wrong: {msg}")
            }
            other => panic!("expected Err(Other(empty)), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn nul_byte_in_symbol_is_rejected() {
        let args = FindReferencesArgs {
            symbol: "Foo\0Bar".to_string(),
            limit: 50,
            project: None,
        };
        let r = run(args).await;
        match r {
            Err(CliError::Other(msg)) => assert!(msg.contains("NUL"), "wrong: {msg}"),
            other => panic!("expected Err(Other(NUL)), got {other:?}"),
        }
    }

    #[test]
    fn args_parse_with_just_symbol() {
        let h = Harness::try_parse_from(["x", "PathManager"]).unwrap();
        assert_eq!(h.args.symbol, "PathManager");
        assert_eq!(h.args.limit, 50);
        assert_eq!(h.args.project, None);
    }

    #[test]
    fn args_parse_with_limit_and_project() {
        let h = Harness::try_parse_from([
            "x",
            "PathManager",
            "--limit",
            "100",
            "--project",
            "/tmp/foo",
        ])
        .unwrap();
        assert_eq!(h.args.symbol, "PathManager");
        assert_eq!(h.args.limit, 100);
        assert_eq!(h.args.project, Some(PathBuf::from("/tmp/foo")));
    }

    #[test]
    fn args_parser_rejects_zero_limit() {
        let r = Harness::try_parse_from(["x", "Foo", "--limit", "0"]);
        assert!(r.is_err(), "limit=0 must be rejected");
    }

    #[test]
    fn args_parser_rejects_huge_limit() {
        let r = Harness::try_parse_from(["x", "Foo", "--limit", "10001"]);
        assert!(r.is_err(), "limit>10000 must be rejected");
    }

    #[test]
    fn map_kind_normalizes_aliases() {
        assert_eq!(map_kind("definition"), "definition");
        assert_eq!(map_kind("call"), "call");
        assert_eq!(map_kind("calls"), "call");
        assert_eq!(map_kind("import"), "import");
        assert_eq!(map_kind("imports"), "import");
        assert_eq!(map_kind("contains"), "usage");
        assert_eq!(map_kind("anything-else"), "usage");
    }

    #[test]
    fn print_hits_handles_empty_list() {
        // Smoke: should not panic.
        print_hits("nothing_here", &[]);
    }
}
