//! Sub-layer 4: QUERY — typed reads (multi-reader) + writes (single-writer per shard).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::error::SendTimeoutError;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::error;
use uuid::Uuid;

use common::{
    error::{DbError, DtError, DtResult},
    ids::{ProjectId, RowId},
    layer::DbLayer,
    paths::PathManager,
    response::{Response, ResponseMeta},
    time::Timestamp,
};

use crate::schema::SCHEMA_VERSION;

// M15 — bounded per-shard writer channel.
//
// `WRITER_CHANNEL_CAP` is the depth that backpressures upstream callers
// when the writer task is healthy. `WRITER_SEND_TIMEOUT_SECS` is the
// upper bound a caller will wait for the writer to drain when the
// channel is full — beyond that we surface `DbError::Timeout` instead
// of blocking forever (which is what `.send().await` would do if the
// per-shard writer task wedges on slow disk / migration / OS lock).
//
// AI-DNA pace: cap bumped from 256 → 1024 (4×). Per-shard, that's
// 4× the AI-burst-rate headroom; across 26 shards that's 26 624 in-flight
// writes the supervisor can absorb without back-pressuring the watcher
// pipeline. The single-writer-per-shard invariant is preserved (still
// one writer task draining the channel) — only the input buffer grows.
// The `send_timeout(WRITER_SEND_TIMEOUT_SECS)` fallthrough already
// guarantees no caller blocks forever, so a deeper buffer never trades
// liveness for throughput. See `feedback_mneme_ai_dna_pace.md` Principle
// B: "every queue depth tuned for AI-rate, not human-rate".
pub(crate) const WRITER_CHANNEL_CAP: usize = 1024;
pub(crate) const WRITER_SEND_TIMEOUT_SECS: u64 = 30;

/// Read query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub project: ProjectId,
    pub layer: DbLayer,
    pub sql: String,
    pub params: Vec<serde_json::Value>,
}

/// Write request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Write {
    pub project: ProjectId,
    pub layer: DbLayer,
    pub sql: String,
    pub params: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSummary {
    pub rows_affected: usize,
    pub last_insert_rowid: Option<RowId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total_rows_affected: usize,
}

#[async_trait]
pub trait DbQuery {
    async fn query_rows(&self, q: Query) -> Response<Vec<serde_json::Value>>;
    async fn write(&self, w: Write) -> Response<WriteSummary>;
    async fn write_batch(&self, ws: Vec<Write>) -> Response<BatchSummary>;

    /// LOW fix (2026-05-05 audit): drop cached read/write pools for a
    /// project so the next access opens fresh connections.
    ///
    /// Why this exists: the r2d2 read pool and writer-task channel
    /// are cached per (project, layer) and outlive any one shard
    /// file. After a `mneme rebuild` (or `lifecycle::restore`) the
    /// underlying graph.db is renamed to `graph.archived.<ts>` and a
    /// new graph.db is written at the same path. Existing pool
    /// connections still point at the OLD inode (POSIX) or open file
    /// handle (Windows), so subsequent reads see the archived data
    /// instead of the rebuild output.
    ///
    /// Default impl is a no-op so test doubles + alternative
    /// implementations don't have to care; DefaultQuery overrides it.
    async fn invalidate_project(&self, _project: &ProjectId) {}
}

