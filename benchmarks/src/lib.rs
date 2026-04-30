//! Mneme benchmark harness.
//!
//! Measures retrieval quality and token reduction of the mneme graph
//! against a naive "cold Claude" baseline (top-5 files by grep). All
//! measurements run in-process: no supervisor, no daemon, no network.
//!
//! Public surface:
//!   - [`index_repo`]       — times a full indexing pass over a project.
//!   - [`run_query_set`]    — runs a golden query set and records results.
//!   - [`compare_vs_cold`]  — compares mneme vs cold baseline per query.
//!
//! Output types are `serde::Serialize` so bin entry points can dump JSON
//! for CI consumption. No floating-point fields are emitted unless
//! strictly required (token counts are u64; times are u128 ns / u64 ms).

#![deny(unsafe_code)]
#![warn(missing_debug_implementations)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};
use parsers::{
    extractor::Extractor, incremental::IncrementalParser, parser_pool::ParserPool, query_cache,
    Language,
};
use store::{inject::InjectOptions, Store};

/// Small fixed query set used when callers want a canned 10-query workload
/// against any repo (token-reduction and incremental benches). Queries are
/// intentionally generic so the set works against arbitrary codebases.
pub const GENERIC_QUERIES: &[&str] = &[
    "error handling",
    "config",
    "database",
    "parser",
    "schema",
    "path manager",
    "logger",
    "test",
    "serialize",
    "pool",
];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// All benchmark harness errors surface through this enum.
#[derive(Debug, Error)]
pub enum BenchError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("serde_json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parser pool: {0}")]
    Pool(String),

    #[error("invalid project path: {0}")]
    InvalidPath(String),

    #[error("fixture: {0}")]
    Fixture(String),

    #[error("internal: {0}")]
    Internal(String),
}

/// Ergonomic alias.
pub type BenchResult<T> = Result<T, BenchError>;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Returned by [`index_repo`]. All fields are CI-consumable JSON primitives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexReport {
    pub project_path: String,
    pub shard_path: String,
    pub files_walked: u64,
    pub files_indexed: u64,
    pub files_skipped: u64,
    pub nodes: u64,
    pub edges: u64,
    pub elapsed_ms: u64,
}

/// One query's raw retrieval payload (mneme side).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub query: String,
    pub top_files: Vec<String>,
    pub token_count_est: u64,
    pub elapsed_ms: u64,
}

/// Aggregate over a query set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySetReport {
    pub results: Vec<QueryResult>,
    pub total_elapsed_ms: u64,
    pub total_tokens_est: u64,
}

/// One row of a head-to-head comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareRow {
    pub query: String,
    pub mneme_top: Vec<String>,
    pub mneme_tokens: u64,
    pub mneme_ms: u64,
    pub cold_top: Vec<String>,
    pub cold_tokens: u64,
    pub cold_ms: u64,
    pub expected_top: Vec<String>,
    pub mneme_precision_at_5: u32,
    pub cold_precision_at_5: u32,
}

/// Aggregate comparison over a set of golden queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareReport {
    pub rows: Vec<CompareRow>,
    pub mneme_total_tokens: u64,
    pub cold_total_tokens: u64,
    pub mneme_total_ms: u64,
    pub cold_total_ms: u64,
    /// Integer percent [0, 100] of precision@5 summed across queries.
    pub mneme_precision_pct: u32,
    pub cold_precision_pct: u32,
}

/// Golden-set entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenQuery {
    pub query: String,
    pub kind: QueryKind,
    #[serde(default)]
    pub target: Option<String>,
    pub expected_top: Vec<String>,
}

/// Which mneme retrieval op a query maps to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryKind {
    /// Generic recall: text search across node names / qualified names.
    Recall,
    /// Blast radius from a file path.
    Blast,
    /// Reverse references to a symbol (callers/importers).
    References,
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

