//! Control-plane IPC.
//!
//! The CLI tool (`mneme daemon ...`) connects to the supervisor over a
//! Unix domain socket (Unix) or named pipe (Windows) and exchanges
//! length-prefixed JSON messages. The supervisor listens forever; each
//! incoming connection is handled on its own task.
//!
//! Wire format (per message):
//!     `<u32 length BE>` `<JSON body>`

use crate::child::ChildStatus;
use crate::error::SupervisorError;
use crate::job_queue::JobQueueSnapshot;
use crate::manager::{ChildManager, ChildSnapshot};
use common::jobs::{Job, JobId, JobOutcome};
use common::query::{BlastItem, GodNode, RecallHit};
use std::path::Path;
use interprocess::local_socket::tokio::{Listener, Stream};
use interprocess::local_socket::traits::tokio::Listener as _;
use interprocess::local_socket::traits::tokio::Stream as IpcStreamExt;
use interprocess::local_socket::ListenerOptions;
#[cfg(unix)]
use interprocess::local_socket::{GenericFilePath, ToFsName};
#[cfg(windows)]
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

/// Commands accepted by the IPC server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    /// Liveness probe.
    Ping,
    /// Return per-child status snapshots.
    Status,
    /// Return the last `n` log entries (optionally filtered by child).
    Logs {
        /// Optional child filter.
        child: Option<String>,
        /// Number of entries to return.
        n: usize,
    },
    /// Restart a single child.
    Restart {
        /// Child name to restart.
        child: String,
    },
    /// Restart every child (rolling).
    RestartAll,
    /// Stop the supervisor (graceful shutdown).
    Stop,
    /// Update a heartbeat for a specific child (called by workers).
    Heartbeat {
        /// Child name reporting the heartbeat.
        child: String,
    },
    /// Route a job payload to the worker pool whose names share `pool`
    /// as a prefix (e.g. `"parser-worker-"`, `"scanner-worker-"`, or
    /// `"brain-worker"`). The daemon writes `payload` as a JSON line to
    /// the selected worker's stdin. Used by `mneme build` and the
    /// scanner/brain orchestrators so the CLI does not have to run parse
    /// / scan / embed work inline.
    Dispatch {
        /// Child-name prefix identifying the pool.
        pool: String,
        /// JSON payload handed verbatim to the worker.
        payload: String,
    },
    /// (v0.3) Queue a structured `Job`. Supervisor owns routing, retry
    /// on worker crash, and back-pressure. Returns [`ControlResponse::Job
    /// Queued`] with a [`JobId`] the CLI can poll for completion.
    DispatchJob {
        /// The job to queue.
        job: Job,
    },
    /// (v0.3) Worker-side notification that a job finished. Payload is
    /// opaque — the CLI interprets it based on the original `Job` kind.
    WorkerCompleteJob {
        /// Job identifier minted by the supervisor at `DispatchJob` time.
        job_id: JobId,
        /// Outcome reported by the worker.
        outcome: JobOutcome,
    },
    /// (v0.3) Return the current job-queue snapshot (pending/in-flight
    /// counts + cumulative totals). Used by `mneme status --jobs` and
    /// the CLI wait loop.
    JobQueueStatus,
    /// (v0.3.1) Supervisor-mediated recall. Opens the project's
    /// `graph.db` shard read-only and runs the same FTS5/LIKE query the
    /// CLI's direct-DB fallback runs today. Benefit: the supervisor
    /// pools read connections and caches the prepared statement across
    /// requests.
    Recall {
        /// Project root whose shard to query.
        project: std::path::PathBuf,
        /// Free-form query string.
        query: String,
        /// Max number of hits.
        limit: usize,
        /// Optional filter (unused today; kept for wire-compat).
        #[serde(rename = "filter_type")]
        filter_type: Option<String>,
    },
    /// (v0.3.1) Supervisor-mediated blast-radius query.
    Blast {
        /// Project root whose shard to query.
        project: std::path::PathBuf,
        /// File path or fully-qualified function name.
        target: String,
        /// Max traversal depth.
        depth: usize,
    },
    /// (v0.3.1) Supervisor-mediated top-N most-connected concept query.
    GodNodes {
        /// Project root whose shard to query.
        project: std::path::PathBuf,
        /// How many nodes to return.
        n: usize,
    },
}

