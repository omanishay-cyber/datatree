//! `mneme recall <query>` — semantic search across the project graph.
//!
//! v0.3.1: dual-path dispatch. When the supervisor is up the CLI sends a
//! `Recall` IPC request so the daemon can service the query from its
//! warm-connection pool + prepared-statement cache. When the supervisor
//! is down (or the IPC hop fails with a connection-level error), we fall
//! back to the historical in-process `graph.db` read. The fallback is
//! verbatim the v0.3.1-initial code path so offline + supervisor-down
//! behaviour is bit-for-bit compatible.
//!
//! Search strategy (direct-DB path): prefer FTS5 (`nodes_fts` virtual
//! table, added in v0.3) for speed, fall back to a LIKE scan when the
//! FTS5 table isn't present (older shards). Both paths are read-only;
//! no write lock is taken so this is safe to run concurrently with
//! `mneme build`.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::info;

use crate::commands::build::{embedding_model_present, make_client};
use crate::commands::ipc_helpers::{
    graph_db_path, resolve_project_root, semantic_db_path, try_ipc_dispatch, IpcDispatch,
};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::query::RecallHit;

/// K3: once-per-session guard for the "no embedding model" warning.
/// Process-wide so two `mneme recall` calls from the same `mneme step`
/// session don't both nag the user. Ok to lose the lock between processes
/// — every fresh `mneme recall` invocation may print at most one warning.
static EMBED_WARNED: OnceLock<()> = OnceLock::new();

/// Print the K3 warning once per process. Idempotent.
// UX-5 (2026-05-07 audit): downgrade tone + channel — was ALL-CAPS WARN on stderr
// reading like a panic; now sentence-case advisory on stdout reserved-for-info.
fn warn_no_embedding_model_once() {
    if !embedding_model_present() && EMBED_WARNED.set(()).is_ok() {
        println!(
            "Note: no embedding model installed -- recall is keyword-only. \
             Run `mneme models install qwen-embed-0.5b` for semantic search."
        );
    }
}

/// CLI args for `mneme recall`.
#[derive(Debug, Args)]
pub struct RecallArgs {
    /// Free-form query string. Required.
    pub query: String,

    /// Restrict to one source. For v0.3.1 the only indexed source is
    /// code concepts (nodes) — future layers (decisions, conversation,
    /// todo) will accept this filter. Currently used only to suppress
    /// the default column.
    #[arg(long = "type")]
    pub kind: Option<String>,

    /// Max results to return. Clamped at parse-time to the range 1..=10000
    /// (REG-022) — a 0 limit is a no-op and unbounded values would let a
    /// pathological query fill memory before any DB-side limit triggers.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=10000))]
    pub limit: u64,

