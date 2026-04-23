//! Async client to the supervisor's control socket.
//!
//! The supervisor listens on a Unix socket (Unix) or named pipe (Windows)
//! at `~/.datatree/run/datatree-supervisor.sock`. Wire format is one
//! length-prefixed JSON message per request — we use 4-byte big-endian
//! length, then UTF-8 JSON body, matching the framing the supervisor's
//! `IpcServer` expects.
//!
//! The CLI is a *thin* IPC client. It connects, sends one request, awaits
//! one response, then drops the connection. No multiplexing, no streaming.
//! That keeps the client trivially correct and lets the supervisor pool
//! file descriptors however it likes.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{CliError, CliResult};

/// Default timeout for a single round-trip. Chosen to be generous enough
/// that even cold-start operations (`build` on a 10k-file repo) succeed,
/// while still capping total wall-clock for misbehaving supervisors.
pub const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(120);

/// One message the CLI can send to the supervisor.
///
/// Variants intentionally mirror the public CLI subcommands. The supervisor
/// dispatches each to the worker that owns the corresponding DB layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Liveness check; supervisor responds with [`IpcResponse::Pong`].
    Ping,
    /// Aggregated status: graph stats, drift findings count, last build ts.
    Status {
        /// Optional path; if `None`, the supervisor uses CWD.
        project: Option<PathBuf>,
    },
    /// Trigger an initial full build of the named project.
    Build {
        /// Absolute path to the project root.
        project: PathBuf,
        /// Skip files that haven't changed since the last build.
        incremental: bool,
    },
    /// Incremental update sweep.
    Update {
        /// Project to update.
        project: PathBuf,
    },
    /// Multimodal extraction pass.
    Graphify {
        /// Project to graphify.
        project: PathBuf,
    },
    /// Recall via embedding + FTS.
    Recall {
        /// Free-form query string.
        query: String,
        /// Optional filter: decision | conversation | concept | file | todo | constraint.
        #[serde(rename = "type")]
        kind: Option<String>,
        /// Max number of hits to return.
        limit: usize,
    },
    /// Blast-radius lookup.
    Blast {
        /// File path or fully-qualified function name.
        target: String,
        /// Max traversal depth.
        depth: usize,
    },
    /// Top-N most-connected concepts.
    GodNodes {
        /// Project to inspect.
        project: Option<PathBuf>,
        /// How many nodes to return.
        n: usize,
    },
    /// Current drift findings.
    Drift {
        /// Optional severity filter.
        severity: Option<String>,
    },
    /// Conversation history search.
    History {
        /// Free-form query.
        query: String,
        /// Optional ISO-8601 lower bound.
        since: Option<String>,
    },
    /// Force a manual snapshot of the active shard.
    Snapshot {
        /// Project to snapshot.
        project: Option<PathBuf>,
    },
    /// Run a full self-test.
    Doctor,
    /// Drop all DBs and re-parse from scratch.
    Rebuild {
        /// Project to rebuild.
        project: PathBuf,
    },
    /// Run all configured scanners.
    Audit {
        /// theme | security | a11y | perf | types | all.
        scope: String,
    },
    /// Step ledger op.
    Step {
        /// status | show | verify | complete | resume | plan-from
        op: String,
        /// Optional argument (step id, markdown path, …).
        arg: Option<String>,
    },
    /// Hook entry point: SessionStart.
    SessionPrime {
        /// Active project path.
        project: PathBuf,
        /// Session ID assigned by the host.
        session_id: String,
    },
    /// Hook entry point: UserPromptSubmit.
    Inject {
        /// User's prompt text.
        prompt: String,
        /// Session ID.
        session_id: String,
        /// Working directory at the time the hook fired.
        cwd: PathBuf,
    },
    /// Hook entry point: PreToolUse.
    PreTool {
        /// Tool name about to be invoked.
        tool: String,
        /// JSON-encoded params.
        params: String,
        /// Session ID.
        session_id: String,
    },
    /// Hook entry point: PostToolUse.
    PostTool {
        /// Tool name that ran.
        tool: String,
        /// Path to the file containing the tool's serialized result.
        result_file: PathBuf,
        /// Session ID.
        session_id: String,
    },
    /// Hook entry point: Stop (between turns).
    TurnEnd {
        /// Session ID.
        session_id: String,
    },
    /// Hook entry point: SessionEnd.
    SessionEnd {
        /// Session ID.
        session_id: String,
    },
    /// Daemon control: status | stop | logs. Legacy alias; prefer
    /// [`IpcRequest::Stop`], [`IpcRequest::Logs`], [`IpcRequest::Status`].
    Daemon {
        /// Sub-op.
        op: String,
    },
    /// Stop the supervisor (graceful shutdown). Maps to supervisor's Stop.
    Stop,
    /// Tail recent log entries. Maps to supervisor's Logs.
    Logs {
        /// Optional child filter.
        child: Option<String>,
        /// Number of entries to return.
        n: usize,
    },
    /// Restart a single child. Maps to supervisor's Restart.
    Restart {
        /// Child name to restart.
        child: String,
    },
    /// Restart every child (rolling). Maps to supervisor's RestartAll.
    RestartAll,
    /// Heartbeat report from a worker. Maps to supervisor's Heartbeat.
    Heartbeat {
        /// Child name.
        child: String,
    },
    /// Catch-all forwarder for not-yet-typed commands. The supervisor
    /// echoes the verbatim payload to the matching worker.
    Raw {
        /// Channel name (e.g. `"store.recall"`).
        channel: String,
        /// Arbitrary JSON payload.
        payload: serde_json::Value,
    },
}

