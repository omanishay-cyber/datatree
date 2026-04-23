//! `datatree build [project_path]` — initial full project ingest.
//!
//! v0.1 strategy: drive parse + store IN-PROCESS. The CLI walks the project,
//! parses each supported file with Tree-sitter directly (via the `parsers`
//! library), and writes nodes + edges to the project's `graph.db` via the
//! store library. No supervisor round-trip — that path is wired in v0.2.
//!
//! Benefit: `datatree build .` produces a real, queryable SQLite graph
//! without any worker pool round-trip or IPC dependency.

use clap::Args;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

use crate::error::{CliError, CliResult};
use crate::ipc::{IpcClient, IpcRequest, IpcResponse};

use datatree_common::{layer::DbLayer, paths::PathManager, ids::ProjectId};
use datatree_store::{inject::InjectOptions, Store};
use parsers::{
    extractor::Extractor, incremental::IncrementalParser, parser_pool::ParserPool, query_cache,
    Language,
};

/// CLI args for `datatree build`.
#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Path to the project root. Defaults to CWD.
    pub project: Option<PathBuf>,

    /// Force a re-parse of every file (default: only changed since last build).
    #[arg(long)]
    pub full: bool,

    /// Maximum files to process (0 = unlimited). Useful for smoke-testing.
    #[arg(long, default_value_t = 0)]
    pub limit: usize,
}

/// Entry point used by `main.rs`.
pub async fn run(args: BuildArgs, _socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project)?;
    info!(project = %project.display(), full = args.full, "building datatree graph");

    // 1. Store setup: open (or create) the per-project shard.
    let paths = PathManager::default_root();
    let store = Store::new(paths.clone());
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let project_name = project
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let _shard = store
        .builder
        .build_or_migrate(&project_id, &project, &project_name)
        .await
        .map_err(|e| CliError::Other(format!("store build_or_migrate: {e}")))?;
    println!(
        "shard ready at {}",
        paths.project_root(&project_id).display()
    );

    // 2. Parser pool — small (4 parsers/language) so one CLI process stays
    // lean. Tree-sitter parses are CPU-bound; tokio's multi-threaded
    // runtime will use all cores concurrently via `spawn_blocking`.
    let pool = Arc::new(
        ParserPool::new(4)
            .map_err(|e| CliError::Other(format!("parser pool init: {e}")))?,
    );
    if let Err(e) = query_cache::warm_up() {
        warn!(error = %e, "query warm-up reported issues (non-fatal)");
    }
    let inc = Arc::new(IncrementalParser::new(pool.clone()));

    // 3. Walk the project. Respect common ignore patterns.
    let walker = walkdir::WalkDir::new(&project)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()));

    let mut total = 0usize;
    let mut indexed = 0usize;
    let mut skipped = 0usize;
    let mut node_total = 0u64;
    let mut edge_total = 0u64;

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "walk error; continuing");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if args.limit > 0 && indexed >= args.limit {
            break;
        }
        total += 1;

        let path = entry.path();
        let Some(lang) = Language::from_filename(path) else {
            skipped += 1;
            continue;
        };
        if !lang.is_enabled() {
            skipped += 1;
            continue;
        }

        let content = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                warn!(file = %path.display(), error = %e, "read failed; skipping");
                skipped += 1;
                continue;
            }
        };
        if looks_binary(&content) {
            skipped += 1;
            continue;
        }

        let content_arc = Arc::new(content);
        let parse_result = inc.parse_file(path, lang, content_arc.clone()).await;
        let parse = match parse_result {
            Ok(p) => p,
            Err(e) => {
                warn!(file = %path.display(), error = %e, "parse failed; skipping");
                skipped += 1;
                continue;
            }
        };

        let extractor = Extractor::new(lang);
        let graph = match extractor.extract(&parse.tree, &content_arc, path) {
            Ok(g) => g,
            Err(e) => {
                warn!(file = %path.display(), error = %e, "extract failed; skipping");
                skipped += 1;
                continue;
            }
        };
        let n_nodes = graph.nodes.len();
        let n_edges = graph.edges.len();

        // Persist. Map parsers::Node → graph.db schema. `id` from parsers
        // becomes the qualified_name (it's already a stable, unique string
        // per §stable_id in extractor.rs).
        for node in &graph.nodes {
            let sql = "INSERT OR REPLACE INTO nodes(kind,name,qualified_name,file_path,line_start,line_end,language,extra,updated_at) \
                       VALUES(?1,?2,?3,?4,?5,?6,?7,?8,datetime('now'))";
            let params = vec![
                serde_json::Value::String(format!("{:?}", node.kind).to_lowercase()),
                serde_json::Value::String(node.name.clone()),
                serde_json::Value::String(node.id.clone()),
                serde_json::Value::String(node.file.display().to_string()),
                serde_json::Value::Number((node.line_range.0 as i64).into()),
                serde_json::Value::Number((node.line_range.1 as i64).into()),
                serde_json::Value::String(format!("{:?}", node.language).to_lowercase()),
                serde_json::Value::String(
                    serde_json::json!({
                        "confidence": format!("{:?}", node.confidence).to_lowercase(),
                        "byte_range": [node.byte_range.0, node.byte_range.1],
                    })
                    .to_string(),
                ),
            ];
            let _ = store
                .inject
                .insert(
                    &project_id,
                    DbLayer::Graph,
                    sql,
                    params,
                    InjectOptions {
                        emit_event: false,
                        audit: false,
                        ..InjectOptions::default()
                    },
                )
                .await;
        }
        for edge in &graph.edges {
            let sql = "INSERT INTO edges(kind,source_qualified,target_qualified,confidence,confidence_score,source_extractor,extra,updated_at) \
                       VALUES(?1,?2,?3,?4,?5,?6,?7,datetime('now'))";
            let conf = format!("{:?}", edge.confidence).to_lowercase();
            let score = edge.confidence.weight();
            let params = vec![
                serde_json::Value::String(format!("{:?}", edge.kind).to_lowercase()),
                serde_json::Value::String(edge.from.clone()),
                serde_json::Value::String(edge.to.clone()),
                serde_json::Value::String(conf),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(score as f64)
                        .unwrap_or_else(|| serde_json::Number::from(1)),
                ),
                serde_json::Value::String("parsers".into()),
                serde_json::Value::String(
                    serde_json::json!({
                        "unresolved": edge.unresolved_target,
                    })
                    .to_string(),
                ),
            ];
            let _ = store
                .inject
                .insert(
                    &project_id,
                    DbLayer::Graph,
                    sql,
                    params,
                    InjectOptions {
                        emit_event: false,
                        audit: false,
                        ..InjectOptions::default()
                    },
                )
                .await;
        }

        indexed += 1;
        node_total += n_nodes as u64;
        edge_total += n_edges as u64;
        if indexed % 25 == 0 {
            println!("  indexed {indexed} files ({node_total} nodes, {edge_total} edges)");
        }
    }

    println!();
    println!("build complete:");
    println!("  walked:  {total} files");
    println!("  indexed: {indexed}");
    println!("  skipped: {skipped}");
    println!("  nodes:   {node_total}");
    println!("  edges:   {edge_total}");
    println!("  shard:   {}", paths.project_root(&project_id).display());

    Ok(())
}

