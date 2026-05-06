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

async fn handle_request(store: &Arc<Store>, req: Request) -> WireResponse {
    match req {
        Request::BuildOrMigrate { project_root, name } => {
            let path = std::path::PathBuf::from(&project_root);
            let project = match ProjectId::from_path(&path) {
                Ok(p) => p,
                Err(e) => return err(format!("project_id: {}", e)),
            };
            match store.builder.build_or_migrate(&project, &path, &name).await {
                Ok(h) => ok(serde_json::to_value(h.project).unwrap_or_default()),
                Err(e) => err(e.to_string()),
            }
        }
        Request::FindByCwd { cwd } => {
            match store.finder.find_by_cwd(std::path::Path::new(&cwd)).await {
                Ok(Some(h)) => ok(serde_json::to_value(h.project).unwrap_or_default()),
                Ok(None) => ok(serde_json::Value::Null),
                Err(e) => err(e.to_string()),
            }
        }
        Request::FindByHash { hash } => match store.finder.find_by_hash(&hash).await {
            Ok(Some(h)) => ok(serde_json::to_value(h.project).unwrap_or_default()),
            Ok(None) => ok(serde_json::Value::Null),
            Err(e) => err(e.to_string()),
        },
        Request::QueryRows { q } => {
            let r = store.query.query_rows(q).await;
            wire_from_response(r)
        }
        Request::Write { w } => {
            let r = store.query.write(w).await;
            wire_from_response(r)
        }
        Request::WriteBatch { ws } => {
            let r = store.query.write_batch(ws).await;
            wire_from_response(r)
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
            let r = store
                .inject
                .insert(&project, layer, &sql, params, opts)
                .await;
            wire_from_response(r)
        }
        Request::Snapshot { project } => match store.lifecycle.snapshot(&project).await {
            Ok(id) => ok(serde_json::to_value(id).unwrap_or_default()),
            Err(e) => err(e.to_string()),
        },
        Request::ListSnapshots { project } => {
            match store.lifecycle.list_snapshots(&project).await {
                Ok(s) => ok(serde_json::to_value(s).unwrap_or_default()),
                Err(e) => err(e.to_string()),
            }
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
            match store.lifecycle.integrity_check(&project).await {
                Ok(r) => ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => err(e.to_string()),
            }
        }
        Request::Vacuum { project } => match store.lifecycle.vacuum(&project).await {
            Ok(r) => ok(serde_json::to_value(r).unwrap_or_default()),
            Err(e) => err(e.to_string()),
        },
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
/// - `ATTACH DATABASE 'evil.db' AS evil` → "not an INSERT"
/// - `PRAGMA writable_schema = 1` → "not an INSERT"
fn validate_insert_sql(sql: &str) -> Result<(), &'static str> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err("empty SQL");
    }

    // Strip an optional single trailing `;`.
    let body = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();

    // Must start with INSERT (case-insensitive). `INSERT OR IGNORE` /
    // `INSERT OR REPLACE` are accepted because they still begin with
    // INSERT. `WITH ... INSERT` / `INSERT ... RETURNING` are accepted
    // (they are still single INSERT statements).
    let upper_prefix = body
        .chars()
        .take(7)
        .collect::<String>()
        .to_ascii_uppercase();
    if !upper_prefix.starts_with("INSERT ") && !upper_prefix.starts_with("INSERT\t") {
        return Err("not an INSERT");
    }

    // Reject multiple statements. SQLite tolerates a trailing `;` we
    // already stripped above. Any inner `;` followed by non-whitespace
    // is a chained statement.
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
                // Block SQL line comments. Genuine INSERT statements
                // do not need them; rejecting prevents a comment from
                // hiding a chained statement.
                if chars.peek() == Some(&'-') {
                    return Err("trailing comment");
                }
            }
            _ => {}
        }
    }

    Ok(())
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
}
