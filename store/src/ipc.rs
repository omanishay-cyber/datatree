//! IPC entry point: the store-worker binary listens on a local socket
//! and serves DB ops issued by other workers and the MCP server.

use interprocess::local_socket::traits::tokio::Listener as _;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};

use common::{error::DtResult, ids::ProjectId, layer::DbLayer, paths::PathManager};

use crate::{
    inject::InjectOptions,
    query::{Query, Write},
    Store,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Request {
    BuildOrMigrate {
        project_root: String,
        name: String,
    },
    FindByCwd {
        cwd: String,
    },
    FindByHash {
        hash: String,
    },
    QueryRows {
        q: Query,
    },
    Write {
        w: Write,
    },
    WriteBatch {
        ws: Vec<Write>,
    },
    Insert {
        project: ProjectId,
        layer: DbLayer,
        sql: String,
        params: Vec<serde_json::Value>,
        opts: InjectOptions,
    },
    Snapshot {
        project: ProjectId,
    },
    ListSnapshots {
        project: ProjectId,
    },
    Restore {
        project: ProjectId,
        snapshot: String,
    },
    IntegrityCheck {
        project: ProjectId,
    },
    Vacuum {
        project: ProjectId,
    },
    Health,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WireResponse {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

pub async fn run_listener(store: Arc<Store>) -> DtResult<()> {
    // B-L06 (2026-05-03): use the dedicated store_socket(), NOT the
    // supervisor's socket. Pre-fix, this called supervisor_socket() and
    // collided with mneme-daemon already binding it — store crashed in
    // <50ms with EADDRINUSE, hit restart budget, got marked degraded.
    // Verified on Linux VM 2026-05-03 01:14 UTC.
    //
    // B-L06.5 (2026-05-03): unlink any stale socket file left behind by
    // a prior crashed worker before binding. Without this, supervisor's
    // respawn hits EADDRINUSE on the worker's *own* prior file because
    // the file persists across process exit. Matches the supervisor's
    // own bind-site pattern in supervisor/src/ipc.rs.
    //
    // NEW-B (2026-05-04): the original B-L06.5 fix worked on Linux but
    // was broken on Windows. `to_fs_name::<GenericFilePath>` requires the
    // path to start with `\\.\pipe\` (two leading backslashes), but
    // `PathBuf::from(r"\\.\pipe\...")` normalises that to `\.\pipe\...`
    // (one leading backslash) via Windows UNC path canonicalisation.
    // The bind therefore failed with `os error 5 / "Access is denied"`
    // (or `"not a named pipe path"` depending on the input form), the
    // worker exited cleanly with code 0, the supervisor's Permanent
    // restart strategy respawned it, and the loop hit the 6-restarts-in-
    // 60s budget within milliseconds — surfacing as `status=degraded`
    // in `mneme doctor --strict`.
    //
    // The fix mirrors supervisor::ipc::build_listener's pattern: on
    // Windows the file_name component (e.g. "mneme-store") is fed to
    // `to_ns_name::<GenericNamespaced>()`, which is the platform-correct
    // API for Windows named pipes. On Unix the existing path-based bind
    // is preserved (along with the stale-socket unlink).
    let socket_path = store.paths.store_socket();
    info!(socket = %socket_path.display(), "store IPC listening");

    let listener = build_listener(&socket_path).map_err(|e| {
        // Surface the actual bind failure loudly. Pre-fix this error was
        // wrapped silently into the listener-exit warn and the worker
        // looked like it had "shut down cleanly" — degraded with no
        // detail. Now the doctor / supervisor logs include the real
        // cause on the very first restart.
        error!(socket = %socket_path.display(), error = %e, "store IPC bind failed");
        e
    })?;

    loop {
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "accept failed");
                continue;
            }
        };
        let store = store.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(conn, store).await {
                error!(error = ?e, "conn handler");
            }
        });
    }
}