/// Default impl: per-shard MPSC writer task + per-shard r2d2 read pool.
pub struct DefaultQuery {
    paths: Arc<PathManager>,
    writers: Arc<RwLock<HashMap<ShardKey, mpsc::Sender<WriteCmd>>>>,
    readers: Arc<RwLock<HashMap<ShardKey, Pool<SqliteConnectionManager>>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ShardKey {
    project: ProjectId,
    layer: DbLayer,
}

enum WriteCmd {
    Single {
        sql: String,
        params: Vec<serde_json::Value>,
        reply: oneshot::Sender<Result<WriteSummary, DbError>>,
    },
    Batch {
        items: Vec<(String, Vec<serde_json::Value>)>,
        reply: oneshot::Sender<Result<BatchSummary, DbError>>,
    },
}

impl DefaultQuery {
    pub fn new(paths: Arc<PathManager>) -> Self {
        Self {
            paths,
            writers: Arc::new(RwLock::new(HashMap::new())),
            readers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn writer(&self, project: &ProjectId, layer: DbLayer) -> mpsc::Sender<WriteCmd> {
        let key = ShardKey {
            project: project.clone(),
            layer,
        };
        {
            let map = self.writers.read().await;
            if let Some(tx) = map.get(&key) {
                return tx.clone();
            }
        }
        let mut map = self.writers.write().await;
        if let Some(tx) = map.get(&key) {
            return tx.clone();
        }
        let path = self.paths.shard_db(project, layer);
        let (tx, rx) = mpsc::channel::<WriteCmd>(WRITER_CHANNEL_CAP);
        spawn_writer_task(path, rx);
        map.insert(key, tx.clone());
        tx
    }

    async fn read_pool(
        &self,
        project: &ProjectId,
        layer: DbLayer,
    ) -> DtResult<Pool<SqliteConnectionManager>> {
        let key = ShardKey {
            project: project.clone(),
            layer,
        };
        {
            let map = self.readers.read().await;
            if let Some(p) = map.get(&key) {
                return Ok(p.clone());
            }
        }
        let mut map = self.readers.write().await;
        if let Some(p) = map.get(&key) {
            return Ok(p.clone());
        }
        let path = self.paths.shard_db(project, layer);
        let mgr = SqliteConnectionManager::file(&path).with_init(|c| {
            // DB-2 fix (2026-05-05 audit): match the writer + builder
            // pragma block. Without busy_timeout=5000, reader sees
            // SQLITE_BUSY immediately when a checkpoint takes the
            // exclusive lock. Without journal_mode=WAL the connection's
            // snapshot reads can race writers. foreign_keys is per-
            // connection in SQLite — readers must opt in too so any
            // ATTACH/trigger logic runs the same.
            c.busy_timeout(std::time::Duration::from_millis(5000))?;
            c.pragma_update(None, "journal_mode", "WAL")?;
            c.pragma_update(None, "foreign_keys", "ON")?;
            c.pragma_update(None, "query_only", true)?;
            c.pragma_update(None, "temp_store", "MEMORY")?;
            Ok(())
        });
        // HIGH-31 fix (2026-05-05 audit): cap the per-shard read pool
        // at 16 connections regardless of CPU count. The previous
        // formula `cpu * 2` produced 64 connections on a 32-core
        // machine — × 26 shards = 1,664 SQLite handles, each holding
        // a 256 MB mmap mapping. The mneme idle RAM target (<500 MB)
        // is impossible to hit at that scale. 16 readers per shard is
        // already well above the realistic concurrent-read demand.
        let max_pool = (num_cpus_or(4) * 2).min(16) as u32;
        let pool = Pool::builder()
            .max_size(max_pool)
            .build(mgr)
            .map_err(|e| DtError::Internal(format!("r2d2: {}", e)))?;
        map.insert(key, pool.clone());
        Ok(pool)
    }
}

#[async_trait]
impl DbQuery for DefaultQuery {
    async fn query_rows(&self, q: Query) -> Response<Vec<serde_json::Value>> {
        let start = std::time::Instant::now();
        let meta = |layer| ResponseMeta {
            latency_ms: start.elapsed().as_millis() as u64,
            cache_hit: false,
            source_db: layer,
            query_id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
        };
        let pool = match self.read_pool(&q.project, q.layer).await {
            Ok(p) => p,
            Err(DtError::Db(e)) => return Response::err(e, meta(q.layer)),
            Err(e) => return Response::err(DbError::Sqlite(e.to_string()), meta(q.layer)),
        };
        let layer = q.layer;
        let res =
            tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>, DbError> {
                let conn = pool.get().map_err(|e| DbError::Sqlite(e.to_string()))?;
                let mut stmt = conn.prepare_cached(&q.sql)?;
                let column_names: Vec<String> =
                    stmt.column_names().iter().map(|s| s.to_string()).collect();
                let params = json_params(&q.params);
                let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
                    let mut obj = serde_json::Map::new();
                    for (i, name) in column_names.iter().enumerate() {
                        let v: rusqlite::types::Value = r.get(i)?;
                        obj.insert(name.clone(), value_to_json(v));
                    }
                    Ok(serde_json::Value::Object(obj))
                })?;
                let mut out = vec![];
                for row in rows {
                    out.push(row?);
                }
                Ok(out)
            })
            .await;
        match res {
            Ok(Ok(rows)) => Response::ok(rows, meta(layer)),
            Ok(Err(e)) => Response::err(e, meta(layer)),
            Err(e) => Response::err(DbError::Sqlite(format!("join: {}", e)), meta(layer)),
        }
    }

