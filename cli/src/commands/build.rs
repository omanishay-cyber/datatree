//! `mneme build [project_path]` — initial full project ingest.
//!
//! v0.1 strategy: drive parse + store IN-PROCESS. The CLI walks the project,
//! parses each supported file with Tree-sitter directly (via the `parsers`
//! library), and writes nodes + edges to the project's `graph.db` via the
//! store library. No supervisor round-trip — that path is wired in v0.2.
//!
//! Benefit: `mneme build .` produces a real, queryable SQLite graph
//! without any worker pool round-trip or IPC dependency.

use clap::Args;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

use crate::error::{CliError, CliResult};
use crate::ipc::{IpcClient, IpcRequest, IpcResponse};

use common::{layer::DbLayer, paths::PathManager, ids::ProjectId};
use multimodal::{ExtractedDoc, Registry as MmRegistry};
use sha2::{Digest, Sha256};
use store::{inject::InjectOptions, Store};
use parsers::{
    extractor::Extractor, incremental::IncrementalParser, parser_pool::ParserPool, query_cache,
    Language,
};

/// CLI args for `mneme build`.
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

    /// (v0.3) Dispatch parse/scan/embed work to the supervisor's
    /// worker pool instead of running the pipeline inline in this CLI
    /// process. Falls back to inline automatically if the supervisor is
    /// unreachable.
    #[arg(long)]
    pub dispatch: bool,

    /// Force inline execution even when the supervisor is reachable.
    /// This is the v0.1–v0.2 path and remains the default today; the
    /// flag exists to let future defaults flip without breaking scripts.
    #[arg(long, conflicts_with = "dispatch")]
    pub inline: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: BuildArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project.clone())?;
    info!(project = %project.display(), full = args.full, dispatch = args.dispatch, "building mneme graph");

    // --dispatch: try the supervisor path; fall back to inline if the
    // daemon isn't running. --inline: force the in-process pipeline
    // (also the default).
    if args.dispatch && !args.inline {
        let client = make_client(socket_override.clone());
        if client.is_running().await {
            match run_dispatched(&args, &project, &client).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(error = %e, "dispatch path failed; falling back to inline build");
                }
            }
        } else {
            warn!("supervisor unreachable; falling back to inline build");
        }
    }

    run_inline(args, project).await
}

/// Walk the project and submit one `Job::Parse` per source file to the
/// supervisor, then poll `JobQueueStatus` until the queue drains.
///
/// v0.3 MVP: wires only `Job::Parse`. `Scan`/`Embed`/`Ingest` are still
/// running inline from the subcommand that needs them; they'll be
/// migrated in follow-ups per ARCHITECTURE.md §Worker dispatch roadmap.
async fn run_dispatched(
    args: &BuildArgs,
    project: &Path,
    client: &IpcClient,
) -> CliResult<()> {
    use common::jobs::Job;

    let paths = PathManager::default_root();
    let project_id = ProjectId::from_path(project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let shard_root = paths.project_root(&project_id);

    let walker = walkdir::WalkDir::new(project)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()));

    let mut submitted = 0usize;
    let mut total = 0usize;
    let mut skipped = 0usize;
    let started = std::time::Instant::now();

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
        if args.limit > 0 && submitted >= args.limit {
            break;
        }
        let job = Job::Parse {
            file_path: path.to_path_buf(),
            shard_root: shard_root.clone(),
        };
        match client.request(IpcRequest::DispatchJob { job }).await? {
            IpcResponse::JobQueued { .. } => submitted += 1,
            IpcResponse::Error { message } => {
                return Err(CliError::Supervisor(format!(
                    "DispatchJob rejected after {submitted} submissions: {message}"
                )));
            }
            other => {
                return Err(CliError::Supervisor(format!(
                    "unexpected DispatchJob response: {other:?}"
                )));
            }
        }
    }
    println!("submitted {submitted} parse jobs ({skipped} skipped, {total} walked)");

    // Watchdog: poll until pending + in_flight == 0 or timeout.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if std::time::Instant::now() > deadline {
            return Err(CliError::Supervisor(
                "timeout waiting for dispatched build to finish".into(),
            ));
        }
        let resp = client.request(IpcRequest::JobQueueStatus).await?;
        let snap = match resp {
            IpcResponse::JobQueue { snapshot } => snapshot,
            IpcResponse::Error { message } => {
                return Err(CliError::Supervisor(format!("JobQueueStatus: {message}")))
            }
            _ => return Err(CliError::Supervisor("unexpected JobQueueStatus resp".into())),
        };
        let pending = snap.get("pending").and_then(|v| v.as_u64()).unwrap_or(0);
        let in_flight = snap.get("in_flight").and_then(|v| v.as_u64()).unwrap_or(0);
        if pending == 0 && in_flight == 0 {
            let completed = snap.get("completed").and_then(|v| v.as_u64()).unwrap_or(0);
            let failed = snap.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
            let elapsed = started.elapsed();
            println!(
                "dispatched build done in {elapsed:?}: completed={completed} failed={failed}"
            );
            return Ok(());
        }
    }
}