/// One response from the supervisor. Wire format mirrors the supervisor's
/// own `ControlResponse` enum (tag = "response") so the CLI can speak the
/// native supervisor protocol without a translation layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Liveness pong.
    Pong,
    /// Status reply with per-child snapshots (free-form JSON so the CLI
    /// doesn't need the supervisor's ChildSnapshot type linked in).
    Status {
        /// Per-child snapshots (JSON values matching supervisor ChildSnapshot).
        children: Vec<serde_json::Value>,
    },
    /// Log-tail reply.
    Logs {
        /// Log entries (oldest first).
        entries: Vec<serde_json::Value>,
    },
    /// Generic acknowledgement.
    Ok {
        /// Optional message.
        message: Option<String>,
    },
    /// Error payload.
    Error {
        /// Human-readable error.
        message: String,
    },
}

/// Async client. Construct with [`IpcClient::connect`] and call
/// [`IpcClient::request`] for each round trip.
#[derive(Debug)]
pub struct IpcClient {
    socket_path: PathBuf,
    timeout: Duration,
}

impl IpcClient {
    /// Build a client. `socket_path` should be an absolute path to a
    /// Unix socket on Unix or a named pipe (`\\.\pipe\<name>`) on Windows.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            timeout: DEFAULT_IPC_TIMEOUT,
        }
    }

    /// Build a client using the standard runtime dir from
    /// [`crate::runtime_dir`].
    ///
    /// Discovery order:
    ///   1. `~/.datatree/supervisor.pipe` (Windows PID-scoped pipe name,
    ///      written by the running supervisor at boot)
    ///   2. Fallback to the legacy `~/.datatree/run/...sock` path.
    pub fn default_path() -> Self {
        if let Some(home) = dirs::home_dir() {
            let disco = home.join(".datatree").join("supervisor.pipe");
            if let Ok(content) = std::fs::read_to_string(&disco) {
                let p = content.trim();
                if !p.is_empty() {
                    return Self::new(std::path::PathBuf::from(p));
                }
            }
        }
        let socket_path = crate::runtime_dir().join(crate::DEFAULT_IPC_SOCKET_NAME);
        Self::new(socket_path)
    }

    /// Override the per-request timeout.
    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// Connect, send `request`, await a single response, disconnect.
    ///
    /// Returns `Err(CliError::Ipc)` if the supervisor isn't running, the
    /// socket can't be opened, or the timeout is exceeded.
    pub async fn request(&self, request: IpcRequest) -> CliResult<IpcResponse> {
        let body = serde_json::to_vec(&request)?;
        let payload = framed(&body);

        let response = tokio::time::timeout(self.timeout, self.round_trip(payload))
            .await
            .map_err(|_| CliError::Ipc(format!(
                "timeout after {:?} talking to supervisor at {}",
                self.timeout,
                self.socket_path.display()
            )))??;

        Ok(response)
    }

    /// Connect, ping, disconnect. Returns `true` iff the supervisor is up.
    pub async fn is_running(&self) -> bool {
        matches!(self.request(IpcRequest::Ping).await, Ok(IpcResponse::Pong))
    }

    async fn round_trip(&self, framed_request: Vec<u8>) -> CliResult<IpcResponse> {
        let mut stream = connect_stream(&self.socket_path).await?;

        stream
            .write_all(&framed_request)
            .await
            .map_err(|e| CliError::Ipc(format!("write failed: {e}")))?;
        stream
            .flush()
            .await
            .map_err(|e| CliError::Ipc(format!("flush failed: {e}")))?;

        // Read one length-prefixed response.
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| CliError::Ipc(format!("short read of length prefix: {e}")))?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Cap the response size so a malicious or buggy supervisor can't
        // OOM the CLI. 64 MiB is generous for status / recall payloads.
        const MAX_RESPONSE: usize = 64 * 1024 * 1024;
        if len > MAX_RESPONSE {
            return Err(CliError::Ipc(format!(
                "response too large: {len} bytes (limit {MAX_RESPONSE})"
            )));
        }

        let mut body = vec![0u8; len];
        stream
            .read_exact(&mut body)
            .await
            .map_err(|e| CliError::Ipc(format!("short read of body: {e}")))?;

        let parsed: IpcResponse = serde_json::from_slice(&body)?;
        Ok(parsed)
    }
}