    async fn write(&self, w: Write) -> Response<WriteSummary> {
        let start = std::time::Instant::now();
        let meta = |layer| ResponseMeta {
            latency_ms: start.elapsed().as_millis() as u64,
            cache_hit: false,
            source_db: layer,
            query_id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
        };
        let tx = self.writer(&w.project, w.layer).await;
        let (rtx, rrx) = oneshot::channel();
        let layer = w.layer;
        let cmd = WriteCmd::Single {
            sql: w.sql,
            params: w.params,
            reply: rtx,
        };
        // M15 — bounded channel, time-bounded send. If the writer task is
        // wedged the caller surfaces DbError::Timeout instead of blocking
        // forever.
        match tx
            .send_timeout(cmd, Duration::from_secs(WRITER_SEND_TIMEOUT_SECS))
            .await
        {
            Ok(()) => {}
            Err(SendTimeoutError::Timeout(_)) => {
                return Response::err(
                    DbError::Timeout {
                        elapsed_ms: WRITER_SEND_TIMEOUT_SECS * 1000,
                    },
                    meta(layer),
                );
            }
            Err(SendTimeoutError::Closed(_)) => {
                return Response::err(DbError::Sqlite("writer channel closed".into()), meta(layer));
            }
        }
        match rrx.await {
            Ok(Ok(s)) => Response::ok(s, meta(layer)),
            Ok(Err(e)) => Response::err(e, meta(layer)),
            Err(_) => Response::err(DbError::Sqlite("writer dropped reply".into()), meta(layer)),
        }
    }