    /// Project root to query. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

// `Hit` is now the shared `common::query::RecallHit` so the same type
// flows end-to-end through IPC and the direct-DB fallback. Kept as a
// module-private alias so the existing SQL helpers don't have to be
// renamed.
type Hit = RecallHit;

/// Entry point used by `main.rs`.
///
/// Dispatch order:
///   1. Attempt IPC. If the supervisor is up we ask it to service the
///      query — lets the daemon's connection pool / statement cache
///      absorb the cost instead of re-opening `graph.db` every time.
///   2. If the supervisor is down, or the IPC round-trip surfaces an
///      IO/timeout error, fall back to the historical in-process path.
///      Any *semantic* error from the supervisor (Error response) is
///      NOT caught — that would hide real problems behind a silent
///      fallback.
pub async fn run(args: RecallArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // REG-006: reject obviously-bad inputs before any IPC dispatch — an
    // empty/whitespace query produces no useful work on either path and
    // would otherwise burn a supervisor round-trip.
    if args.query.trim().is_empty() {
        return Err(CliError::Other("query must not be empty".to_string()));
    }
    // A1-029 (2026-05-04): reject queries containing NUL bytes upfront.
    // SQLite's TEXT bind uses the C-string API which truncates at NUL,
    // silently dropping any trailing query text. A user pasting
    // `foo\x00bar` would see results for `foo` only with no indication
    // why their search was incomplete.
    if args.query.contains('\0') {
        return Err(CliError::Other(
            "query contains NUL byte (\\0) -- SQLite would truncate the search; remove the NUL and retry".to_string(),
        ));
    }

    // K3: warn once per session if no embedding model is installed so
    // users aren't surprised when keyword-only results are weak.
    warn_no_embedding_model_once();

    // Project root used by both paths — resolve up-front so we don't do
    // it twice if IPC fails.
    let project_root = resolve_project_root(args.project.clone());

    // 2026-05-07 (edge-case agent W1): if neither graph.db nor
    // semantic.db exist for the resolved project, no path can return
    // hits. Surface a helpful error UP FRONT instead of letting the
    // query silently return "no results" — first-time users running
    // `mneme recall` from outside a project directory should see why
    // their search came back empty, not assume the index is broken.
    let graph_db_for_check = graph_db_path(&project_root)?;
    let semantic_db_for_check = semantic_db_path(&project_root)?;
    if !graph_db_for_check.exists() && !semantic_db_for_check.exists() {
        return Err(CliError::Other(format!(
            "no mneme index found for `{}`.\n\
             Either run `mneme build .` from inside a project, or pass \
             `--project <path>` pointing at one that has been indexed.",
            project_root.display()
        )));
    }

    // HIGH-48 (2026-05-06, 2026-05-05 audit): consolidated IPC dispatch via
    // cli::ipc_helpers::try_ipc_dispatch. Error arms are shared; success arm
    // is inline here because it is specific to recall (RecallResults variant).
    // A1-030 (2026-05-04): wire-decode errors are passed as extra_transient so
    // they fall back to direct-DB instead of surfacing.
    let client = make_client(socket_override);
    let req = IpcRequest::Recall {
        project: project_root.clone(),
        query: args.query.clone(),
        limit: args.limit as usize,
        filter_type: args.kind.clone(),
    };
    // BENCH-FIX-1 (2026-05-07): capture IPC hits instead of printing inside
    // the closure, so we can intercept empty results and run the semantic
    // fallback before the CLI returns. Without this, a running supervisor
    // shadows the fallback because `IpcDispatch::Done` returns immediately.
    let mut ipc_hits: Option<Vec<Hit>> = None;
    let outcome = try_ipc_dispatch(
        &client,
        req,
        |resp| match resp {
            IpcResponse::RecallResults { hits } => {
                ipc_hits = Some(hits);
                Some(Ok(()))
            }
            _ => None,
        },
        // A1-030: broaden fallback to malformed-wire errors. A corrupted shard
        // or wire skew surfaces as CliError::Other("decode failed: ..."); we
        // fall back to direct-DB instead of surfacing the error.
        |e| match e {
            CliError::Other(msg) => {
                msg.contains("decode")
                    || msg.contains("EOF")
                    || msg.contains("unexpected end")
                    || msg.contains("invalid utf")
            }
            _ => false,
        },
    )
    .await?;
    if outcome == IpcDispatch::Done {
        let hits = ipc_hits.unwrap_or_default();
        if !hits.is_empty() {
            info!(source = "supervisor", count = hits.len(), "recall served");
            print_hits(&hits, &args.query);
            return Ok(());
        }
        // BENCH-FIX-1: supervisor returned 0 hits (keyword-only path inside
        // the daemon also can't match multi-word NL queries). Try the
        // semantic embedding fallback before declaring "no results". Same
        // gating as the direct-DB path below: only when an embedding
        // model is installed (so we're not embedding via the noisy
        // hashing-trick fallback).
        // CLI-5 (2026-05-07 audit): pass args.kind to recall_semantic so
        // --type filter is honoured on supervisor-empty -> semantic path.
        // CLI-22 (2026-05-07 audit): capture the semantic-fallback error
        // so the empty-result print can surface a one-line hint instead
        // of silently swallowing it.
        let mut semantic_fallback_err: Option<String> = None;
        if embedding_model_present() {
            info!(
                source = "supervisor-empty->semantic",
                "recall: keyword path empty, trying semantic fallback"
            );
            match recall_semantic(
                &project_root,
                &args.query,
                args.limit as usize,
                args.kind.as_deref(),
            )
            .await
            {
                Ok(v) => {
                    if !v.is_empty() {
                        info!(count = v.len(), "recall: semantic fallback returned hits");
                    }
                    print_hits(&v, &args.query);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "recall: semantic fallback failed");
                    semantic_fallback_err = Some(e.to_string());
                    // Fall through to surface the empty supervisor result
                    // with a hint pointing at the failure.
                }
            }
        }
        info!(source = "supervisor", count = 0, "recall served");
        print_hits_with_hint(&hits, &args.query, semantic_fallback_err.as_deref());
        return Ok(());
    }