/// Build the local-socket listener for the store-worker IPC endpoint.
///
/// Public so integration tests can drive the bind path directly without
/// spinning up `run_listener`'s accept loop. The returned `Listener` is
/// dropped at the end of the test which releases the pipe / removes the
/// socket file.
///
/// **Platform contract** (mirrors `supervisor::ipc::build_listener`):
///
/// - **Unix**: treats `path` as a filesystem socket. Unlinks any stale
///   socket file at that path first (idempotent — `remove_file` on a
///   non-existent path is silently ignored), then binds via
///   `to_fs_name::<GenericFilePath>()`. Required because Linux/macOS
///   `bind()` returns `EADDRINUSE` when the inode survives a previous
///   process's crash.
///
/// - **Windows**: extracts `path.file_name()` (e.g. `mneme-store`) and
///   binds via `to_ns_name::<GenericNamespaced>()`. The interprocess crate
///   prepends `\\.\pipe\` to produce a valid named-pipe address. Using
///   `to_fs_name` here would fail because Windows path normalisation in
///   `PathBuf::from(r"\\.\pipe\...")` strips one of the leading
///   backslashes, leaving a string that no longer starts with `\\.\pipe\`
///   (a hard requirement of `GenericFilePath` per its docs).
pub fn build_listener(
    path: &std::path::Path,
) -> DtResult<interprocess::local_socket::tokio::Listener> {
    use interprocess::local_socket::ListenerOptions;

    #[cfg(unix)]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        let _ = std::fs::remove_file(path);
        let name = path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| common::error::DtError::Internal(format!("name conversion: {e}")))?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| common::error::DtError::Internal(format!("listener create: {e}")))
    }

    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        let pipe_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("mneme-store")
            .to_string();
        let name = pipe_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| {
                common::error::DtError::Internal(format!("name conversion ({pipe_name}): {e}"))
            })?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| {
                common::error::DtError::Internal(format!("listener create ({pipe_name}): {e}"))
            })
    }
}

async fn handle_conn<S>(mut conn: S, store: Arc<Store>) -> std::io::Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    loop {
        let mut len_buf = [0u8; 4];
        if conn.read_exact(&mut len_buf).await.is_err() {
            return Ok(());
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 64 * 1024 * 1024 {
            return Ok(()); // cap at 64MB
        }
        let mut buf = vec![0u8; len];
        if conn.read_exact(&mut buf).await.is_err() {
            return Ok(());
        }

        let req: Request = match serde_json::from_slice(&buf) {
            Ok(r) => r,
            Err(e) => {
                let resp = WireResponse {
                    success: false,
                    data: serde_json::Value::Null,
                    error: Some(format!("decode: {}", e)),
                };
                write_response(&mut conn, &resp).await?;
                continue;
            }
        };

        let resp = handle_request(&store, req).await;
        write_response(&mut conn, &resp).await?;
    }
}

/// Collapse the repetitive `match result { Ok(v) => ok(to_value(v)), Err(e) => err(...) }`
/// boilerplate that appeared identically across ~8 dispatch arms.
///
/// Usage: `dispatch_ok_err!(some_async_call.await)`
/// The expression must be of type `Result<T, E>` where `T: Serialize` and `E: Display`.
macro_rules! dispatch_ok_err {
    ($expr:expr) => {
        match $expr {
            Ok(v) => ok(serde_json::to_value(v).unwrap_or_default()),
            Err(e) => err(e.to_string()),
        }
    };
}

/// Collapse the `wire_from_response(store.X.method(args).await)` pattern
/// used by QueryRows / Write / WriteBatch / Insert after their SQL gates.
macro_rules! dispatch_wire {
    ($expr:expr) => {
        wire_from_response($expr)
    };
}

/// Collapse the tri-state `Ok(Some(h))/Ok(None)/Err` pattern used by
/// the two finder variants (FindByCwd, FindByHash).
macro_rules! dispatch_find {
    ($expr:expr) => {
        match $expr {
            Ok(Some(h)) => ok(serde_json::to_value(h.project).unwrap_or_default()),
            Ok(None) => ok(serde_json::Value::Null),
            Err(e) => err(e.to_string()),
        }
    };
}