/// The classic in-process pipeline — unchanged from v0.2 behaviour.
async fn run_inline(args: BuildArgs, project: PathBuf) -> CliResult<()> {
    let _ = args.inline; // silence unused

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

    // 4. Multimodal pass. The code walk only handles Tree-sitter-backed
    // languages; anything else (PDFs, Markdown, images, audio, video) is
    // handed to `mneme-multimodal`'s registry. PDF pages are the MVP here:
    // extracted text lands in BOTH `multimodal.db::media` (full document
    // payload) AND `graph.db::nodes` (one row per page, kind='pdf_page',
    // `summary` = page text — so `recall_concept` hits PDF content via the
    // existing nodes_fts index without a schema bump).
    let mm_stats = run_multimodal_pass(&store, &project_id, &project).await;

    println!();
    println!("build complete:");
    println!("  walked:       {total} files");
    println!("  indexed:      {indexed}");
    println!("  skipped:      {skipped}");
    println!("  nodes:        {node_total}");
    println!("  edges:        {edge_total}");
    println!(
        "  multimodal:   {} files, {} pages ({} errors, {} pages/sec)",
        mm_stats.files_ok,
        mm_stats.pages_total,
        mm_stats.errors,
        mm_stats.pages_per_sec()
    );
    println!("  shard:        {}", paths.project_root(&project_id).display());

    Ok(())
}

/// Aggregate stats from the multimodal pass.
#[derive(Debug, Default)]
struct MultimodalStats {
    files_ok: usize,
    /// Files we tried to extract but failed on (error already counted too).
    /// Kept as a separate counter so the top-line summary could split the
    /// two; currently only `errors` is printed.
    #[allow(dead_code)]
    files_skipped: usize,
    pages_total: usize,
    errors: usize,
    duration_secs: f64,
}

impl MultimodalStats {
    fn pages_per_sec(&self) -> String {
        if self.duration_secs <= 0.0 {
            return "-".into();
        }
        format!("{:.1}", self.pages_total as f64 / self.duration_secs)
    }
}

/// Walk `project`, dispatch every path the multimodal [`Registry`] claims
/// through its extractor, and persist the result. PDFs are the fully-wired
/// path; other kinds (markdown/image/audio/video) go through the same
/// machinery but are tolerated if their extractors are feature-gated off.
///
/// Errors are logged and counted, never raised — the multimodal pass is
/// strictly additive on top of a successful code build.
async fn run_multimodal_pass(
    store: &Store,
    project_id: &ProjectId,
    project: &Path,
) -> MultimodalStats {
    let mut stats = MultimodalStats::default();
    let start = Instant::now();
    let registry = MmRegistry::default_wired();

    let walker = walkdir::WalkDir::new(project)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()));

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "mm walk error; continuing");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if registry.find(path).is_none() {
            continue;
        }

        let doc = match registry.extract(path) {
            Ok(d) => d,
            Err(multimodal::ExtractError::Unsupported { .. }) => {
                continue;
            }
            Err(e) => {
                warn!(file = %path.display(), error = %e, "multimodal extract failed");
                stats.errors += 1;
                stats.files_skipped += 1;
                continue;
            }
        };

        let pages_written = match persist_multimodal(store, project_id, &doc).await {
            Ok(n) => n,
            Err(e) => {
                warn!(file = %path.display(), error = %e, "multimodal persist failed");
                stats.errors += 1;
                stats.files_skipped += 1;
                continue;
            }
        };
        stats.files_ok += 1;
        stats.pages_total += pages_written;
    }

    stats.duration_secs = start.elapsed().as_secs_f64();
    stats
}