/// Indexes a project using the same pipeline as `mneme build`:
/// walk → tree-sitter parse → Extractor → Store::inject.
///
/// Returns timing + counts. Does NOT require a running daemon.
pub async fn index_repo(project: &Path) -> BenchResult<IndexReport> {
    let project = dunce::canonicalize(project)
        .map_err(|e| BenchError::InvalidPath(format!("{}: {e}", project.display())))?;

    let start = Instant::now();

    let paths = PathManager::default_root();
    let store = Store::new(paths.clone());
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| BenchError::InvalidPath(format!("hash: {e}")))?;
    let project_name = project
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let _shard = store
        .builder
        .build_or_migrate(&project_id, &project, &project_name)
        .await
        .map_err(|e| BenchError::Internal(format!("build_or_migrate: {e}")))?;

    let pool = Arc::new(ParserPool::new(4).map_err(|e| BenchError::Pool(e.to_string()))?);
    let _ = query_cache::warm_up();
    let inc = Arc::new(IncrementalParser::new(pool.clone()));

    let walker = walkdir::WalkDir::new(&project)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()));

    let mut walked = 0u64;
    let mut indexed = 0u64;
    let mut skipped = 0u64;
    let mut node_total = 0u64;
    let mut edge_total = 0u64;

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        walked += 1;

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
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if looks_binary(&content) {
            skipped += 1;
            continue;
        }

        let content_arc = Arc::new(content);
        let parse = match inc.parse_file(path, lang, content_arc.clone()).await {
            Ok(p) => p,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let extractor = Extractor::new(lang);
        let graph = match extractor.extract(&parse.tree, &content_arc, path) {
            Ok(g) => g,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

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
        node_total += graph.nodes.len() as u64;
        edge_total += graph.edges.len() as u64;
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let shard_path = paths.project_root(&project_id).display().to_string();

    Ok(IndexReport {
        project_path: project.display().to_string(),
        shard_path,
        files_walked: walked,
        files_indexed: indexed,
        files_skipped: skipped,
        nodes: node_total,
        edges: edge_total,
        elapsed_ms,
    })
}

// ---------------------------------------------------------------------------
// Retrieval (mneme side)
// ---------------------------------------------------------------------------

/// Run an entire query set against the given shard and return per-query
/// timings + token counts.
pub fn run_query_set(
    shard_graph_db: &Path,
    queries: &[GoldenQuery],
) -> BenchResult<QuerySetReport> {
    let conn = rusqlite::Connection::open_with_flags(
        shard_graph_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let mut results = Vec::with_capacity(queries.len());
    let mut total_elapsed_ms = 0u64;
    let mut total_tokens = 0u64;

    for q in queries {
        let start = Instant::now();
        let top = run_one_query(&conn, q)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let token_count_est = estimate_tokens_from_files(&top);
        total_elapsed_ms += elapsed_ms;
        total_tokens += token_count_est;
        results.push(QueryResult {
            query: q.query.clone(),
            top_files: top,
            token_count_est,
            elapsed_ms,
        });
    }

    Ok(QuerySetReport {
        results,
        total_elapsed_ms,
        total_tokens_est: total_tokens,
    })
}

/// Dispatch one golden query to the right mneme retrieval primitive.
/// Returns up to 5 file paths, de-duplicated and ordered by relevance.
pub fn run_one_query(conn: &rusqlite::Connection, q: &GoldenQuery) -> BenchResult<Vec<String>> {
    match q.kind {
        QueryKind::Recall => recall_files(conn, &q.query),
        QueryKind::Blast => {
            let target = q.target.as_deref().unwrap_or(&q.query);
            blast_radius(conn, target)
        }
        QueryKind::References => {
            let target = q.target.as_deref().unwrap_or(&q.query);
            find_references(conn, target)
        }
    }
}

/// Text search across nodes.name + nodes.qualified_name, ranked by the
/// number of matching nodes per file.
fn recall_files(conn: &rusqlite::Connection, query: &str) -> BenchResult<Vec<String>> {
    let like = format!("%{}%", query.to_lowercase());
    let sql = "SELECT file_path, COUNT(*) AS c \
               FROM nodes \
               WHERE file_path IS NOT NULL \
                 AND (LOWER(name) LIKE ?1 OR LOWER(qualified_name) LIKE ?1) \
               GROUP BY file_path \
               ORDER BY c DESC \
               LIMIT 5";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([&like], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Blast radius: files containing nodes whose qualified_name matches the
/// target, UNION files reached via outbound edges from those nodes.
fn blast_radius(conn: &rusqlite::Connection, target: &str) -> BenchResult<Vec<String>> {
    let like = format!("%{}%", target.to_lowercase());
    let sql = "SELECT DISTINCT file_path FROM ( \
                 SELECT n.file_path FROM nodes n \
                   WHERE LOWER(n.qualified_name) LIKE ?1 AND n.file_path IS NOT NULL \
                 UNION \
                 SELECT n2.file_path FROM edges e \
                   JOIN nodes n1 ON LOWER(n1.qualified_name) LIKE ?1 \
                   JOIN nodes n2 ON n2.qualified_name = e.target_qualified \
                   WHERE n2.file_path IS NOT NULL \
               ) LIMIT 5";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([&like], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Reverse references: files containing edges pointing AT the target.
fn find_references(conn: &rusqlite::Connection, target: &str) -> BenchResult<Vec<String>> {
    let like = format!("%{}%", target.to_lowercase());
    let sql = "SELECT DISTINCT n.file_path \
               FROM edges e \
               JOIN nodes n ON n.qualified_name = e.source_qualified \
               WHERE LOWER(e.target_qualified) LIKE ?1 \
                 AND n.file_path IS NOT NULL \
               LIMIT 5";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([&like], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Cold Claude baseline
// ---------------------------------------------------------------------------

/// Approximates "cold Claude" retrieval: naive substring grep over the
/// repo, take the 5 files with the most matches. Returns files in rank
/// order + timing. No mneme shard involved.
pub fn cold_baseline(repo: &Path, query: &str) -> BenchResult<(Vec<String>, u64)> {
    let start = Instant::now();
    let needle = query.to_lowercase();

    let mut hits: Vec<(String, u64)> = Vec::new();
    for entry in walkdir::WalkDir::new(repo)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if Language::from_filename(path).is_none() {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if looks_binary(&bytes) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let count = text.to_lowercase().matches(&needle).count() as u64;
        if count > 0 {
            hits.push((path.display().to_string(), count));
        }
    }

    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.truncate(5);
    let files: Vec<String> = hits.into_iter().map(|(p, _)| p).collect();
    let elapsed = start.elapsed().as_millis() as u64;
    Ok((files, elapsed))
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

/// Run a full comparison: each query through both mneme and the cold
/// baseline, with precision\@5 computed against the golden expected list.
pub fn compare_vs_cold(
    repo: &Path,
    shard_graph_db: &Path,
    queries: &[GoldenQuery],
) -> BenchResult<CompareReport> {
    let conn = rusqlite::Connection::open_with_flags(
        shard_graph_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let mut rows = Vec::with_capacity(queries.len());
    let mut dt_tokens = 0u64;
    let mut cold_tokens = 0u64;
    let mut dt_ms = 0u64;
    let mut cold_ms = 0u64;
    let mut dt_precision_hits = 0u32;
    let mut cold_precision_hits = 0u32;
    let mut total_expected = 0u32;

    for q in queries {
        let start = Instant::now();
        let dt_top = run_one_query(&conn, q)?;
        let dt_elapsed = start.elapsed().as_millis() as u64;
        let dt_tok = estimate_tokens_from_files(&dt_top);

        let (c_top, c_elapsed) = cold_baseline(repo, &q.query)?;
        let c_tok = estimate_tokens_from_files(&c_top);

        let dt_hit = precision_at_5(&dt_top, &q.expected_top);
        let cold_hit = precision_at_5(&c_top, &q.expected_top);

        dt_tokens += dt_tok;
        cold_tokens += c_tok;
        dt_ms += dt_elapsed;
        cold_ms += c_elapsed;
        dt_precision_hits += dt_hit;
        cold_precision_hits += cold_hit;
        total_expected += q.expected_top.len().min(5) as u32;

        rows.push(CompareRow {
            query: q.query.clone(),
            mneme_top: dt_top,
            mneme_tokens: dt_tok,
            mneme_ms: dt_elapsed,
            cold_top: c_top,
            cold_tokens: c_tok,
            cold_ms: c_elapsed,
            expected_top: q.expected_top.clone(),
            mneme_precision_at_5: dt_hit,
            cold_precision_at_5: cold_hit,
        });
    }

    let mneme_precision_pct = (dt_precision_hits * 100)
        .checked_div(total_expected)
        .unwrap_or(0);
    let cold_precision_pct = (cold_precision_hits * 100)
        .checked_div(total_expected)
        .unwrap_or(0);

    Ok(CompareReport {
        rows,
        mneme_total_tokens: dt_tokens,
        cold_total_tokens: cold_tokens,
        mneme_total_ms: dt_ms,
        cold_total_ms: cold_ms,
        mneme_precision_pct,
        cold_precision_pct,
    })
}

// ---------------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------------

/// Load a golden JSON fixture from disk.
pub fn load_fixture(path: &Path) -> BenchResult<Vec<GoldenQuery>> {
    let bytes =
        std::fs::read(path).map_err(|e| BenchError::Fixture(format!("{}: {e}", path.display())))?;
    let queries: Vec<GoldenQuery> = serde_json::from_slice(&bytes)?;
    Ok(queries)
}

/// Resolve the graph.db for a given project root.
pub fn shard_graph_db(project: &Path) -> BenchResult<PathBuf> {
    let project = dunce::canonicalize(project)
        .map_err(|e| BenchError::InvalidPath(format!("{}: {e}", project.display())))?;
    let paths = PathManager::default_root();
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| BenchError::InvalidPath(format!("hash: {e}")))?;
    Ok(paths.shard_db(&project_id, DbLayer::Graph))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Count how many of `got` (first 5) appear in `expected` (first 5) as a
/// substring match on the file basename. This avoids false mismatches
/// from path separator differences across platforms.
fn precision_at_5(got: &[String], expected: &[String]) -> u32 {
    let got5: Vec<&str> = got.iter().take(5).map(|s| s.as_str()).collect();
    let exp5: Vec<&str> = expected.iter().take(5).map(|s| s.as_str()).collect();
    let mut hits = 0u32;
    for e in &exp5 {
        let needle_norm = e.replace('\\', "/").to_lowercase();
        for g in &got5 {
            let g_norm = g.replace('\\', "/").to_lowercase();
            if g_norm.contains(&needle_norm) || needle_norm.contains(&g_norm) {
                hits += 1;
                break;
            }
        }
    }
    hits
}

/// Cheap token-count estimator. The widely-used rule of thumb for
/// English-plus-code is ~4 bytes per token. We sum file sizes and divide.
/// If a file is missing the estimate for that file is 0. Never panics.
fn estimate_tokens_from_files(files: &[String]) -> u64 {
    let mut total = 0u64;
    for f in files {
        if let Ok(meta) = std::fs::metadata(f) {
            total += meta.len() / 4;
        }
    }
    total
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
            | ".mneme"
    )
}

fn looks_binary(buf: &[u8]) -> bool {
    buf.iter().take(512).any(|&b| b == 0)
}

// ---------------------------------------------------------------------------
// Extended benchmark primitives (v0.2: token-reduction, first-build,
// incremental, viz-scale, recall). Every primitive is side-effect-free
// against user network state and keeps in-process with the same store +
// parser primitives used above.
// ---------------------------------------------------------------------------

/// Aggregate for token-reduction across a query set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReductionReport {
    /// Number of queries actually measured (after filtering empties).
    pub queries: u32,
    /// Per-query ratios (cold_tokens / mneme_tokens). Clamped so that
    /// divide-by-zero yields 0.
    pub ratios: Vec<f64>,
    pub mean_ratio: f64,
    pub p50_ratio: f64,
    pub p95_ratio: f64,
    pub mneme_total_tokens: u64,
    pub cold_total_tokens: u64,
}

/// Cold + warm durations for a full `mneme build` pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstBuildReport {
    pub cold_ms: u64,
    pub warm_ms: u64,
    pub files_indexed: u64,
    pub nodes: u64,
    pub edges: u64,
}

/// Incremental inject benchmark: p50 + p95 over N single-file inject passes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalReport {
    pub samples: u32,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub mean_ms: u64,
    pub max_ms: u64,
}

/// Graph.db scaling metrics: bytes/node and bytes/edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizScaleReport {
    pub graph_db_bytes: u64,
    pub nodes: u64,
    pub edges: u64,
    /// Integer bytes-per-node (graph.db size / nodes). 0 if nodes == 0.
    pub bytes_per_node: u64,
    /// Integer bytes-per-edge (graph.db size / edges). 0 if edges == 0.
    pub bytes_per_edge: u64,
}

/// Precision@10 across a golden set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallReport {
    pub queries: u32,
    pub precision_at_10_pct: u32,
    pub hits: u32,
    pub total_expected: u32,
}

/// Compute tokens_reduced_ratio across GENERIC_QUERIES: for each query,
/// ratio = cold_tokens / mneme_tokens. Produces mean / p50 / p95.
pub fn bench_token_reduction(
    repo: &Path,
    shard_graph_db: &Path,
) -> BenchResult<TokenReductionReport> {
    let repo = dunce::canonicalize(repo)
        .map_err(|e| BenchError::InvalidPath(format!("{}: {e}", repo.display())))?;
    let conn = rusqlite::Connection::open_with_flags(
        shard_graph_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let mut ratios = Vec::with_capacity(GENERIC_QUERIES.len());
    let mut mneme_total = 0u64;
    let mut cold_total = 0u64;

    for q in GENERIC_QUERIES {
        let gq = GoldenQuery {
            query: (*q).to_string(),
            kind: QueryKind::Recall,
            target: None,
            expected_top: Vec::new(),
        };
        let mneme_files = run_one_query(&conn, &gq)?;
        let mneme_tokens = estimate_tokens_from_files(&mneme_files);
        let (cold_files, _) = cold_baseline(&repo, q)?;
        let cold_tokens = estimate_tokens_from_files(&cold_files);

        mneme_total += mneme_tokens;
        cold_total += cold_tokens;

        let ratio = if mneme_tokens == 0 {
            0.0
        } else {
            (cold_tokens as f64) / (mneme_tokens as f64)
        };
        ratios.push(ratio);
    }

    let (mean, p50, p95) = quartiles_f64(&ratios);

    Ok(TokenReductionReport {
        queries: ratios.len() as u32,
        ratios,
        mean_ratio: mean,
        p50_ratio: p50,
        p95_ratio: p95,
        mneme_total_tokens: mneme_total,
        cold_total_tokens: cold_total,
    })
}

/// Time two full `index_repo` passes: cold (delete shard first) and warm
/// (reuse the shard in place). Returns both elapsed durations.
pub async fn bench_first_build(repo: &Path) -> BenchResult<FirstBuildReport> {
    let repo = dunce::canonicalize(repo)
        .map_err(|e| BenchError::InvalidPath(format!("{}: {e}", repo.display())))?;

    let shard = shard_graph_db(&repo)?;
    // Best-effort wipe of the shard parent so the first pass is truly cold.
    if let Some(parent) = shard.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }

    let cold = index_repo(&repo).await?;
    let warm = index_repo(&repo).await?;

    Ok(FirstBuildReport {
        cold_ms: cold.elapsed_ms,
        warm_ms: warm.elapsed_ms,
        files_indexed: cold.files_indexed,
        nodes: cold.nodes,
        edges: cold.edges,
    })
}

/// Time N single-file inject passes through the Store::inject primitive.
/// Samples up to the first 100 source files in the repo. For each file we
/// run one INSERT OR REPLACE for a single node row and time the full
/// writer-task round-trip.
pub async fn bench_incremental(repo: &Path) -> BenchResult<IncrementalReport> {
    let repo = dunce::canonicalize(repo)
        .map_err(|e| BenchError::InvalidPath(format!("{}: {e}", repo.display())))?;

    let paths = PathManager::default_root();
    let store = Store::new(paths.clone());
    let project_id =
        ProjectId::from_path(&repo).map_err(|e| BenchError::InvalidPath(format!("hash: {e}")))?;
    let project_name = repo
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let _ = store
        .builder
        .build_or_migrate(&project_id, &repo, &project_name)
        .await
        .map_err(|e| BenchError::Internal(format!("build_or_migrate: {e}")))?;

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(&repo)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if Language::from_filename(path).is_none() {
            continue;
        }
        files.push(path.to_path_buf());
        if files.len() >= 100 {
            break;
        }
    }

    let mut samples: Vec<u64> = Vec::with_capacity(files.len());
    for (i, path) in files.iter().enumerate() {
        let sql = "INSERT OR REPLACE INTO nodes(kind,name,qualified_name,file_path,line_start,line_end,language,extra,updated_at) \
                   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,datetime('now'))";
        let params = vec![
            serde_json::Value::String("bench".into()),
            serde_json::Value::String(format!("bench_node_{i}")),
            serde_json::Value::String(format!("bench::node::{i}::{}", path.display())),
            serde_json::Value::String(path.display().to_string()),
            serde_json::Value::Number(1i64.into()),
            serde_json::Value::Number(1i64.into()),
            serde_json::Value::String("rust".into()),
            serde_json::Value::String("{}".into()),
        ];
        let start = Instant::now();
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
        samples.push(start.elapsed().as_millis() as u64);
    }

    if samples.is_empty() {
        return Ok(IncrementalReport {
            samples: 0,
            p50_ms: 0,
            p95_ms: 0,
            mean_ms: 0,
            max_ms: 0,
        });
    }

    let mean = (samples.iter().copied().sum::<u64>() as f64 / samples.len() as f64) as u64;
    let max = *samples.iter().max().unwrap_or(&0);
    let (_, p50, p95) = quartiles_u64(&samples);

    Ok(IncrementalReport {
        samples: samples.len() as u32,
        p50_ms: p50,
        p95_ms: p95,
        mean_ms: mean,
        max_ms: max,
    })
}

/// Measure the size of graph.db on disk relative to its node + edge count.
pub fn bench_viz_scale(shard_graph_db: &Path) -> BenchResult<VizScaleReport> {
    let size = std::fs::metadata(shard_graph_db)
        .map(|m| m.len())
        .unwrap_or(0);
    let conn = rusqlite::Connection::open_with_flags(
        shard_graph_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let nodes: u64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) as u64;
    let edges: u64 = conn
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) as u64;

    let bytes_per_node = size.checked_div(nodes).unwrap_or(0);
    let bytes_per_edge = size.checked_div(edges).unwrap_or(0);

    Ok(VizScaleReport {
        graph_db_bytes: size,
        nodes,
        edges,
        bytes_per_node,
        bytes_per_edge,
    })
}

/// Precision@10 over a golden query fixture.
pub fn bench_recall(shard_graph_db: &Path, queries: &[GoldenQuery]) -> BenchResult<RecallReport> {
    let conn = rusqlite::Connection::open_with_flags(
        shard_graph_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let mut hits = 0u32;
    let mut total_expected = 0u32;
    for q in queries {
        let top = run_one_query_top_n(&conn, q, 10)?;
        hits += precision_at_n(&top, &q.expected_top, 10);
        total_expected += q.expected_top.len().min(10) as u32;
    }
    let precision_at_10_pct = (hits * 100).checked_div(total_expected).unwrap_or(0);

    Ok(RecallReport {
        queries: queries.len() as u32,
        precision_at_10_pct,
        hits,
        total_expected,
    })
}

/// Like `run_one_query` but returns up to `n` results (not capped at 5).
pub fn run_one_query_top_n(
    conn: &rusqlite::Connection,
    q: &GoldenQuery,
    n: usize,
) -> BenchResult<Vec<String>> {
    let like_target = q.target.clone().unwrap_or_else(|| q.query.clone());
    let like = format!("%{}%", like_target.to_lowercase());
    let q_like = format!("%{}%", q.query.to_lowercase());
    let limit = n as i64;
    let sql = match q.kind {
        QueryKind::Recall => {
            "SELECT file_path FROM nodes \
             WHERE file_path IS NOT NULL \
               AND (LOWER(name) LIKE ?1 OR LOWER(qualified_name) LIKE ?1) \
             GROUP BY file_path \
             ORDER BY COUNT(*) DESC LIMIT ?2"
        }
        QueryKind::Blast => {
            "SELECT DISTINCT file_path FROM nodes \
             WHERE LOWER(qualified_name) LIKE ?1 AND file_path IS NOT NULL \
             LIMIT ?2"
        }
        QueryKind::References => {
            "SELECT DISTINCT n.file_path FROM edges e \
             JOIN nodes n ON n.qualified_name = e.source_qualified \
             WHERE LOWER(e.target_qualified) LIKE ?1 \
               AND n.file_path IS NOT NULL \
             LIMIT ?2"
        }
    };
    let mut stmt = conn.prepare(sql)?;
    let bound_like = match q.kind {
        QueryKind::Recall => &q_like,
        _ => &like,
    };
    let rows = stmt.query_map(rusqlite::params![bound_like, limit], |r| {
        r.get::<_, String>(0)
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn precision_at_n(got: &[String], expected: &[String], n: usize) -> u32 {
    let got_n: Vec<&str> = got.iter().take(n).map(|s| s.as_str()).collect();
    let exp_n: Vec<&str> = expected.iter().take(n).map(|s| s.as_str()).collect();
    let mut hits = 0u32;
    for e in &exp_n {
        let needle = e.replace('\\', "/").to_lowercase();
        for g in &got_n {
            let g_norm = g.replace('\\', "/").to_lowercase();
            if g_norm.contains(&needle) || needle.contains(&g_norm) {
                hits += 1;
                break;
            }
        }
    }
    hits
}

/// Returns (mean, p50, p95) over a slice of f64 samples. Samples are
/// copied + sorted; empty input yields all zeroes.
fn quartiles_f64(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let p50 = sorted[sorted.len() / 2];
    let p95_idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
    (mean, p50, sorted[p95_idx])
}

fn quartiles_u64(samples: &[u64]) -> (u64, u64, u64) {
    if samples.is_empty() {
        return (0, 0, 0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort();
    let mean = sorted.iter().sum::<u64>() / sorted.len() as u64;
    let p50 = sorted[sorted.len() / 2];
    let p95_idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
    (mean, p50, sorted[p95_idx])
}

// ---------------------------------------------------------------------------
// Linkage marker. Keep the `brain` crate in the dep graph so future
// semantic-recall benchmarks (v0.2) can wire it in without changing
// Cargo.toml. The marker is private and costs nothing at runtime.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
const _BRAIN_EMBED_DIM: usize = brain::EMBEDDING_DIM;