/// Responses sent back over the same connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum ControlResponse {
    /// Generic ack.
    Pong,
    /// Status reply.
    Status {
        /// Per-child snapshots.
        children: Vec<ChildSnapshot>,
    },
    /// Logs reply.
    Logs {
        /// Log entries (oldest-first).
        entries: Vec<crate::log_ring::LogEntry>,
    },
    /// Successful dispatch — carries the worker name the job was routed to.
    Dispatched {
        /// Worker that accepted the job.
        worker: String,
    },
    /// (v0.3) `DispatchJob` accepted — returns the opaque `JobId`.
    JobQueued {
        /// Supervisor-assigned job id.
        job_id: JobId,
    },
    /// (v0.3) Snapshot of the job queue.
    JobQueue {
        /// Queue stats.
        snapshot: JobQueueSnapshot,
    },
    /// Generic OK acknowledgement.
    Ok {
        /// Optional human-readable message.
        message: Option<String>,
    },
    /// (v0.3.1) Supervisor-mediated recall results. Shape matches what
    /// the CLI's direct-DB fallback would render so downstream printing
    /// is source-agnostic.
    RecallResults {
        /// Hits ranked by FTS5 (or LIKE fallback).
        hits: Vec<RecallHit>,
    },
    /// (v0.3.1) Supervisor-mediated blast-radius results.
    BlastResults {
        /// Dependents in BFS order.
        impacted: Vec<BlastItem>,
    },
    /// (v0.3.1) Supervisor-mediated top-N concept results.
    GodNodesResults {
        /// Nodes sorted by (degree desc, qualified_name asc).
        nodes: Vec<GodNode>,
    },
    /// Error reply.
    Error {
        /// Error message.
        message: String,
    },
}

/// IPC server. Listens on a Unix socket / Windows named pipe.
pub struct IpcServer {
    manager: Arc<ChildManager>,
    socket_path: PathBuf,
}

impl IpcServer {
    /// Construct a new IPC server.
    pub fn new(manager: Arc<ChildManager>, socket_path: PathBuf) -> Self {
        Self {
            manager,
            socket_path,
        }
    }