    // Direct-DB fallback — bit-for-bit the v0.3.1 behaviour.
    info!(source = "direct-db", "recall served");
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

    // Prefer FTS5 if the virtual table exists; otherwise fall back to LIKE.
    let limit = args.limit as usize;
    let kind_filter = args.kind.as_deref();
    // CLI-5 / PERF-011 (2026-05-07 audit): thread `--type` through FTS / LIKE.
    // PERF-011: when FTS5 is available, accept zero hits and let the semantic
    // fallback take over — DON'T silently degrade to a full-table LIKE scan
    // that the user never asked for. `recall_like` is reserved for shards
    // predating the FTS5 virtual table.
    let hits = if has_nodes_fts(&conn)? {
        recall_fts(&conn, &args.query, limit, kind_filter)?
    } else {
        recall_like(&conn, &args.query, limit, kind_filter)?
    };

    // BENCH-FIX-1 (2026-05-07): semantic-embedding fallback. Multi-word
    // natural-language queries ("where is DbLayer defined", "drift
    // detection") tokenize across `name` / `qualified_name` and never
    // match in FTS5 or LIKE — that's the reported 7-of-10-empty bench
    // result. Symbol-anchored embeddings (Item #117) ARE populated in
    // semantic.db but were never read on the recall path. Now they are.
    //
    // Only triggers when:
    //   1. Keyword paths returned nothing (so we're not displacing fast hits)
    //   2. An embedding model is installed (otherwise we'd embed via the
    //      hashing-trick fallback and the cosine scores would be noise)
    //   3. semantic.db exists in this project's shard
    //
    // CLI-22 (2026-05-07 audit): capture the semantic-fallback error so the
    // empty-result print can surface a one-line hint pointing at it instead
    // of silently swallowing it.
    let mut semantic_fallback_err: Option<String> = None;
    let hits = if hits.is_empty() && embedding_model_present() {
        // Drop the read-only `conn` against graph.db before re-opening
        // it inside `recall_semantic`. Read connections don't take a
        // file lock on WAL-mode SQLite, but dropping is cleaner and
        // keeps the FD count predictable.
        drop(conn);
        match recall_semantic(&project_root, &args.query, limit, kind_filter).await {
            Ok(v) => {
                if !v.is_empty() {
                    info!(count = v.len(), "recall: semantic fallback returned hits");
                }
                v
            }
            Err(e) => {
                tracing::warn!(error = %e, "recall: semantic fallback failed");
                semantic_fallback_err = Some(e.to_string());
                Vec::new()
            }
        }
    } else {
        hits
    };

    print_hits_with_hint(&hits, &args.query, semantic_fallback_err.as_deref());
    Ok(())
}