async fn handle_request(store: &Arc<Store>, req: Request) -> WireResponse {
    match req {
        Request::BuildOrMigrate { project_root, name } => {
            let path = std::path::PathBuf::from(&project_root);
            let project = match ProjectId::from_path(&path) {
                Ok(p) => p,
                Err(e) => return err(format!("project_id: {}", e)),
            };
            dispatch_ok_err!(store
                .builder
                .build_or_migrate(&project, &path, &name)
                .await
                .map(|h| h.project))
        }
        Request::FindByCwd { cwd } => {
            dispatch_find!(store.finder.find_by_cwd(std::path::Path::new(&cwd)).await)
        }
        Request::FindByHash { hash } => {
            dispatch_find!(store.finder.find_by_hash(&hash).await)
        }
        Request::QueryRows { q } => {
            // Audit fix (2026-05-06): CRIT-3 closed `Request::Insert`
            // but the same arbitrary-SQL vector was still wide open
            // on QueryRows/Write/WriteBatch. Apply the same allowlist
            // gate here. QueryRows is read-only; restrict to
            // `SELECT...` and `WITH...` (CTE) prefixes.
            if let Err(reason) = validate_query_sql(&q.sql) {
                return err(format!("store IPC: rejected QueryRows request: {reason}"));
            }
            dispatch_wire!(store.query.query_rows(q).await)
        }
        Request::Write { w } => {
            // Audit fix (2026-05-06): same allowlist gate as
            // QueryRows. Write accepts INSERT/UPDATE/DELETE only —
            // matches the typed mutation semantics of the
            // store::query::write() path. DDL, PRAGMA, ATTACH, etc.
            // all fail this gate.
            if let Err(reason) = validate_write_sql(&w.sql) {
                return err(format!("store IPC: rejected Write request: {reason}"));
            }
            dispatch_wire!(store.query.write(w).await)
        }
        Request::WriteBatch { ws } => {
            // Audit fix (2026-05-06): WriteBatch is the most attacker-
            // friendly variant — N statements per call, each in a
            // single transaction. Validate every entry's SQL before
            // dispatch. First failure aborts the whole batch with a
            // diagnostic so the caller sees which entry failed.
            for (idx, w) in ws.iter().enumerate() {
                if let Err(reason) = validate_write_sql(&w.sql) {
                    return err(format!(
                        "store IPC: rejected WriteBatch entry [{idx}]: {reason}"
                    ));
                }
            }
            dispatch_wire!(store.query.write_batch(ws).await)
        }
        Request::Insert {
            project,
            layer,
            sql,
            params,
            opts,
        } => {
            // CRIT-3 fix (2026-05-05 audit): the IPC `Insert` request
            // previously accepted any SQL string from any local process
            // and piped it to `conn.prepare_cached(&sql)`. The named-pipe
            // ACL is permissive by default on Windows and the Unix socket
            // has no peer-credential check, so any same-user process
            // could ATTACH a foreign DB, DROP tables, write to
            // `sqlite_master` via `PRAGMA writable_schema=1`, etc.
            //
            // Reject anything that doesn't look like a single INSERT
            // statement. The shape we accept: optional whitespace, then
            // "INSERT" (case-insensitive), and at most one trailing
            // `;` (no statement chaining). Multi-statement payloads,
            // DDL (DROP / ALTER / CREATE), connection-affecting pragmas,
            // and ATTACH / DETACH all fail this gate.
            if let Err(reason) = validate_insert_sql(&sql) {
                return err(format!("store IPC: rejected Insert request: {reason}"));
            }
            dispatch_wire!(
                store
                    .inject
                    .insert(&project, layer, &sql, params, opts)
                    .await
            )
        }
        Request::Snapshot { project } => {
            dispatch_ok_err!(store.lifecycle.snapshot(&project).await)
        }
        Request::ListSnapshots { project } => {
            dispatch_ok_err!(store.lifecycle.list_snapshots(&project).await)
        }
        Request::Restore { project, snapshot } => {
            let id = common::ids::SnapshotId::from_str(snapshot);
            match store.lifecycle.restore(&project, id).await {
                Ok(()) => {
                    // LOW fix (2026-05-05 audit): drop cached
                    // reader/writer pools so subsequent queries hit
                    // the freshly-restored shard file. Without this,
                    // r2d2 connections still point at the OLD inode
                    // (which restore renamed to .pre-restore.<ts>)
                    // and reads return data from the snapshot the
                    // user just rolled back FROM, not the one they
                    // rolled back TO. Defense-in-depth — the
                    // online_backup path inside restore() already
                    // produces a self-consistent file at the canonical
                    // path; this just makes sure callers see it.
                    store.query.invalidate_project(&project).await;
                    ok(serde_json::Value::Null)
                }
                Err(e) => err(e.to_string()),
            }
        }
        Request::IntegrityCheck { project } => {
            dispatch_ok_err!(store.lifecycle.integrity_check(&project).await)
        }
        Request::Vacuum { project } => {
            dispatch_ok_err!(store.lifecycle.vacuum(&project).await)
        }
        Request::Health => ok(serde_json::json!({ "status": "ok" })),
        Request::Shutdown => {
            // WIDE-010: cooperative graceful shutdown.
            // Trigger the one-shot bound by the main loop, then return
            // a successful WireResponse so the caller sees a clean ack.
            // The main loop is responsible for fsync + writer-task
            // teardown after observing the signal.
            let triggered = store.trigger_shutdown();
            ok(serde_json::json!({
                "shutdown": "scheduled",
                "triggered": triggered,
            }))
        }
    }
}