/// Open one client connection to the supervisor's IPC endpoint.
///
/// On Unix the socket is addressed by filesystem path; on Windows the file
/// name component of `socket_path` is used as the pipe name (the
/// supervisor mirrors this).
async fn connect_stream(socket_path: &std::path::Path) -> CliResult<interprocess::local_socket::tokio::Stream> {
    use interprocess::local_socket::tokio::Stream;
    use interprocess::local_socket::traits::tokio::Stream as IpcStreamExt;

    #[cfg(unix)]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        let name = socket_path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| CliError::Ipc(format!("invalid socket path {}: {e}", socket_path.display())))?;
        <Stream as IpcStreamExt>::connect(name).await.map_err(|e| {
            CliError::Ipc(format!(
                "could not connect to supervisor at {}: {e}",
                socket_path.display()
            ))
        })
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        let pipe_name = socket_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("datatree-supervisor")
            .to_string();
        let name = pipe_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| CliError::Ipc(format!("invalid pipe name {pipe_name}: {e}")))?;
        <Stream as IpcStreamExt>::connect(name).await.map_err(|e| {
            CliError::Ipc(format!(
                "could not connect to supervisor pipe '{pipe_name}': {e}"
            ))
        })
    }
}

/// Frame `body` with a 4-byte big-endian length prefix.
fn framed(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_prefixes_length_big_endian() {
        let f = framed(b"hello");
        assert_eq!(&f[0..4], &[0, 0, 0, 5]);
        assert_eq!(&f[4..], b"hello");
    }

    #[test]
    fn request_round_trips_json() {
        let req = IpcRequest::Recall {
            query: "auth flow".into(),
            kind: Some("decision".into()),
            limit: 10,
        };
        let bytes = serde_json::to_vec(&req).unwrap();
        let back: IpcRequest = serde_json::from_slice(&bytes).unwrap();
        match back {
            IpcRequest::Recall { query, kind, limit } => {
                assert_eq!(query, "auth flow");
                assert_eq!(kind.as_deref(), Some("decision"));
                assert_eq!(limit, 10);
            }
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn missing_socket_yields_ipc_error() {
        let client = IpcClient::new(PathBuf::from(
            "/this/path/definitely/does/not/exist.sock",
        ))
        .with_timeout(Duration::from_millis(50));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.request(IpcRequest::Ping));
        assert!(matches!(result, Err(CliError::Ipc(_))));
    }
}