/// BENCH-FIX-1 (2026-05-07): semantic recall via BGE embeddings.
///
/// Strategy:
///   1. Embed the user's query with the same model the build pass used
///      (so query vectors live in the same space as stored vectors).
///   2. Brute-force cosine similarity against every row of
///      `semantic.db::embeddings`. SQLite has no native vector index;
///      typical project shards stay under ~50k embeddings so a linear
///      scan is fine for v0.4.1. Future versions can swap in a
///      segment-tree / HNSW index without changing this surface.
///   3. Take top-`limit` by score, JOIN against `graph.db::nodes` to
///      build human-readable [`Hit`] rows.
///
/// All failures are CliError — the caller decides whether to surface
/// or swallow them.
// CLI-5 (2026-05-07 audit): accept `kind_filter` so `--type` is honoured
// on the semantic path (was previously ignored, returning concept hits when
// the user asked for `--type decision`).
// PERF-009 (2026-05-07 audit): replaced unbounded `Vec<(id, score)>` and
// per-row `Vec<f32>` materialisation with a bounded BinaryHeap of size
// `limit` and an inline cosine that walks the BLOB without allocating.
// PERF-010 (2026-05-07 audit): batched the per-hit graph.db `query_row`
// loop into a single `WHERE id IN (...)` query when top-K >= 20.
async fn recall_semantic(
    project_root: &std::path::Path,
    query: &str,
    limit: usize,
    kind_filter: Option<&str>,
) -> CliResult<Vec<Hit>> {
    let semantic_db = semantic_db_path(project_root)?;
    if !semantic_db.exists() {
        return Ok(Vec::new());
    }
    let graph_db = graph_db_path(project_root)?;
    if !graph_db.exists() {
        return Ok(Vec::new());
    }
    if limit == 0 {
        return Ok(Vec::new());
    }

    // Embed the query — BGE is CPU-bound, run on the blocking pool so
    // the tokio runtime doesn't stall.
    let query_owned = query.to_string();
    let qvec: Vec<f32> = tokio::task::spawn_blocking(move || -> CliResult<Vec<f32>> {
        let embedder = brain::Embedder::from_default_path()
            .map_err(|e| CliError::Other(format!("embedder init: {e}")))?;
        if !embedder.is_ready() {
            return Err(CliError::Other(
                "embedder not ready (model file missing or ORT unavailable)".into(),
            ));
        }
        embedder
            .embed(&query_owned)
            .map_err(|e| CliError::Other(format!("embed query: {e}")))
    })
    .await
    .map_err(|e| CliError::Other(format!("embed: spawn_blocking join: {e}")))??;

    if qvec.len() != brain::EMBEDDING_DIM {
        return Err(CliError::Other(format!(
            "embedded query has dim {}, expected {}",
            qvec.len(),
            brain::EMBEDDING_DIM
        )));
    }

    // CLI-5: when --type is supplied, pre-fetch the set of node_ids whose
    // kind matches the filter so the cosine scan can skip ineligible rows.
    // Returning None here means "no kind filter"; Some(set) means "score
    // only nodes in this set". An empty set short-circuits to no hits.
    let kind_set: Option<std::collections::HashSet<i64>> = if let Some(k) = kind_filter {
        let graph_pre = Connection::open_with_flags(
            &graph_db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| CliError::Other(format!("open {}: {e}", graph_db.display())))?;
        let mut s = std::collections::HashSet::new();
        let mut stmt = graph_pre
            .prepare("SELECT id FROM nodes WHERE kind = ?1")
            .map_err(|e| CliError::Other(format!("prep kind filter: {e}")))?;
        let mut rows = stmt
            .query(rusqlite::params![k])
            .map_err(|e| CliError::Other(format!("exec kind filter: {e}")))?;
        while let Some(row) = rows
            .next()
            .map_err(|e| CliError::Other(format!("row read kind: {e}")))?
        {
            let id: i64 = row
                .get(0)
                .map_err(|e| CliError::Other(format!("col 0 kind: {e}")))?;
            s.insert(id);
        }
        if s.is_empty() {
            return Ok(Vec::new());
        }
        Some(s)
    } else {
        None
    };

    // Scan semantic.db, score each row by cosine — bounded heap of size
    // `limit` keeps memory at O(limit * 16B) regardless of corpus size.
    let sem_conn = Connection::open_with_flags(
        &semantic_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", semantic_db.display())))?;

    let mut stmt = sem_conn
        .prepare("SELECT node_id, vector FROM embeddings WHERE node_id IS NOT NULL")
        .map_err(|e| CliError::Other(format!("prep semantic scan: {e}")))?;

    // PERF-009: BinaryHeap<Reverse<...>> = min-heap. We push up to `limit`
    // elements, then pop-min when a better score arrives. Score wrapped in
    // OrderedF32 so partial_cmp doesn't panic on NaN (filtered out anyway).
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut heap: BinaryHeap<Reverse<(OrderedF32, i64)>> = BinaryHeap::with_capacity(limit + 1);
    {
        let mut rows = stmt
            .query([])
            .map_err(|e| CliError::Other(format!("exec semantic scan: {e}")))?;
        while let Some(row) = rows
            .next()
            .map_err(|e| CliError::Other(format!("row read: {e}")))?
        {
            let node_id: i64 = row
                .get(0)
                .map_err(|e| CliError::Other(format!("col 0: {e}")))?;
            // CLI-5: skip rows whose kind doesn't match --type before paying
            // the BLOB-decode + cosine cost.
            if let Some(set) = &kind_set {
                if !set.contains(&node_id) {
                    continue;
                }
            }
            // PERF-009: borrow the BLOB and compute cosine inline against
            // its little-endian f32 view — avoids the per-row Vec<f32>
            // allocation that drove ~75 MB peak on a 50k-shard.
            let blob: &[u8] = row
                .get_ref(1)
                .map_err(|e| CliError::Other(format!("col 1 ref: {e}")))?
                .as_blob()
                .map_err(|e| CliError::Other(format!("col 1 blob: {e}")))?;
            let Some(score) = cosine_le_f32_blob(&qvec, blob) else {
                continue; // dim mismatch / truncated row — skip
            };
            if !score.is_finite() {
                continue;
            }
            let item = Reverse((OrderedF32(score), node_id));
            if heap.len() < limit {
                heap.push(item);
            } else if let Some(Reverse((cur_min, _))) = heap.peek() {
                // Push only if the new score beats the current worst.
                if OrderedF32(score) > *cur_min {
                    heap.pop();
                    heap.push(item);
                }
            }
        }
    }

    if heap.is_empty() {
        return Ok(Vec::new());
    }

    // Drain heap into a score-descending Vec<(node_id, score)>.
    let mut top: Vec<(i64, f32)> = heap
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse((s, id))| (id, s.0))
        .collect();
    // into_sorted_vec on a min-heap of Reverse(...) yields ascending Reverse,
    // i.e. descending score. Confirm with a final sort to be defensive.
    top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // PERF-010: batch the per-hit graph.db lookup. For top.len() >= 20 a
    // single `WHERE id IN (...)` query is faster than N round trips. Below
    // that, keep the per-row form (10 round trips of indexed lookups is
    // already in the microsecond range and avoids the SQL-build overhead).
    let graph_conn = Connection::open_with_flags(
        &graph_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", graph_db.display())))?;

    let hits = if top.len() >= 20 {
        fetch_hits_batched(&graph_conn, &top)?
    } else {
        fetch_hits_per_row(&graph_conn, &top)?
    };
    Ok(hits)
}

/// PERF-010: per-row graph.db lookup — used when `top` is small enough that
/// the SQL-build overhead of an IN clause outweighs N indexed point queries.
fn fetch_hits_per_row(graph_conn: &Connection, top: &[(i64, f32)]) -> CliResult<Vec<Hit>> {
    let mut hits: Vec<Hit> = Vec::with_capacity(top.len());
    for (node_id, _score) in top {
        let row: Result<Hit, _> = graph_conn.query_row(
            "SELECT kind, name, qualified_name, file_path, line_start \
             FROM nodes WHERE id = ?1",
            rusqlite::params![node_id],
            |row| {
                Ok(Hit {
                    kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    file_path: row.get::<_, Option<String>>(3)?,
                    line_start: row.get::<_, Option<i64>>(4)?,
                })
            },
        );
        if let Ok(h) = row {
            hits.push(h);
        }
    }
    Ok(hits)
}

/// PERF-010: batched graph.db lookup via `WHERE id IN (?,?,...)`. Re-orders
/// the result to match the score-sorted `top` ordering before returning.
fn fetch_hits_batched(graph_conn: &Connection, top: &[(i64, f32)]) -> CliResult<Vec<Hit>> {
    use std::collections::HashMap;
    // Build a `?,?,...` placeholder string of the right arity. ids are i64
    // so they can also be inlined safely without an injection risk, but
    // parameter binding is the discipline.
    let placeholders = std::iter::repeat("?")
        .take(top.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, kind, name, qualified_name, file_path, line_start \
         FROM nodes WHERE id IN ({placeholders})"
    );
    let mut stmt = graph_conn
        .prepare(&sql)
        .map_err(|e| CliError::Other(format!("prep batched node fetch: {e}")))?;
    let params: Vec<&dyn rusqlite::ToSql> = top
        .iter()
        .map(|(id, _)| id as &dyn rusqlite::ToSql)
        .collect();
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let id: i64 = row.get(0)?;
            Ok((
                id,
                Hit {
                    kind: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    qualified_name: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    file_path: row.get::<_, Option<String>>(4)?,
                    line_start: row.get::<_, Option<i64>>(5)?,
                },
            ))
        })
        .map_err(|e| CliError::Other(format!("exec batched node fetch: {e}")))?;
    let mut by_id: HashMap<i64, Hit> = HashMap::with_capacity(top.len());
    for r in rows {
        let (id, h) = r.map_err(|e| CliError::Other(format!("row map: {e}")))?;
        by_id.insert(id, h);
    }
    // Re-sort to match score-descending top ordering. Missing nodes
    // (semantic.db row references a node deleted from graph.db) are
    // silently skipped — benign drift.
    let mut hits: Vec<Hit> = Vec::with_capacity(top.len());
    for (id, _score) in top {
        if let Some(h) = by_id.remove(id) {
            hits.push(h);
        }
    }
    Ok(hits)
}