fn ok(data: serde_json::Value) -> WireResponse {
    WireResponse {
        success: true,
        data,
        error: None,
    }
}

fn err(msg: String) -> WireResponse {
    WireResponse {
        success: false,
        data: serde_json::Value::Null,
        error: Some(msg),
    }
}

fn wire_from_response<T: Serialize>(r: common::response::Response<T>) -> WireResponse {
    if r.success {
        WireResponse {
            success: true,
            data: serde_json::to_value(&r.data).unwrap_or_default(),
            error: None,
        }
    } else {
        WireResponse {
            success: false,
            data: serde_json::Value::Null,
            error: r.error.map(|e| format!("{}: {}", e.kind, e.message)),
        }
    }
}

async fn write_response<S: AsyncWriteExt + Unpin>(
    conn: &mut S,
    resp: &WireResponse,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(resp).unwrap_or_default();
    let len = (bytes.len() as u32).to_be_bytes();
    conn.write_all(&len).await?;
    conn.write_all(&bytes).await?;
    conn.flush().await?;
    Ok(())
}

/// Helper for non-store crates to construct a path manager.
pub fn default_paths() -> PathManager {
    PathManager::default_root()
}

/// CRIT-3 fix (2026-05-05 audit): allowlist gate on the IPC `Insert`
/// request's `sql` field. We accept exactly one well-formed INSERT
/// statement (with or without a trailing `;`). Everything else — DDL,
/// PRAGMA, ATTACH, DETACH, VACUUM, REINDEX, multi-statement chains,
/// SELECT-into payloads — is rejected.
///
/// This is intentionally narrow. The Insert request is the only IPC
/// path that takes user-supplied SQL; every other request shape uses
/// typed parameters that go through `prepare_cached` with parameter
/// bindings.
///
/// Rejected examples (each as a `code: reason` string):
/// - `DROP TABLE nodes` → "not an INSERT"
/// - `INSERT INTO x VALUES (1); DROP TABLE y` → "multiple statements"
/// - `INSERT INTO x VALUES (1) -- DROP TABLE y` → "trailing comment"
/// - `INSERT INTO x VALUES (1) /* ; DROP TABLE y; */` → "block comment"
/// - `ATTACH DATABASE 'evil.db' AS evil` → "not an INSERT"
/// - `PRAGMA writable_schema = 1` → "not an INSERT"
fn validate_insert_sql(sql: &str) -> Result<(), &'static str> {
    validate_sql_for_kind(sql, SqlKind::Insert)
}

