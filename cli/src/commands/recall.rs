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
fn warn_no_embedding_model_once() {
    if !embedding_model_present() && EMBED_WARNED.set(()).is_ok() {
        eprintln!(
            "WARN: NO EMBEDDING MODEL CONFIGURED — semantic recall will degrade to keyword-only. \
             Run `mneme models install qwen-embed-0.5b` to enable."
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
        if embedding_model_present() {
            info!(
                source = "supervisor-empty->semantic",
                "recall: keyword path empty, trying semantic fallback"
            );
            match recall_semantic(&project_root, &args.query, args.limit as usize).await {
                Ok(v) => {
                    if !v.is_empty() {
                        info!(count = v.len(), "recall: semantic fallback returned hits");
                    }
                    print_hits(&v, &args.query);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "recall: semantic fallback failed");
                    // Fall through to surface the empty supervisor result.
                }
            }
        }
        info!(source = "supervisor", count = 0, "recall served");
        print_hits(&hits, &args.query);
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
    let hits = if has_nodes_fts(&conn)? {
        recall_fts(&conn, &args.query, limit)?
    } else {
        recall_like(&conn, &args.query, limit)?
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
    // Failures are logged but never surface — the user gets the original
    // empty result rather than an unrelated embedder error.
    let hits = if hits.is_empty() && embedding_model_present() {
        // Drop the read-only `conn` against graph.db before re-opening
        // it inside `recall_semantic`. Read connections don't take a
        // file lock on WAL-mode SQLite, but dropping is cleaner and
        // keeps the FD count predictable.
        drop(conn);
        match recall_semantic(&project_root, &args.query, limit).await {
            Ok(v) => {
                if !v.is_empty() {
                    info!(count = v.len(), "recall: semantic fallback returned hits");
                }
                v
            }
            Err(e) => {
                tracing::warn!(error = %e, "recall: semantic fallback failed");
                Vec::new()
            }
        }
    } else {
        hits
    };

    print_hits(&hits, &args.query);
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
async fn recall_semantic(
    project_root: &std::path::Path,
    query: &str,
    limit: usize,
) -> CliResult<Vec<Hit>> {
    let semantic_db = semantic_db_path(project_root)?;
    if !semantic_db.exists() {
        return Ok(Vec::new());
    }
    let graph_db = graph_db_path(project_root)?;
    if !graph_db.exists() {
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

    // Scan semantic.db, decode each f32 LE BLOB, score by cosine.
    let sem_conn = Connection::open_with_flags(
        &semantic_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", semantic_db.display())))?;

    let mut stmt = sem_conn
        .prepare("SELECT node_id, vector FROM embeddings WHERE node_id IS NOT NULL")
        .map_err(|e| CliError::Other(format!("prep semantic scan: {e}")))?;

    let mut top: Vec<(i64, f32)> = Vec::new();
    {
        // Scope `rows` and `stmt` so their borrows on `sem_conn` are
        // released before we open the graph.db connection below. Two
        // simultaneous read-only SQLite handles would also be fine,
        // but minimising file-descriptor lifetime is the cleaner habit.
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
            let blob: Vec<u8> = row
                .get(1)
                .map_err(|e| CliError::Other(format!("col 1: {e}")))?;
            let Some(vec) = decode_le_f32_blob(&blob, qvec.len()) else {
                continue; // dim mismatch / truncated row — skip
            };
            let score = brain::cosine_similarity(&qvec, &vec);
            if score.is_finite() {
                top.push((node_id, score));
            }
        }
    }

    // Top-K selection. Sort descending; truncate to `limit`. For typical
    // limits (10-50) over O(50k) embeddings this is microseconds.
    top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    top.truncate(limit);
    if top.is_empty() {
        return Ok(Vec::new());
    }

    // JOIN against graph.db nodes for display. Single-row lookups in a
    // loop — `id` is the primary key so each is O(log N). For limit=10
    // this is 10 indexed lookups; faster than building an IN clause and
    // re-sorting.
    let graph_conn = Connection::open_with_flags(
        &graph_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", graph_db.display())))?;

    let mut hits: Vec<Hit> = Vec::with_capacity(top.len());
    for (node_id, _score) in &top {
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
        // Missing nodes (semantic.db row references a node deleted from
        // graph.db) are silently skipped; this is benign drift.
    }
    Ok(hits)
}

/// Decode a little-endian f32 BLOB to `Vec<f32>`, returning `None` if the
/// byte length doesn't match `expected_dim * 4`. The build pipeline writes
/// vectors via `encode_le_f32_hex` + `unhex()`; this is the inverse.
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
fn recall_fts(conn: &Connection, raw: &str, limit: usize) -> CliResult<Vec<Hit>> {
    // FTS5 is sensitive to punctuation/reserved chars. Sanitize the query
    // by keeping only word characters + spaces; if nothing survives, fall
    // back to LIKE. This mirrors mcp/src/store.ts::fts5Sanitize().
    let sanitized = fts5_sanitize(raw);
    if sanitized.is_empty() {
        return recall_like(conn, raw, limit);
    }

    let sql = "
        SELECT n.kind, n.name, n.qualified_name, n.file_path, n.line_start
        FROM nodes_fts
        JOIN nodes n ON n.rowid = nodes_fts.rowid
        WHERE nodes_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
    ";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep fts recall: {e}")))?;
    let rows = stmt
        .query_map(rusqlite::params![sanitized, limit as i64], |row| {
            Ok(Hit {
                kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                file_path: row.get::<_, Option<String>>(3)?,
                line_start: row.get::<_, Option<i64>>(4)?,
            })
        })
        .map_err(|e| CliError::Other(format!("exec fts recall: {e}")))?;

    let mut hits = Vec::new();
    for r in rows {
        match r {
            Ok(h) => hits.push(h),
            Err(e) => return Err(CliError::Other(format!("row map: {e}"))),
        }
    }
    // If FTS5 returned zero (sanitized query too aggressive, no match),
    // fall back to LIKE so users don't see empty results when a simple
    // substring would match.
    if hits.is_empty() {
        return recall_like(conn, raw, limit);
    }
    Ok(hits)
}

/// LIKE fallback — slow but always works.
fn recall_like(conn: &Connection, query: &str, limit: usize) -> CliResult<Vec<Hit>> {
    let pattern = format!("%{}%", query.replace('%', r"\%").replace('_', r"\_"));
    let sql = "
        SELECT kind, name, qualified_name, file_path, line_start
        FROM nodes
        WHERE name LIKE ?1 ESCAPE '\\' OR qualified_name LIKE ?1 ESCAPE '\\'
        ORDER BY LENGTH(qualified_name) ASC
        LIMIT ?2
    ";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep like recall: {e}")))?;
    let rows = stmt
        .query_map(rusqlite::params![pattern, limit as i64], |row| {
            Ok(Hit {
                kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                file_path: row.get::<_, Option<String>>(3)?,
                line_start: row.get::<_, Option<i64>>(4)?,
            })
        })
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
    if hits.is_empty() {
        println!("no results for `{query}`");
        return;
    }
    println!("{} hit(s) for `{}`:", hits.len(), query);
    println!();
    for h in hits {
        let loc = match (&h.file_path, h.line_start) {
            (Some(f), Some(l)) if l > 0 => format!("{}:{}", f, l),
            (Some(f), _) => f.clone(),
            _ => "-".into(),
        };
        println!("  [{}] {}", h.kind, h.qualified_name);
        if h.name != h.qualified_name {
            println!("      name: {}", h.name);
        }
        println!("      {}", loc);
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