/// PERF-009: total-order f32 wrapper for use inside BinaryHeap. NaN scores
/// are filtered out at the call site (via `score.is_finite()`), so within
/// the heap a strict ordering by bit-pattern would be defensive overkill —
/// `partial_cmp` is enough. We treat tied/incomparable scores as equal.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct OrderedF32(f32);
impl Eq for OrderedF32 {}
impl Ord for OrderedF32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// PERF-009: cosine similarity between a query vector and a little-endian
/// f32 BLOB, computed in a single pass without allocating an intermediate
/// `Vec<f32>`. Returns `None` if `blob.len() != qvec.len() * 4` (dim
/// mismatch / truncated row).
fn cosine_le_f32_blob(qvec: &[f32], blob: &[u8]) -> Option<f32> {
    if blob.len() != qvec.len() * 4 {
        return None;
    }
    let mut dot: f32 = 0.0;
    let mut norm_b: f32 = 0.0;
    let mut norm_a: f32 = 0.0;
    for (i, chunk) in blob.chunks_exact(4).enumerate() {
        // SAFETY: chunks_exact(4) yields exactly 4-byte slices.
        let bf = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let af = qvec[i];
        dot += af * bf;
        norm_a += af * af;
        norm_b += bf * bf;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        return Some(0.0);
    }
    Some(dot / denom)
}