/// Post-audit (2026-05-06) extension: validators for the OTHER IPC
/// endpoints that accept user-supplied SQL.
///
/// CRIT-3 closed `Request::Insert`. The deep-audit fan-out flagged
/// that `Request::QueryRows`, `Request::Write`, and
/// `Request::WriteBatch` ALSO carry free-form `sql: String` fields
/// that flow into `prepare_cached(&sql)` with no allowlist. Same
/// attack vector (named-pipe / unix-socket has no peer-credential
/// check on Windows, any same-user process can DROP TABLE / ATTACH
/// DATABASE / PRAGMA writable_schema=1). This commit closes the
/// remaining three doors.
#[derive(Clone, Copy, Debug)]
enum SqlKind {
    /// `INSERT [OR IGNORE/REPLACE] [INTO] ... [RETURNING ...]`
    Insert,
    /// SELECT for `Request::QueryRows`. WITH-clause CTE allowed
    /// because they're read-only too.
    Select,
    /// `INSERT | UPDATE | DELETE` for `Request::Write` /
    /// `Request::WriteBatch`. Read-only DML is intentionally NOT
    /// allowed here — Write is for mutations only.
    InsertUpdateDelete,
}

fn validate_sql_for_kind(sql: &str, kind: SqlKind) -> Result<(), &'static str> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err("empty SQL");
    }

    // Strip an optional single trailing `;`.
    let body = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();

    // Audit fix (2026-05-06): accept ALL whitespace classes after the
    // leading keyword (was previously space/tab only — `INSERT\nINTO`
    // would false-negative). Use is_whitespace() so newline + CR
    // pass too.
    let body_upper: String = body.to_ascii_uppercase();
    let starts_with_kw = |kw: &str| {
        body_upper
            .strip_prefix(kw)
            .map(|rest| rest.chars().next().is_some_and(|c| c.is_whitespace()))
            .unwrap_or(false)
    };
    let prefix_ok = match kind {
        SqlKind::Insert => starts_with_kw("INSERT"),
        SqlKind::Select => {
            // `WITH ... SELECT` is a SELECT (CTE). Accept either prefix.
            starts_with_kw("SELECT") || starts_with_kw("WITH")
        }
        SqlKind::InsertUpdateDelete => {
            starts_with_kw("INSERT") || starts_with_kw("UPDATE") || starts_with_kw("DELETE")
        }
    };
    if !prefix_ok {
        return Err(match kind {
            SqlKind::Insert => "not an INSERT",
            SqlKind::Select => "not a SELECT or WITH",
            SqlKind::InsertUpdateDelete => "not an INSERT/UPDATE/DELETE",
        });
    }

    // Reject multiple statements + comments that could hide them.
    // Tracks both `--` line comments and `/* */` block comments now.
    // Audit fix (2026-05-06): the prior tokenizer skipped block
    // comments entirely — `INSERT INTO x VALUES (1) /* ; DROP TABLE y */`
    // would pass even though SQLite parses the trailing ";" inside
    // the comment as a statement separator depending on driver
    // configuration. Belt-and-suspenders: reject any `/*`.
    let mut chars = body.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                // SQL escapes a single quote by doubling it ('').
                if in_single_quote && chars.peek() == Some(&'\'') {
                    chars.next();
                } else {
                    in_single_quote = !in_single_quote;
                }
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ';' if !in_single_quote && !in_double_quote => {
                let rest: String = chars.by_ref().collect();
                if !rest.trim().is_empty() {
                    return Err("multiple statements");
                }
                break;
            }
            '-' if !in_single_quote && !in_double_quote => {
                // Block SQL line comments. Genuine DML does not need
                // them; rejecting prevents a comment from hiding a
                // chained statement.
                if chars.peek() == Some(&'-') {
                    return Err("trailing comment");
                }
            }
            '/' if !in_single_quote && !in_double_quote => {
                // Block SQL block comments. SQLite tolerates them in
                // some clients but they're unnecessary inside an IPC
                // wire payload and create a parse-difference between
                // the validator and the engine.
                if chars.peek() == Some(&'*') {
                    return Err("block comment");
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Validator wrapper for `Request::Write`.
fn validate_write_sql(sql: &str) -> Result<(), &'static str> {
    validate_sql_for_kind(sql, SqlKind::InsertUpdateDelete)
}

/// Validator wrapper for `Request::QueryRows`.
fn validate_query_sql(sql: &str) -> Result<(), &'static str> {
    validate_sql_for_kind(sql, SqlKind::Select)
}

