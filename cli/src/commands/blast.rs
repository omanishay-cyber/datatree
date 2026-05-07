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
use crate::commands::ipc_helpers::{
    graph_db_path, resolve_project_root, try_ipc_dispatch, IpcDispatch,
};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::query::BlastItem;

/// CLI args for `mneme blast`.
#[derive(Debug, Args)]
pub struct BlastArgs {
    /// File path or fully-qualified function name (e.g. `src/auth.ts:login`
    /// or just a bare name like `authenticate`).
    pub target: String,

    /// Max traversal depth. 1 = direct dependents only. Clamped at
    /// parse-time to 1..=64 (REG-022 spirit: no unbounded fan-out).
    /// Default of 1 keeps responses small; pass `--depth 5` or `--deep`
    /// for transitive walks on highly-connected nodes.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..=64))]
    pub depth: u64,

    /// Convenience flag: equivalent to `--depth 5`. Ignored if `--depth`
    /// is also passed explicitly with a value > 1.
    #[arg(long, default_value_t = false)]
    pub deep: bool,

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
    // REG-006: empty/whitespace target is meaningless — no node will ever
    // match. Reject up-front so we don't waste an IPC round trip.
    if args.target.trim().is_empty() {
        return Err(CliError::Other("target must not be empty".to_string()));
    }

    let project_root = resolve_project_root(args.project.clone());
    // `--deep` expands to depth=5 only when `--depth` is at the default (1).
    // An explicit `--depth N` with N > 1 wins over `--deep`.
    let depth = if args.deep && args.depth == 1 {
        5usize
    } else {
        args.depth as usize
    };