/// Decode a little-endian f32 BLOB to `Vec<f32>`, returning `None` if the
/// byte length doesn't match `expected_dim * 4`. The build pipeline writes
/// vectors via `encode_le_f32_hex` + `unhex()`; this is the inverse.
// PERF-009 (2026-05-07 audit): superseded by `cosine_le_f32_blob` for the
// hot path (computes cosine inline without a Vec<f32>). Retained for tests
// that still pin the round-trip contract.
#[allow(dead_code)]
fn decode_le_f32_blob(blob: &[u8], expected_dim: usize) -> Option<Vec<f32>> {
    if blob.len() != expected_dim * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(expected_dim);
    for chunk in blob.chunks_exact(4) {
        // chunks_exact(4) guarantees len 4; safe to construct array.
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(out)
}

fn has_nodes_fts(conn: &Connection) -> CliResult<bool> {
    let mut stmt = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'nodes_fts' LIMIT 1")
        .map_err(|e| CliError::Other(format!("prep fts check: {e}")))?;
    let exists: Option<i64> = stmt.query_row([], |row| row.get(0)).ok();
    Ok(exists.is_some())
}

/// FTS5 path — fast, ranked by MATCH relevance.
// CLI-5 (2026-05-07 audit): accept `kind_filter` so `--type` is honoured
// on the direct-DB FTS5 path (was previously ignored).
// PERF-011 (2026-05-07 audit): if FTS5 returns zero hits we no longer
// silently degrade to a full-table LIKE scan — accept the empty result
// and let the caller's semantic-embedding path try next. `recall_like`
// is reserved for shards predating the FTS5 virtual table. The
// sanitized-empty fallback is also dropped: an all-punctuation query
// (e.g. `"::"`) yields no useful keyword match anyway.
fn recall_fts(
    conn: &Connection,
    raw: &str,
    limit: usize,
    kind_filter: Option<&str>,
) -> CliResult<Vec<Hit>> {
    // FTS5 is sensitive to punctuation/reserved chars. Sanitize the query
    // by keeping only word characters + spaces. If nothing survives, return
    // empty — the semantic fallback in `run` will pick up.
    let sanitized = fts5_sanitize(raw);
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }

    // CLI-5: optional `AND n.kind = ?3` clause.
    let (sql, has_kind) = match kind_filter {
        Some(_) => (
            "SELECT n.kind, n.name, n.qualified_name, n.file_path, n.line_start
             FROM nodes_fts
             JOIN nodes n ON n.rowid = nodes_fts.rowid
             WHERE nodes_fts MATCH ?1 AND n.kind = ?3
             ORDER BY rank
             LIMIT ?2",
            true,
        ),
        None => (
            "SELECT n.kind, n.name, n.qualified_name, n.file_path, n.line_start
             FROM nodes_fts
             JOIN nodes n ON n.rowid = nodes_fts.rowid
             WHERE nodes_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
            false,
        ),
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep fts recall: {e}")))?;
    let row_mapper = |row: &rusqlite::Row<'_>| {
        Ok(Hit {
            kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            file_path: row.get::<_, Option<String>>(3)?,
            line_start: row.get::<_, Option<i64>>(4)?,
        })
    };
    let rows = if has_kind {
        stmt.query_map(
            rusqlite::params![sanitized, limit as i64, kind_filter.unwrap()],
            row_mapper,
        )
    } else {
        stmt.query_map(rusqlite::params![sanitized, limit as i64], row_mapper)
    }
    .map_err(|e| CliError::Other(format!("exec fts recall: {e}")))?;

    let mut hits = Vec::new();
    for r in rows {
        match r {
            Ok(h) => hits.push(h),
            Err(e) => return Err(CliError::Other(format!("row map: {e}"))),
        }
    }
    Ok(hits)
}