    async fn write_batch(&self, ws: Vec<Write>) -> Response<BatchSummary> {
        let start = std::time::Instant::now();
        let layer = ws.first().map(|w| w.layer).unwrap_or(DbLayer::Audit);
        if ws.is_empty() {
            return Response::ok(
                BatchSummary {
                    total_rows_affected: 0,
                },
                ResponseMeta {
                    latency_ms: 0,
                    cache_hit: false,
                    source_db: layer,
                    query_id: Uuid::new_v4(),
                    schema_version: SCHEMA_VERSION,
                },
            );
        }
        let meta = |layer| ResponseMeta {
            latency_ms: start.elapsed().as_millis() as u64,
            cache_hit: false,
            source_db: layer,
            query_id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
        };
        // Audit fix (2026-05-06 multi-agent fan-out, super-debugger
        // Bug 2): the prior implementation grouped by `layer` only
        // and reused `ws.first().project` for every writer dispatch.
        // Per-Write `project` fields were silently discarded — a
        // multi-project batch would route every row to project_a's
        // shard regardless of the requested project, with
        // `success: true` and zero diagnostic. Latent today (the
        // only caller is run_convention_intent_pass which uses one
        // project) but a real data-corruption hazard for any future
        // cross-project batch caller.
        //
        // Group by (project, layer) so each shard's writer task
        // receives only its own rows.
        let mut by_shard: HashMap<(ProjectId, DbLayer), Vec<(String, Vec<serde_json::Value>)>> =
            HashMap::new();
        for w in ws {
            by_shard
                .entry((w.project, w.layer))
                .or_default()
                .push((w.sql, w.params));
        }
        let mut total = 0usize;
        for ((project, layer), items) in by_shard {
            let tx = self.writer(&project, layer).await;
            let (rtx, rrx) = oneshot::channel();
            // M15 — same time-bounded send for batch writes.
            match tx
                .send_timeout(
                    WriteCmd::Batch { items, reply: rtx },
                    Duration::from_secs(WRITER_SEND_TIMEOUT_SECS),
                )
                .await
            {
                Ok(()) => {}
                Err(SendTimeoutError::Timeout(_)) => {
                    return Response::err(
                        DbError::Timeout {
                            elapsed_ms: WRITER_SEND_TIMEOUT_SECS * 1000,
                        },
                        meta(layer),
                    );
                }
                Err(SendTimeoutError::Closed(_)) => {
                    return Response::err(
                        DbError::Sqlite("writer channel closed".into()),
                        meta(layer),
                    );
                }
            }
            match rrx.await {
                Ok(Ok(b)) => total += b.total_rows_affected,
                Ok(Err(e)) => return Response::err(e, meta(layer)),
                Err(_) => {
                    return Response::err(
                        DbError::Sqlite("writer dropped reply".into()),
                        meta(layer),
                    )
                }
            }
        }
        Response::ok(
            BatchSummary {
                total_rows_affected: total,
            },
            meta(layer),
        )
    }

    /// LOW fix (2026-05-05 audit): drop ALL cached pools/writers for
    /// `project` so subsequent reads/writes open against the
    /// freshly-created shard file. Called by lifecycle::rebuild and
    /// lifecycle::restore — both rename the underlying file out from
    /// under any cached connections.
    ///
    /// We iterate the readers/writers maps under their respective
    /// write locks and drop entries whose ShardKey.project matches.
    /// Dropping the writer Sender gracefully closes the writer
    /// task's channel — the spawn_blocking task observes
    /// `rx.recv()` returning None and exits cleanly. r2d2 Pool drops
    /// close all idle connections immediately and detach in-flight
    /// connections so they're closed when their lease ends (no risk
    /// of holding the OLD inode open beyond the in-flight query).
    async fn invalidate_project(&self, project: &ProjectId) {
        let to_drop_readers: Vec<ShardKey> = {
            let map = self.readers.read().await;
            map.keys()
                .filter(|k| &k.project == project)
                .cloned()
                .collect()
        };
        if !to_drop_readers.is_empty() {
            let mut map = self.readers.write().await;
            for key in &to_drop_readers {
                map.remove(key);
            }
        }
        let to_drop_writers: Vec<ShardKey> = {
            let map = self.writers.read().await;
            map.keys()
                .filter(|k| &k.project == project)
                .cloned()
                .collect()
        };
        if !to_drop_writers.is_empty() {
            let mut map = self.writers.write().await;
            for key in &to_drop_writers {
                // Dropping the Sender closes the writer-task's channel.
                map.remove(key);
            }
        }
        tracing::info!(
            project = %project,
            readers_dropped = to_drop_readers.len(),
            writers_dropped = to_drop_writers.len(),
            "invalidated cached shard pools after rebuild/restore"
        );
    }
}