/// Persist an [`ExtractedDoc`] to the project shard.
///
/// Two writes per document:
///   1. `multimodal.db::media` — one row per file (whole payload).
///   2. `graph.db::nodes` — one row per page. For PDFs this is what
///      `recall_concept` returns, since `nodes_fts.summary` is indexed.
///      For `kind != "pdf"` we still write the top-level document as a
///      single node (no per-page split) so the text is discoverable.
///
/// Idempotent via `INSERT OR REPLACE` on the unique key columns
/// (`media.path`, `nodes.qualified_name`).
async fn persist_multimodal(
    store: &Store,
    project: &ProjectId,
    doc: &ExtractedDoc,
) -> Result<usize, String> {
    let bytes = std::fs::read(&doc.source)
        .map_err(|e| format!("read {}: {e}", doc.source.display()))?;
    let sha = hex_sha256(&bytes);
    let elements_json = serde_json::to_string(&doc.elements).unwrap_or_else(|_| "[]".into());
    let transcript_json = if doc.transcript.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&doc.transcript).unwrap_or_default()
    };

    // Write to multimodal.db::media (whole-document payload).
    let media_sql = "INSERT OR REPLACE INTO media(path, sha256, media_type, extracted_text, elements, transcript, extracted_at, extractor_version) \
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), ?7)";
    let media_params = vec![
        serde_json::Value::String(doc.source.display().to_string()),
        serde_json::Value::String(sha),
        serde_json::Value::String(doc.kind.clone()),
        serde_json::Value::String(doc.text.clone()),
        serde_json::Value::String(elements_json),
        if transcript_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(transcript_json)
        },
        serde_json::Value::String(doc.extractor_version.clone()),
    ];
    let resp = store
        .inject
        .insert(
            project,
            DbLayer::Multimodal,
            media_sql,
            media_params,
            InjectOptions {
                emit_event: false,
                audit: false,
                ..InjectOptions::default()
            },
        )
        .await;
    if !resp.success {
        return Err(resp
            .error
            .map(|e| format!("media insert: {e:?}"))
            .unwrap_or_else(|| "unknown media insert error".into()));
    }

    // Write to graph.db::nodes (one row per page for PDFs, one for the
    // whole doc otherwise). The `summary` column is indexed by nodes_fts
    // so `recall_concept` surfaces the text without a schema change.
    let mut pages_written = 0usize;
    let source_display = doc.source.display().to_string();
    let scheme = match doc.kind.as_str() {
        "pdf" => "pdf",
        "markdown" => "md",
        "image" => "img",
        "audio" => "audio",
        "video" => "video",
        _ => "file",
    };

    let page_records: Vec<(u32, String, Option<String>)> = if doc.pages.is_empty() {
        vec![(1, doc.text.clone(), None)]
    } else {
        doc.pages
            .iter()
            .map(|p| (p.index, p.text.clone(), p.heading.clone()))
            .collect()
    };

    for (page_num, page_text, heading) in page_records {
        // Skip empty pages — no point indexing whitespace. Keeps the graph
        // lean and avoids a noisy FTS row per blank PDF page.
        if page_text.trim().is_empty() {
            continue;
        }

        let node_kind = format!("{}_page", doc.kind);
        let qualified = format!("{scheme}://{source_display}#page{page_num}");
        let name = heading.clone().unwrap_or_else(|| format!("Page {page_num}"));
        let extra = serde_json::json!({
            "kind": doc.kind,
            "page_num": page_num,
            "heading": heading,
            "bbox": serde_json::Value::Null,
            "extractor_version": doc.extractor_version,
        })
        .to_string();

        let node_sql = "INSERT OR REPLACE INTO nodes(kind,name,qualified_name,file_path,line_start,line_end,language,summary,extra,updated_at) \
                        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,datetime('now'))";
        let node_params = vec![
            serde_json::Value::String(node_kind),
            serde_json::Value::String(name),
            serde_json::Value::String(qualified),
            serde_json::Value::String(source_display.clone()),
            serde_json::Value::Number((page_num as i64).into()),
            serde_json::Value::Number((page_num as i64).into()),
            serde_json::Value::String(doc.kind.clone()),
            serde_json::Value::String(page_text),
            serde_json::Value::String(extra),
        ];
        let resp = store
            .inject
            .insert(
                project,
                DbLayer::Graph,
                node_sql,
                node_params,
                InjectOptions {
                    emit_event: false,
                    audit: false,
                    ..InjectOptions::default()
                },
            )
            .await;
        if !resp.success {
            warn!(
                file = %doc.source.display(),
                page = page_num,
                error = ?resp.error,
                "pdf page node insert failed"
            );
            continue;
        }
        pages_written += 1;
    }

    Ok(pages_written)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
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
        IpcResponse::JobQueued { job_id } => {
            println!("queued job {job_id}");
            Ok(())
        }
        IpcResponse::JobQueue { snapshot } => {
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
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