/// LIKE fallback — slow but always works. Reserved for shards predating
/// the FTS5 virtual table (PERF-011, 2026-05-07 audit).
// CLI-5 (2026-05-07 audit): accept `kind_filter` so `--type` is honoured.
fn recall_like(
    conn: &Connection,
    query: &str,
    limit: usize,
    kind_filter: Option<&str>,
) -> CliResult<Vec<Hit>> {
    let pattern = format!("%{}%", query.replace('%', r"\%").replace('_', r"\_"));
    let (sql, has_kind) = match kind_filter {
        Some(_) => (
            "SELECT kind, name, qualified_name, file_path, line_start
             FROM nodes
             WHERE (name LIKE ?1 ESCAPE '\\' OR qualified_name LIKE ?1 ESCAPE '\\')
               AND kind = ?3
             ORDER BY LENGTH(qualified_name) ASC
             LIMIT ?2",
            true,
        ),
        None => (
            "SELECT kind, name, qualified_name, file_path, line_start
             FROM nodes
             WHERE name LIKE ?1 ESCAPE '\\' OR qualified_name LIKE ?1 ESCAPE '\\'
             ORDER BY LENGTH(qualified_name) ASC
             LIMIT ?2",
            false,
        ),
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep like recall: {e}")))?;
    let row_mapper = |row: &rusqlite::Row<'_>| {
        Ok(Hit {
            kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            file_path: row.get::<_, Option<String>>(3)?,
            line_start: row.get::<_, Option<i64>>(4)?,
        })
    };
    let rows = if has_kind {
        stmt.query_map(
            rusqlite::params![pattern, limit as i64, kind_filter.unwrap()],
            row_mapper,
        )
    } else {
        stmt.query_map(rusqlite::params![pattern, limit as i64], row_mapper)
    }
    .map_err(|e| CliError::Other(format!("exec like recall: {e}")))?;
    let mut hits = Vec::new();
    for h in rows.flatten() {
        hits.push(h);
    }
    Ok(hits)
}