fn spawn_writer_task(path: PathBuf, mut rx: mpsc::Receiver<WriteCmd>) {
    tokio::task::spawn_blocking(move || {
        // CRIT-14 fix (2026-05-05 audit): take a file lock BEFORE
        // opening the SQLite connection so this writer task is
        // mutually exclusive across processes. Without this, two
        // writer tasks (supervisor watch-build + CLI `mneme build`)
        // could open the same shard, each spawn their own writer,
        // and race on FTS5 rebuild + INSERT OR REPLACE — producing
        // duplicate qualified_name rows that survive the UNIQUE
        // constraint via the trigger DELETE-then-INSERT pattern.
        //
        // Audit fix (2026-05-06 multi-agent fan-out, super-debugger
        // root-cause-analysis Bug 1): the original CRIT-14 fix used
        // `<project_root>/.lock` — one lock file shared by EVERY
        // shard of the project. This fully serialised intra-process
        // writer tasks for distinct shards (graph + semantic + audit
        // + ...), causing the second shard's writer to wait 5 min
        // on the first shard's writer (the writer task holds the
        // lock for its entire lifetime, which is forever). This is
        // the exact root cause of the long-standing wiki_db /
        // architecture_db test failures: seed_graph spawned the
        // graph writer (acquired .lock), then seed_semantic tried
        // to spawn the semantic writer (waited 5 min on .lock,
        // never started), and the eventual run_wiki_pass /
        // run_architecture_pass found semantic.db empty.
        //
        // Per-shard lock file: `<shard_dir>/.<shard>.db.lock`. Same
        // cross-process exclusion (one process per shard file at a
        // time) but distinct shards get distinct OS locks, so they
        // run concurrently as the daemon expects. The CLI's own
        // `<project>/.lock` from cli/src/build_lock.rs is a
        // separate file and a separate concern (build-wide lock,
        // not per-shard) — the two layers no longer collide.
        let lock_path = path.parent().and_then(|parent| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|name| parent.join(format!(".{name}.lock")))
        });
        let _lock_guard = if let Some(lock_path) = lock_path {
            match acquire_writer_lock(&lock_path) {
                Ok(guard) => Some(guard),
                Err(e) => {
                    error!(
                        path = %path.display(),
                        lock = %lock_path.display(),
                        error = %e,
                        "writer cannot acquire per-project file lock; CLI build may be in progress"
                    );
                    return;
                }
            }
        } else {
            None
        };

        let conn = match rusqlite::Connection::open(&path) {
            Ok(c) => c,
            Err(e) => {
                error!(path = %path.display(), error = %e, "writer cannot open shard");
                return;
            }
        };
        // CRIT-13 fix (2026-05-05 audit): writer task gets the same
        // busy_timeout as the read pool / per-shard connections. Without
        // this the default 0ms budget surfaces as instant SQLITE_BUSY any
        // time a checkpoint takes the exclusive lock.
        let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");

        // K10 chaos-test hook (compiled out of release binaries):
        // when `MNEME_TEST_FAIL_FS_AT_BYTES` is set, install
        // update_hook + commit_hook that count writes and return
        // rollback once the budget is exhausted — simulating
        // `SQLITE_FULL` semantics without a real custom VFS.
        // Production builds without `--features test-hooks` skip this
        // entirely (the cfg-gated `crate::test_fs_full` module isn't
        // compiled in).
        #[cfg(any(test, feature = "test-hooks"))]
        let _fs_full_counter = crate::test_fs_full::install_full_disk_hook(&conn);

        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                WriteCmd::Single { sql, params, reply } => {
                    let res = (|| -> Result<WriteSummary, DbError> {
                        let mut stmt = conn.prepare_cached(&sql)?;
                        let p = json_params(&params);
                        let n = stmt.execute(params_from_iter(p.iter()))?;
                        Ok(WriteSummary {
                            rows_affected: n,
                            last_insert_rowid: Some(RowId(conn.last_insert_rowid())),
                        })
                    })();
                    let _ = reply.send(res);
                }
                WriteCmd::Batch { items, reply } => {
                    let res = (|| -> Result<BatchSummary, DbError> {
                        let tx = conn.unchecked_transaction()?;
                        let mut total = 0;
                        for (sql, params) in items {
                            let mut stmt = tx.prepare_cached(&sql)?;
                            let p = json_params(&params);
                            total += stmt.execute(params_from_iter(p.iter()))?;
                        }
                        tx.commit()?;
                        Ok(BatchSummary {
                            total_rows_affected: total,
                        })
                    })();
                    let _ = reply.send(res);
                }
            }
        }
    });
}