    /// Run the listener until `shutdown.notified()`.
    pub async fn serve(self, shutdown: Arc<Notify>) {
        // Best-effort cleanup of a stale socket file from a previous run.
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        let listener = match build_listener(&self.socket_path) {
            Ok(l) => l,
            Err(e) => {
                error!(socket = %self.socket_path.display(), error = %e, "ipc listener bind failed");
                return;
            }
        };
        info!(socket = %self.socket_path.display(), "ipc server listening");

        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    info!("ipc server shutting down");
                    break;
                }
                accept = listener.accept() => {
                    match accept {
                        Ok(stream) => {
                            let manager = self.manager.clone();
                            let sd = shutdown.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_conn(stream, manager, sd).await {
                                    warn!(error = %e, "ipc connection closed with error");
                                }
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "ipc accept failed");
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
fn build_listener(path: &PathBuf) -> Result<Listener, SupervisorError> {
    let name = path
        .as_path()
        .to_fs_name::<GenericFilePath>()
        .map_err(|e| SupervisorError::Ipc(format!("name conversion failed: {e}")))?;
    let listener = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .map_err(|e| SupervisorError::Ipc(format!("listener create failed: {e}")))?;
    Ok(listener)
}

#[cfg(windows)]
fn build_listener(path: &PathBuf) -> Result<Listener, SupervisorError> {
    let pipe_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mneme-supervisor")
        .to_string();
    let name = pipe_name
        .as_str()
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| SupervisorError::Ipc(format!("name conversion failed: {e}")))?;
    let listener = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .map_err(|e| SupervisorError::Ipc(format!("listener create failed: {e}")))?;
    Ok(listener)
}

async fn handle_conn(
    mut stream: Stream,
    manager: Arc<ChildManager>,
    shutdown: Arc<Notify>,
) -> Result<(), SupervisorError> {
    loop {
        // Read a length prefix (u32 BE).
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(SupervisorError::Io(e)),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 16 * 1024 * 1024 {
            return Err(SupervisorError::Ipc(format!("frame too large: {len}")));
        }

        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).await?;

        let cmd: ControlCommand = match serde_json::from_slice(&body) {
            Ok(c) => c,
            Err(e) => {
                let resp = ControlResponse::Error {
                    message: format!("malformed command: {e}"),
                };
                write_response(&mut stream, &resp).await?;
                continue;
            }
        };
        debug!(?cmd, "ipc command received");

        let resp = dispatch(cmd, manager.clone(), shutdown.clone()).await;
        write_response(&mut stream, &resp).await?;
    }
}

async fn dispatch(
    cmd: ControlCommand,
    manager: Arc<ChildManager>,
    shutdown: Arc<Notify>,
) -> ControlResponse {
    match cmd {
        ControlCommand::Ping => ControlResponse::Pong,
        ControlCommand::Status => ControlResponse::Status {
            children: manager.snapshot().await,
        },
        ControlCommand::Logs { child, n } => {
            let entries = manager.log_ring().tail(child.as_deref(), n);
            ControlResponse::Logs { entries }
        }
        ControlCommand::Restart { child } => {
            let names = manager.child_names().await;
            if !names.iter().any(|n| n == &child) {
                return ControlResponse::Error {
                    message: format!("unknown child: {child}"),
                };
            }
            if let Err(e) = manager.kill_child(&child).await {
                return ControlResponse::Error {
                    message: format!("kill failed: {e}"),
                };
            }
            ControlResponse::Ok {
                message: Some(format!("child '{child}' kill signalled; restart pending")),
            }
        }
        ControlCommand::RestartAll => {
            let names = manager.child_names().await;
            for n in names {
                let _ = manager.kill_child(&n).await;
            }
            ControlResponse::Ok {
                message: Some("all children kill signalled".into()),
            }
        }
        ControlCommand::Stop => {
            shutdown.notify_waiters();
            ControlResponse::Ok {
                message: Some("shutdown signalled".into()),
            }
        }
        ControlCommand::Heartbeat { child } => {
            let names = manager.child_names().await;
            if !names.iter().any(|n| n == &child) {
                return ControlResponse::Error {
                    message: format!("unknown child: {child}"),
                };
            }
            let _ = ChildStatus::Running; // keep the import alive
            manager.record_heartbeat(&child).await;
            ControlResponse::Ok { message: None }
        }
        ControlCommand::Dispatch { pool, payload } => {
            match manager.dispatch_to_pool(&pool, &payload).await {
                Ok(worker) => ControlResponse::Dispatched { worker },
                Err(e) => ControlResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        ControlCommand::DispatchJob { job } => {
            let Some(queue) = manager.job_queue().await else {
                return ControlResponse::Error {
                    message: "supervisor job queue is not attached".into(),
                };
            };
            match queue.submit(job, None) {
                Ok(job_id) => ControlResponse::JobQueued { job_id },
                Err(e) => ControlResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        ControlCommand::WorkerCompleteJob { job_id, outcome } => {
            let Some(queue) = manager.job_queue().await else {
                return ControlResponse::Error {
                    message: "supervisor job queue is not attached".into(),
                };
            };
            // Pull the telemetry we need BEFORE `complete` moves the
            // outcome into the waker channel — the worker + manager
            // update must happen even if the CLI is no longer listening.
            let duration_ms = outcome.duration_ms();
            let status = outcome.status_str();
            let worker = queue.complete(job_id, outcome);
            if let Some(name) = worker {
                manager
                    .record_job_completion(&name, job_id.0, status, duration_ms)
                    .await;
                debug!(
                    %job_id,
                    worker = %name,
                    status,
                    duration_ms,
                    "worker_complete_job recorded"
                );
            }
            ControlResponse::Ok { message: None }
        }
        ControlCommand::JobQueueStatus => {
            let Some(queue) = manager.job_queue().await else {
                return ControlResponse::Error {
                    message: "supervisor job queue is not attached".into(),
                };
            };
            ControlResponse::JobQueue {
                snapshot: queue.snapshot(),
            }
        }
        ControlCommand::Recall {
            project,
            query,
            limit,
            filter_type: _,
        } => match query_runner::run_recall(&project, &query, limit) {
            Ok(hits) => ControlResponse::RecallResults { hits },
            Err(e) => ControlResponse::Error { message: e },
        },
        ControlCommand::Blast {
            project,
            target,
            depth,
        } => match query_runner::run_blast(&project, &target, depth) {
            Ok(impacted) => ControlResponse::BlastResults { impacted },
            Err(e) => ControlResponse::Error { message: e },
        },
        ControlCommand::GodNodes { project, n } => {
            match query_runner::run_godnodes(&project, n) {
                Ok(nodes) => ControlResponse::GodNodesResults { nodes },
                Err(e) => ControlResponse::Error { message: e },
            }
        }
    }
}

/// Read-side helpers for the three new supervisor-mediated queries.
///
/// Each one opens the project's `graph.db` shard in
/// `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` mode and runs the same
/// SQL the CLI's direct-DB fallback runs today. The supervisor never
/// writes here — the per-shard writer-task invariant is preserved.
///
/// All failures are surfaced as `Err(String)` (not `SupervisorError`) so
/// the caller can forward the message verbatim in `ControlResponse::Error`
/// without losing detail.
mod query_runner {
    use super::{BlastItem, GodNode, Path, RecallHit};
    use common::ids::ProjectId;
    use common::paths::PathManager;
    use rusqlite::{Connection, OpenFlags};
    use std::collections::{HashSet, VecDeque};

    /// Resolve a project root to its `graph.db` path via `PathManager`.
    fn resolve_graph_db(project: &Path) -> Result<std::path::PathBuf, String> {
        let root = dunce::canonicalize(project)
            .unwrap_or_else(|_| project.to_path_buf());
        let id = ProjectId::from_path(&root)
            .map_err(|e| format!("cannot hash project path {}: {e}", root.display()))?;
        let paths = PathManager::default_root();
        let db = paths.project_root(&id).join("graph.db");
        if !db.exists() {
            return Err(format!(
                "graph.db not found at {}. Run `mneme build .` first.",
                db.display()
            ));
        }
        Ok(db)
    }

    fn open_ro(db: &Path) -> Result<Connection, String> {
        Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("open {}: {e}", db.display()))
    }

    /// Mirrors `cli/src/commands/recall.rs::has_nodes_fts`.
    fn has_nodes_fts(conn: &Connection) -> Result<bool, String> {
        let mut stmt = conn
            .prepare(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'nodes_fts' LIMIT 1",
            )
            .map_err(|e| format!("prep fts check: {e}"))?;
        let exists: Option<i64> = stmt.query_row([], |row| row.get(0)).ok();
        Ok(exists.is_some())
    }

    /// Identical sanitizer to `cli/src/commands/recall.rs::fts5_sanitize`.
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

    fn recall_like(
        conn: &Connection,
        query: &str,
        limit: usize,
    ) -> Result<Vec<RecallHit>, String> {
        let pattern = format!(
            "%{}%",
            query.replace('%', r"\%").replace('_', r"\_")
        );
        let sql = "
            SELECT kind, name, qualified_name, file_path, line_start
            FROM nodes
            WHERE name LIKE ?1 ESCAPE '\\' OR qualified_name LIKE ?1 ESCAPE '\\'
            ORDER BY LENGTH(qualified_name) ASC
            LIMIT ?2
        ";
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("prep like recall: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![pattern, limit as i64], |row| {
                Ok(RecallHit {
                    kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    file_path: row.get::<_, Option<String>>(3)?,
                    line_start: row.get::<_, Option<i64>>(4)?,
                })
            })
            .map_err(|e| format!("exec like recall: {e}"))?;

        let mut hits = Vec::new();
        for r in rows {
            if let Ok(h) = r {
                hits.push(h);
            }
        }
        Ok(hits)
    }

    fn recall_fts(
        conn: &Connection,
        raw: &str,
        limit: usize,
    ) -> Result<Vec<RecallHit>, String> {
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
            .map_err(|e| format!("prep fts recall: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![sanitized, limit as i64], |row| {
                Ok(RecallHit {
                    kind: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    qualified_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    file_path: row.get::<_, Option<String>>(3)?,
                    line_start: row.get::<_, Option<i64>>(4)?,
                })
            })
            .map_err(|e| format!("exec fts recall: {e}"))?;

        let mut hits = Vec::new();
        for r in rows {
            match r {
                Ok(h) => hits.push(h),
                Err(e) => return Err(format!("row map: {e}")),
            }
        }
        if hits.is_empty() {
            return recall_like(conn, raw, limit);
        }
        Ok(hits)
    }

    pub(super) fn run_recall(
        project: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<RecallHit>, String> {
        let db = resolve_graph_db(project)?;
        let conn = open_ro(&db)?;
        if has_nodes_fts(&conn)? {
            recall_fts(&conn, query, limit)
        } else {
            recall_like(&conn, query, limit)
        }
    }

    pub(super) fn run_blast(
        project: &Path,
        target: &str,
        depth: usize,
    ) -> Result<Vec<BlastItem>, String> {
        let db = resolve_graph_db(project)?;
        let conn = open_ro(&db)?;

        // Resolve target to one or more starting node qualified_names.
        let starts: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT qualified_name FROM nodes
                     WHERE qualified_name = ?1 OR name = ?1 OR file_path = ?1
                     ORDER BY CASE
                       WHEN qualified_name = ?1 THEN 0
                       WHEN name = ?1 THEN 1
                       ELSE 2
                     END
                     LIMIT 10",
                )
                .map_err(|e| format!("prep target resolve: {e}"))?;
            let rows = stmt
                .query_map(rusqlite::params![target], |row| {
                    row.get::<_, Option<String>>(0)
                })
                .map_err(|e| format!("exec target resolve: {e}"))?;
            let mut out = Vec::new();
            for r in rows {
                if let Ok(Some(q)) = r {
                    out.push(q);
                }
            }
            out
        };

        if starts.is_empty() {
            return Ok(Vec::new());
        }

        let mut visited: HashSet<String> = starts.iter().cloned().collect();
        let mut frontier: VecDeque<(String, usize)> =
            starts.iter().map(|s| (s.clone(), 0)).collect();
        let mut impacted: Vec<BlastItem> = Vec::new();

        let mut stmt = conn
            .prepare(
                "SELECT source_qualified FROM edges WHERE target_qualified = ?1
                 UNION
                 SELECT source_qualified FROM edges WHERE target_qualified IN (
                   SELECT qualified_name FROM nodes WHERE name = ?1
                 )",
            )
            .map_err(|e| format!("prep blast query: {e}"))?;

        while let Some((node, d)) = frontier.pop_front() {
            if d >= depth {
                continue;
            }
            let rows = stmt
                .query_map(rusqlite::params![node], |row| {
                    row.get::<_, Option<String>>(0)
                })
                .map_err(|e| format!("exec blast query: {e}"))?;
            for r in rows {
                if let Ok(Some(src)) = r {
                    if visited.insert(src.clone()) {
                        let next_depth = d + 1;
                        impacted.push(BlastItem {
                            qualified_name: src.clone(),
                            depth: next_depth,
                        });
                        frontier.push_back((src, next_depth));
                    }
                }
            }
        }

        Ok(impacted)
    }

    pub(super) fn run_godnodes(
        project: &Path,
        n: usize,
    ) -> Result<Vec<GodNode>, String> {
        let db = resolve_graph_db(project)?;
        let conn = open_ro(&db)?;

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
            .map_err(|e| format!("prep godnodes: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![n as i64], |row| {
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
            .map_err(|e| format!("exec godnodes: {e}"))?;

        let mut gods = Vec::new();
        for r in rows {
            if let Ok(g) = r {
                gods.push(g);
            }
        }
        Ok(gods)
    }
}

async fn write_response(
    stream: &mut Stream,
    resp: &ControlResponse,
) -> Result<(), SupervisorError> {
    let body = serde_json::to_vec(resp)?;
    let len = (body.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;
    Ok(())
}

/// Connect to the supervisor's IPC endpoint as a client.
///
/// Used by the binary's CLI subcommands and exposed publicly so other
/// workers / tests can speak the same protocol.
pub async fn connect_client(path: &PathBuf) -> Result<Stream, SupervisorError> {
    #[cfg(unix)]
    {
        let name = path
            .as_path()
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| SupervisorError::Ipc(format!("name conversion failed: {e}")))?;
        <Stream as IpcStreamExt>::connect(name)
            .await
            .map_err(|e| SupervisorError::Ipc(format!("ipc connect failed: {e}")))
    }
    #[cfg(windows)]
    {
        let pipe_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("mneme-supervisor")
            .to_string();
        let name = pipe_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| SupervisorError::Ipc(format!("name conversion failed: {e}")))?;
        <Stream as IpcStreamExt>::connect(name)
            .await
            .map_err(|e| SupervisorError::Ipc(format!("ipc connect failed: {e}")))
    }
}