/// Strip anything FTS5 would choke on. Keep alphanumerics + space. Collapse
/// whitespace. Mirrors mcp/src/store.ts `fts5Sanitize` for parity.
fn fts5_sanitize(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    let mut last_was_space = true;
    for c in q.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

fn print_hits(hits: &[Hit], query: &str) {
    print_hits_with_hint(hits, query, None);
}

// UX-20 (2026-05-07 audit): reordered output to lead with the human-readable
// name + file:line (the universal identifier editors open) and demote
// `qualified_name` to secondary context. The kind moves to a parenthesised
// suffix so it reads as English: `authenticate_user (function)` instead of
// `[function] src/auth/handlers::authenticate_user`.
// CLI-22 (2026-05-07 audit): when results are empty AND the semantic
// fallback errored, surface a one-line hint pointing the user at
// `mneme doctor` instead of swallowing the error.
fn print_hits_with_hint(hits: &[Hit], query: &str, semantic_err: Option<&str>) {
    if hits.is_empty() {
        println!("no results for `{query}`");
        if let Some(err) = semantic_err {
            // Trim multi-line errors to a single line for the hint; the
            // full error is already in the warn-level tracing log.
            let short = err.lines().next().unwrap_or(err).trim();
            println!(
                "note: semantic recall fallback failed ({short}). \
                 Run `mneme doctor` to diagnose."
            );
        }
        return;
    }
    println!("{} hit(s) for `{}`:", hits.len(), query);
    println!();
    for h in hits {
        // Bug #38: strip Windows long-path prefix at display boundary.
        let loc = match (&h.file_path, h.line_start) {
            (Some(f), Some(l)) if l > 0 => {
                format!("{}:{}", super::display_path(f), l)
            }
            (Some(f), _) => super::display_path(f).to_string(),
            _ => "-".into(),
        };
        // Primary line: name + kind suffix + location. `name` may be empty
        // for anonymous nodes; fall back to qualified_name in that case so
        // the row is never blank.
        let primary_name = if h.name.is_empty() {
            h.qualified_name.as_str()
        } else {
            h.name.as_str()
        };
        if h.kind.is_empty() {
            println!("  {} -- {}", primary_name, loc);
        } else {
            println!("  {} ({}) -- {}", primary_name, h.kind, loc);
        }
        // Secondary line: qualified_name only when it adds info beyond the
        // primary name (e.g. shows the module path). Kept for the developer
        // who DOES think in qualified names; not the lead-in.
        if !h.qualified_name.is_empty() && h.qualified_name != h.name {
            println!("      {}", h.qualified_name);
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts5_sanitize_strips_punctuation() {
        assert_eq!(fts5_sanitize("foo.bar"), "foo bar");
        assert_eq!(fts5_sanitize("hello, world!"), "hello world");
        assert_eq!(fts5_sanitize("   "), "");
    }

    /// BENCH-FIX-1 (2026-05-07): round-trip f32 → little-endian bytes →
    /// decoded f32 must reproduce the original values bit-for-bit. The
    /// build pipeline writes vectors via `f.to_le_bytes()`; this asserts
    /// the inverse decoder matches.
    #[test]
    fn decode_le_f32_blob_round_trip() {
        let v: Vec<f32> = vec![1.0, -2.5, 3.14159, 0.0, f32::MIN, f32::MAX];
        let mut bytes = Vec::with_capacity(v.len() * 4);
        for f in &v {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        let decoded = decode_le_f32_blob(&bytes, v.len()).expect("matching dim should decode");
        assert_eq!(decoded.len(), v.len());
        for (a, b) in v.iter().zip(decoded.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "f32 bits must round-trip");
        }
    }

    /// BENCH-FIX-1: dim mismatch (truncated row, schema drift, wrong model
    /// width) returns None instead of panicking on chunks_exact assumptions.
    #[test]
    fn decode_le_f32_blob_rejects_dim_mismatch() {
        let v: Vec<f32> = vec![1.0, 2.0, 3.0];
        let mut bytes = Vec::new();
        for f in &v {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        // Caller expects 4 floats but blob only contains 3.
        assert!(decode_le_f32_blob(&bytes, 4).is_none());
        // Caller expects 2 floats — also a mismatch (we don't truncate).
        assert!(decode_le_f32_blob(&bytes, 2).is_none());
        // Empty blob, 0 expected: edge case — accept as valid empty.
        assert_eq!(decode_le_f32_blob(&[], 0), Some(Vec::new()));
    }

    #[test]
    fn fts5_sanitize_preserves_underscores_and_alphanumerics() {
        assert_eq!(fts5_sanitize("foo_bar123"), "foo_bar123");
    }

    #[tokio::test]
    async fn empty_query_rejected_before_ipc() {
        // REG-006: an empty/whitespace query short-circuits with a clear
        // error and does NOT touch IPC.
        let args = RecallArgs {
            query: "   ".to_string(),
            kind: None,
            limit: 10,
            project: None,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme.sock"))).await;
        match r {
            Err(CliError::Other(msg)) => assert!(
                msg.contains("query must not be empty"),
                "wrong message: {msg}"
            ),
            other => panic!("expected Other(empty), got {other:?}"),
        }
    }
}
