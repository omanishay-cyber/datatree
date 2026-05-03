//! IPC entry point: the store-worker binary listens on a local socket
//! and serves DB ops issued by other workers and the MCP server.

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
    let socket_path = store.paths.store_socket();
    info!(socket = %socket_path.display(), "store IPC listening");

    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&socket_path);
    }

    use interprocess::local_socket::tokio::prelude::*;
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
    let name = socket_path
        .to_fs_name::<GenericFilePath>()
        .map_err(|e| common::error::DtError::Internal(e.to_string()))?;
    let listener = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .map_err(|e| common::error::DtError::Internal(e.to_string()))?;

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
                Ok(()) => ok(serde_json::Value::Null),
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