#[cfg(test)]
mod validate_insert_sql_tests {
    use super::validate_insert_sql;

    #[test]
    fn accepts_simple_insert() {
        assert!(validate_insert_sql("INSERT INTO nodes (id) VALUES (?)").is_ok());
    }

    #[test]
    fn accepts_insert_or_ignore() {
        assert!(validate_insert_sql("insert or ignore into edges (a, b) values (?, ?)").is_ok());
    }

    #[test]
    fn accepts_with_returning_and_trailing_semicolon() {
        assert!(validate_insert_sql("INSERT INTO x (a) VALUES (?) RETURNING id;").is_ok());
    }

    #[test]
    fn rejects_drop_table() {
        assert!(validate_insert_sql("DROP TABLE nodes").is_err());
    }

    #[test]
    fn rejects_pragma() {
        assert!(validate_insert_sql("PRAGMA writable_schema = 1").is_err());
    }

    #[test]
    fn rejects_attach() {
        assert!(validate_insert_sql("ATTACH DATABASE 'evil.db' AS evil").is_err());
    }

    #[test]
    fn rejects_chained_statement() {
        assert!(validate_insert_sql("INSERT INTO x VALUES (1); DROP TABLE y").is_err());
    }

    #[test]
    fn rejects_line_comment_chain() {
        assert!(validate_insert_sql("INSERT INTO x VALUES (1) -- DROP TABLE y").is_err());
    }

    #[test]
    fn allows_semicolon_inside_string_literal() {
        // Semicolon inside a quoted string is part of the data, not a
        // statement separator.
        assert!(validate_insert_sql("INSERT INTO x (msg) VALUES ('hello;world')").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_insert_sql("   ").is_err());
    }

    // Audit fix (2026-05-06): coverage for the new validator surface
    // and the block-comment + multi-whitespace bypasses the audit
    // flagged.

    #[test]
    fn rejects_block_comment_in_insert() {
        assert!(validate_insert_sql("INSERT INTO x VALUES (1) /* DROP TABLE y */").is_err());
    }

    #[test]
    fn accepts_newline_after_insert_keyword() {
        // Whitespace after INSERT must accept all whitespace classes
        // (newline, tab, CR), not just space/tab.
        assert!(validate_insert_sql("INSERT\nINTO x VALUES (1)").is_ok());
        assert!(validate_insert_sql("INSERT\r\nINTO x VALUES (1)").is_ok());
    }

    #[test]
    fn validate_query_sql_accepts_select() {
        use super::validate_query_sql;
        assert!(validate_query_sql("SELECT 1").is_ok());
        assert!(validate_query_sql("select id from nodes where x = ?").is_ok());
    }

    #[test]
    fn validate_query_sql_accepts_with_cte() {
        use super::validate_query_sql;
        assert!(validate_query_sql("WITH x AS (SELECT 1) SELECT * FROM x").is_ok());
    }

    #[test]
    fn validate_query_sql_rejects_insert() {
        use super::validate_query_sql;
        assert!(validate_query_sql("INSERT INTO x VALUES (1)").is_err());
    }

    #[test]
    fn validate_query_sql_rejects_drop() {
        use super::validate_query_sql;
        assert!(validate_query_sql("DROP TABLE x").is_err());
    }

    #[test]
    fn validate_query_sql_rejects_attach() {
        use super::validate_query_sql;
        assert!(validate_query_sql("ATTACH DATABASE 'evil.db' AS evil").is_err());
    }