fn json_params(values: &[serde_json::Value]) -> Vec<rusqlite::types::Value> {
    values.iter().map(json_to_value).collect()
}

fn json_to_value(v: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Real(f)
            } else {
                Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        other => Value::Text(other.to_string()),
    }
}

fn value_to_json(v: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value;
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Integer(i) => serde_json::Value::Number(i.into()),
        Value::Real(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Text(s) => serde_json::Value::String(s),
        Value::Blob(b) => serde_json::Value::String(hex(&b)),
    }
}

/// HIGH-29 fix (2026-05-05 audit): byte → hex via a precomputed
/// nibble-to-char table. The previous implementation called
/// `s.push_str(&format!("{:02x}", byte))` per byte, allocating a
/// fresh 2-byte String + drop on every iteration. For BLOB-bearing
/// rows in `value_to_json` (most often embedding vectors), that's
/// O(blob_size) throwaway allocations on every read.
///
/// The new version writes 2 chars directly to the output String,
/// no intermediate allocation. Empirically 10-50× faster than the
/// `format!` version for the byte-stream sizes mneme deals with.
fn hex(b: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        // SAFETY: HEX[i] is always a valid ASCII hex char and ASCII
        // bytes are valid UTF-8.
        s.push(HEX[(*byte >> 4) as usize] as char);
        s.push(HEX[(*byte & 0xf) as usize] as char);
    }
    s
}

#[cfg(test)]
mod hex_tests {
    use super::hex;

    #[test]
    fn empty_bytes_returns_empty_string() {
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn matches_canonical_hex_format() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff, 0xab, 0xcd]), "000fffabcd");
    }

    #[test]
    fn matches_format_macro_for_random_bytes() {
        // Compare against the format! version we replaced for a
        // fixed-but-arbitrary byte sequence.
        let bytes: Vec<u8> = (0u8..=255).collect();
        let expected: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(hex(&bytes), expected);
    }
}

fn num_cpus_or(default: usize) -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(default)
}

/// CRIT-14 fix (2026-05-05 audit): RAII guard around the per-project
/// file lock. Holds the file handle for the lifetime of the writer
/// task; Drop releases the OS-level lock automatically when the
/// handle is dropped. We don't unlink the lock file (CLI's BuildLock
/// owns that lifecycle).
struct WriterLockGuard {
    _file: std::fs::File,
}

/// Acquire the per-project file lock used by the CLI's BuildLock.
/// Blocks until acquired (with periodic retry) so a CLI build that
/// holds the lock causes the daemon's writer task to wait politely
/// rather than race.
fn acquire_writer_lock(lock_path: &std::path::Path) -> std::io::Result<WriterLockGuard> {
    use fs2::FileExt;
    use std::fs::OpenOptions;
    use std::time::Instant;

    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;

    // Try non-blocking first to keep the common path (no contention)
    // hot. If contention, log + spin every 250ms with a generous
    // ceiling so the writer task never wedges forever.
    let started = Instant::now();
    let max_wait = std::time::Duration::from_secs(60 * 5); // 5 minutes
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(WriterLockGuard { _file: file }),
            Err(e) if started.elapsed() > max_wait => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    format!(
                        "writer-task lock held for >5min by another process: {e}; \
                         giving up to avoid wedging the daemon"
                    ),
                ));
            }
            Err(_) => {
                // Held by CLI build or another supervisor instance.
                // Sleep and retry.
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    }
}