    // HIGH-48 (2026-05-06, 2026-05-05 audit): consolidated IPC dispatch via
    // cli::ipc_helpers::try_ipc_dispatch. Error arms are shared; success arm
    // is inline here because it is specific to blast (BlastResults variant).
    let client = make_client(socket_override);
    let req = IpcRequest::Blast {
        project: project_root.clone(),
        target: args.target.clone(),
        depth,
    };
    let outcome = try_ipc_dispatch(
        &client,
        req,
        |resp| match resp {
            IpcResponse::BlastResults { impacted } => {
                info!(
                    source = "supervisor",
                    count = impacted.len(),
                    "blast served"
                );
                print_layers_from_items(&args.target, &impacted, depth, &project_root);
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

    // Direct-DB fallback — identical to the v0.3.1 behaviour.
    info!(source = "direct-db", "blast served");
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

    // Resolve target to one or more starting node qualified_names.
    let starts = resolve_target(&conn, &args.target)?;
    if starts.is_empty() {
        println!("no node matches target `{}`", args.target);
        return Ok(());
    }

    // BFS in the reverse direction: who points AT me.
    let mut visited: HashSet<String> = starts.iter().cloned().collect();
    let mut frontier: VecDeque<(String, usize)> = starts.iter().map(|s| (s.clone(), 0)).collect();
    let mut layers: Vec<Vec<String>> = vec![starts.clone()];
    for _ in 0..depth {
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
        if d >= depth {
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

    print_layers(&args.target, &layers, Some(&conn));
    Ok(())
}

/// Render an IPC `BlastResults` into the same layered output the direct-DB
/// path produces. Supervisor-side BFS emits a flat `Vec<BlastItem>` with
/// per-item `depth` so we can reconstruct the layered presentation here
/// without the CLI having to track BFS state itself.
///
/// BENCH-FIX-3 (2026-05-07): open a read-only graph.db connection (if
/// available) to enrich opaque `n_<hex>` qualified_names with friendly
/// `[kind] name @ file:line` rows. Without this, blast prints internal
/// stable_ids that the user can't act on. `project_root` is the same
/// resolved root used in `run` — falls back to None on path-derive
/// failure (rare).
fn print_layers_from_items(
    target: &str,
    items: &[BlastItem],
    max_depth: usize,
    project_root: &std::path::Path,
) {
    let mut layers: Vec<Vec<String>> = vec![Vec::new()];
    for _ in 0..max_depth {
        layers.push(Vec::new());
    }
    for it in items {
        if let Some(layer) = layers.get_mut(it.depth) {
            layer.push(it.qualified_name.clone());
        }
    }
    // Best-effort enrichment: open a read-only graph.db conn just for
    // the lookup. If the path can't be derived (project hash failed) or
    // the file isn't there yet, fall through to the raw-id printer.
    let conn = graph_db_path(project_root).ok().and_then(|p| {
        Connection::open_with_flags(
            &p,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()
    });
    print_layers(target, &layers, conn.as_ref());
}

/// Resolve a target string to one or more starting node qualified_names.
/// Matches both `qualified_name` (exact) and `name` (exact, case-sensitive).
fn resolve_target(conn: &Connection, target: &str) -> CliResult<Vec<String>> {
    // Match `file_path` BOTH with and without the Windows `\\?\`
    // long-path prefix that `Path::canonicalize` injects on Windows. The
    // parser stores file_path values in canonical form (with prefix) but
    // CLI users typically pass the unprefixed form (`C:\Users\...`).
    // Without this OR-clause every blast on a Windows full-path target
    // returns 0 dependents — `starts` is empty so BFS never runs.
    let mut stmt = conn
        .prepare(
            r#"SELECT qualified_name FROM nodes
             WHERE qualified_name = ?1
                OR name = ?1
                OR file_path = ?1
                OR file_path = '\\?\' || ?1
             ORDER BY CASE
               WHEN qualified_name = ?1 THEN 0
               WHEN name = ?1 THEN 1
               WHEN file_path = ?1 THEN 2
               ELSE 3
             END
             LIMIT 10"#,
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

/// BENCH-FIX-3 (2026-05-07): when a graph.db `Connection` is available,
/// each `qualified_name` gets resolved to its `[kind] name @ file:line`
/// representation via `enrich_qn`. Without enrichment users see opaque
/// stable_ids like `n_9919e10ae058faed` that are useless without a
/// follow-up `mneme recall`.
fn print_layers(target: &str, layers: &[Vec<String>], conn: Option<&Connection>) {
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
            match conn.and_then(|c| enrich_qn(c, q)) {
                Some(e) => {
                    let kind = if e.kind.is_empty() { "?" } else { &e.kind };
                    let name = if e.name.is_empty() {
                        q.as_str()
                    } else {
                        &e.name
                    };
                    // Bug #38: strip Windows long-path prefix at display boundary.
                    let loc = match (e.file_path.as_deref(), e.line_start) {
                        (Some(f), Some(l)) if l > 0 => {
                            format!("{}:{l}", super::display_path(f))
                        }
                        (Some(f), _) => super::display_path(f).to_string(),
                        _ => "-".into(),
                    };
                    println!("  [{kind}] {name} @ {loc}");
                }
                None => println!("  {q}"),
            }
        }
        println!();
    }
}

/// One enriched node row used by `print_layers` to render
/// `[kind] name @ file:line` instead of the raw `qualified_name`.
struct EnrichedNode {
    kind: String,
    name: String,
    file_path: Option<String>,
    line_start: Option<i64>,
}

/// Look up a single `qualified_name` row to produce the friendly display.
/// Returns `None` on any error (missing row, prepared-stmt failure, etc.) —
/// the caller falls back to printing the raw qualified_name in that case,
/// so the user still sees something rather than the lookup failing the
/// whole blast print.
fn enrich_qn(conn: &Connection, qn: &str) -> Option<EnrichedNode> {
    conn.query_row(
        "SELECT kind, name, file_path, line_start FROM nodes WHERE qualified_name = ?1 LIMIT 1",
        rusqlite::params![qn],
        |row| {
            Ok(EnrichedNode {
                kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                file_path: row.get::<_, Option<String>>(2)?,
                line_start: row.get::<_, Option<i64>>(3)?,
            })
        },
    )
    .ok()
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
        args: BlastArgs,
    }

    #[tokio::test]
    async fn empty_target_rejected_before_ipc() {
        // REG-006: blast must reject an empty/whitespace target up-front.
        let args = BlastArgs {
            target: "   ".to_string(),
            depth: 1,
            deep: false,
            project: None,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme.sock"))).await;
        match r {
            Err(CliError::Other(msg)) => assert!(
                msg.contains("target must not be empty"),
                "wrong message: {msg}"
            ),
            other => panic!("expected Other(empty), got {other:?}"),
        }
    }

    #[test]
    fn print_layers_handles_empty_input() {
        // smoke: should not panic with an empty layer list. `None` for
        // the conn — no enrichment lookup is attempted.
        print_layers("nothing", &[], None);
    }

    #[test]
    fn blast_args_parse_with_default_depth() {
        // Target alone must parse; depth defaults to 1 (direct neighbours
        // only). Power users opt into deeper walks via --depth N or --deep.
        let h = Harness::try_parse_from(["x", "myFunc"]).unwrap();
        assert_eq!(h.args.target, "myFunc");
        assert_eq!(h.args.depth, 1);
        assert!(!h.args.deep);
        assert!(h.args.project.is_none());
    }

    #[test]
    fn blast_args_parse_with_deep_flag() {
        // --deep should set deep=true while leaving depth at the default 1
        // — the run() function expands deep=true + depth=1 into depth=5.
        let h = Harness::try_parse_from(["x", "myFunc", "--deep"]).unwrap();
        assert_eq!(h.args.depth, 1);
        assert!(h.args.deep);
    }

    #[test]
    fn blast_args_parse_with_explicit_depth() {
        // --depth must accept user values within 1..=64.
        let h = Harness::try_parse_from(["x", "myFunc", "--depth", "5"]).unwrap();
        assert_eq!(h.args.depth, 5);
    }

    #[test]
    fn blast_args_parser_rejects_depth_zero() {
        // REG-022: depth is clamped to 1..=64; 0 must fail at parse time.
        let r = Harness::try_parse_from(["x", "myFunc", "--depth", "0"]);
        assert!(r.is_err(), "depth=0 must be rejected at parse time");
    }

    #[test]
    fn blast_args_parser_rejects_depth_over_max() {
        // REG-022: depth above 64 must fail at parse time.
        let r = Harness::try_parse_from(["x", "myFunc", "--depth", "65"]);
        assert!(r.is_err(), "depth>64 must be rejected at parse time");
    }

    #[test]
    fn print_layers_skips_layer_zero() {
        // depth-0 is the target itself (already in header) — must not be
        // re-printed even if populated. Also must not panic. Pass `None`
        // for the conn (no enrichment) — the test just verifies depth-0
        // is skipped, not the rendering format.
        let layers = vec![
            vec!["self".to_string()],
            vec!["a".to_string(), "b".to_string()],
        ];
        print_layers("self", &layers, None);
    }

    #[test]
    fn print_layers_from_items_smoke() {
        // Construct a flat IPC response with depths and verify it
        // doesn't panic when reconstructed into layers. The dummy
        // project_root here doesn't have a graph.db — print_layers_from_items
        // tolerates that and falls back to raw qualified_name printing.
        let items = vec![
            BlastItem {
                qualified_name: "x::a".into(),
                depth: 1,
            },
            BlastItem {
                qualified_name: "x::b".into(),
                depth: 2,
            },
        ];
        print_layers_from_items("x", &items, 3, std::path::Path::new("."));
    }
}