    #[test]
    fn validate_query_sql_rejects_block_comment() {
        use super::validate_query_sql;
        assert!(validate_query_sql("SELECT 1 /* DROP TABLE x */").is_err());
    }

    #[test]
    fn validate_write_sql_accepts_insert() {
        use super::validate_write_sql;
        assert!(validate_write_sql("INSERT INTO x VALUES (1)").is_ok());
    }

    #[test]
    fn validate_write_sql_accepts_update() {
        use super::validate_write_sql;
        assert!(validate_write_sql("UPDATE x SET a = 1 WHERE b = ?").is_ok());
    }

    #[test]
    fn validate_write_sql_accepts_delete() {
        use super::validate_write_sql;
        assert!(validate_write_sql("DELETE FROM x WHERE id = ?").is_ok());
    }

    #[test]
    fn validate_write_sql_rejects_select() {
        // Write is for mutations only — SELECT should fail.
        use super::validate_write_sql;
        assert!(validate_write_sql("SELECT * FROM x").is_err());
    }

    #[test]
    fn validate_write_sql_rejects_drop() {
        use super::validate_write_sql;
        assert!(validate_write_sql("DROP TABLE x").is_err());
    }

    #[test]
    fn validate_write_sql_rejects_attach() {
        use super::validate_write_sql;
        assert!(validate_write_sql("ATTACH DATABASE 'evil.db' AS evil").is_err());
    }

    #[test]
    fn validate_write_sql_rejects_chained_statement() {
        use super::validate_write_sql;
        assert!(validate_write_sql("UPDATE x SET a = 1; DROP TABLE y").is_err());
    }

    #[test]
    fn validate_write_sql_rejects_block_comment() {
        use super::validate_write_sql;
        assert!(
            validate_write_sql("UPDATE x SET a = 1 /* ; DROP TABLE y; */ WHERE b = ?").is_err()
        );
    }
}

/// HIGH-49 regression guard: confirms the dispatch macro refactor did not
/// accidentally remove the SQL allowlist check from any of the gated arms.
/// Each test calls the public-facing validator (same function the dispatch
/// arm calls) to guarantee the guard still fires.
///
/// These are pure-function tests — no Store is constructed, so each runs
/// in < 1 ms with zero I/O dependencies.
#[cfg(test)]
mod dispatch_macro_regression_tests {
    use super::{validate_insert_sql, validate_query_sql, validate_write_sql};

    /// QueryRows with a DROP TABLE payload must be rejected by its gate.
    /// Guards the `Request::QueryRows` arm in `handle_request`.
    #[test]
    fn query_rows_bad_sql_hits_allowlist() {
        let result = validate_query_sql("DROP TABLE nodes");
        assert!(
            result.is_err(),
            "expected allowlist rejection for QueryRows bad SQL, got Ok"
        );
        assert_eq!(result.unwrap_err(), "not a SELECT or WITH");
    }

    /// Write with a DROP TABLE payload must be rejected.
    /// Guards the `Request::Write` arm.
    #[test]
    fn write_bad_sql_hits_allowlist() {
        let result = validate_write_sql("DROP TABLE edges");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "not an INSERT/UPDATE/DELETE");
    }

    /// Insert with a PRAGMA payload must be rejected.
    /// Guards the `Request::Insert` arm.
    #[test]
    fn insert_bad_sql_hits_allowlist() {
        let result = validate_insert_sql("PRAGMA writable_schema = 1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "not an INSERT");
    }

    /// Confirm valid SQL still passes through each gate — the macro must
    /// not over-reject legitimate payloads.
    #[test]
    fn valid_sql_passes_gates() {
        assert!(validate_query_sql("SELECT id FROM nodes WHERE kind = ?").is_ok());
        assert!(validate_write_sql("UPDATE nodes SET name = ? WHERE id = ?").is_ok());
        assert!(validate_insert_sql("INSERT INTO edges (a, b) VALUES (?, ?)").is_ok());
    }
}