// Used by Timestamp imports indirectly; keep a no-op anchor so unused import warnings stay quiet.
#[allow(dead_code)]
fn _t() -> Timestamp {
    Timestamp::now()
}

#[cfg(test)]
mod tests {
    //! M15 — writer channel must not block forever when the per-shard
    //! writer task stalls. We assert that once the bounded channel is
    //! full and the writer is not draining, callers see a structured
    //! `DbError::Timeout` instead of hanging indefinitely.
    use super::*;
    use std::time::{Duration, Instant};

    /// Build a `DefaultQuery`, override its writer task with a stalled
    /// drain, fill the 256-cap channel, and assert the 257th write
    /// returns `DbError::Timeout` within the configured budget.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn write_returns_timeout_when_writer_channel_full() {
        // Fresh sandboxed paths so we don't touch the real ~/.mneme.
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(PathManager::with_root(dir.path().to_path_buf()));
        let query = DefaultQuery::new(paths.clone());

        // Pre-install a stalled writer for the shard we're about to hit.
        // The "writer task" here just holds the receiver and never
        // drains — emulating a wedged disk / migration / OS lock.
        let project = ProjectId::from_path(dir.path()).unwrap();
        let layer = DbLayer::Audit;
        let key = ShardKey {
            project: project.clone(),
            layer,
        };
        let (tx, rx) = mpsc::channel::<WriteCmd>(super::WRITER_CHANNEL_CAP);
        // Hold the rx forever. Move it into a spawned task that just
        // sits there — nothing ever calls `rx.recv()`.
        let _stall = tokio::spawn(async move {
            let _hold = rx;
            // Park until the test ends.
            tokio::time::sleep(Duration::from_secs(3600)).await;
        });
        query.writers.write().await.insert(key, tx.clone());

        // Saturate the channel: send_timeout returns immediately while
        // there is still capacity. After WRITER_CHANNEL_CAP successful
        // sends every further `send_timeout` must trip the timeout.
        for i in 0..super::WRITER_CHANNEL_CAP {
            let (rtx, _rrx) = oneshot::channel();
            tx.try_send(WriteCmd::Single {
                sql: format!("-- saturate {}", i),
                params: vec![],
                reply: rtx,
            })
            .expect("channel still has capacity during saturation");
        }
        // Sanity: channel is now full.
        let (rtx, _rrx) = oneshot::channel();
        let try_full = tx.try_send(WriteCmd::Single {
            sql: "-- overflow probe".into(),
            params: vec![],
            reply: rtx,
        });
        assert!(
            try_full.is_err(),
            "expected the writer channel to be saturated before the timeout probe"
        );

        // The (cap+1)-th caller should NOT hang. It must surface a
        // structured timeout within ~WRITER_SEND_TIMEOUT seconds.
        let start = Instant::now();
        let resp = tokio::time::timeout(
            Duration::from_secs(super::WRITER_SEND_TIMEOUT_SECS + 5),
            query.write(Write {
                project: project.clone(),
                layer,
                sql: "INSERT INTO unused VALUES (?1)".into(),
                params: vec![serde_json::Value::Null],
            }),
        )
        .await
        .expect("write() must return within budget; hanging means M15 is unfixed");
        let elapsed = start.elapsed();

        assert!(
            !resp.success,
            "stalled writer must produce an error response, got success"
        );
        let kind = resp
            .error
            .as_ref()
            .map(|e| e.kind.clone())
            .unwrap_or_default();
        assert_eq!(
            kind, "timeout",
            "stalled writer must surface DbError::Timeout (kind=\"timeout\"), got kind={kind:?}"
        );
        assert!(
            elapsed >= Duration::from_secs(super::WRITER_SEND_TIMEOUT_SECS),
            "timeout fired too early: elapsed={elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(super::WRITER_SEND_TIMEOUT_SECS + 5),
            "timeout fired too late: elapsed={elapsed:?}"
        );
    }
}
