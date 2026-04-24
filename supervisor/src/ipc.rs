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
            queue.complete(job_id, outcome);
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