/// Resolve `project` to an absolute, canonicalised path. Falls back to
/// CWD if the user passed nothing.
pub(crate) fn resolve_project(arg: Option<PathBuf>) -> CliResult<PathBuf> {
    let raw = arg.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let canonical = std::fs::canonicalize(&raw).unwrap_or(raw);
    Ok(canonical)
}

/// Build an IPC client honoring `--socket` overrides.
pub(crate) fn make_client(socket_override: Option<PathBuf>) -> IpcClient {
    match socket_override {
        Some(p) => IpcClient::new(p),
        None => IpcClient::default_path(),
    }
}

/// Pretty-print any [`IpcResponse`] variant, surface [`IpcResponse::Error`]
/// as a [`CliError::Supervisor`]. Used by every IPC-bound command.
pub(crate) fn handle_response(response: IpcResponse) -> CliResult<()> {
    match response {
        IpcResponse::Pong => {
            println!("pong");
            Ok(())
        }
        IpcResponse::Status { children } => {
            println!("{}", serde_json::to_string_pretty(&children)?);
            Ok(())
        }
        IpcResponse::Logs { entries } => {
            for e in &entries {
                println!("{}", serde_json::to_string(e)?);
            }
            Ok(())
        }
        IpcResponse::Ok { message } => {
            if let Some(m) = message {
                println!("{m}");
            } else {
                println!("ok");
            }
            Ok(())
        }
        IpcResponse::Error { message } => Err(CliError::Supervisor(message)),
    }
}

/// Keep `IpcRequest` import alive so `cargo check --warnings` stays clean
/// even when the supervisor-bound request machinery is exercised only by
/// other commands.
#[allow(dead_code)]
fn _ipc_req_anchor() -> Option<IpcRequest> {
    None
}

fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(
        name,
        "target"
            | "node_modules"
            | ".git"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | ".idea"
            | ".vscode"
    )
}

/// Heuristic: treat any byte slice whose first 512 bytes contain a NUL as
/// binary. This skips images, compiled object files, etc.
fn looks_binary(buf: &[u8]) -> bool {
    buf.iter().take(512).any(|&b| b == 0)
}
