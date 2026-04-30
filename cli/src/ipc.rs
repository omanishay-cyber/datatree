//! Async client to the supervisor's control socket.
//!
//! The supervisor listens on a Unix socket (Unix) or named pipe (Windows)
//! at `~/.mneme/run/mneme-supervisor.sock`. Wire format is one
//! length-prefixed JSON message per request — we use 4-byte big-endian
//! length, then UTF-8 JSON body, matching the framing the supervisor's
//! `IpcServer` expects.
//!
//! The CLI is a *thin* IPC client. It connects, sends one request, awaits
//! one response, then drops the connection. No multiplexing, no streaming.
//! That keeps the client trivially correct and lets the supervisor pool
//! file descriptors however it likes.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{CliError, CliResult};
use common::query::{BlastItem, GodNode, RecallHit};

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
    /// Recall via embedding + FTS. v0.3.1+: supervisor-mediated when the
    /// daemon is up; CLI falls back to a direct `graph.db` read when the
    /// daemon is down.
    Recall {
        /// Project root whose `graph.db` shard to query.
        project: PathBuf,
        /// Free-form query string.
        query: String,
        /// Max number of hits to return.
        limit: usize,
        /// Optional filter: decision | conversation | concept | file | todo | constraint.
        #[serde(rename = "filter_type")]
        filter_type: Option<String>,
    },
    /// Blast-radius lookup. Supervisor path opens the same shard the CLI
    /// would and runs identical SQL — the fallback stays correct.
    Blast {
        /// Project root whose `graph.db` shard to query.
        project: PathBuf,
        /// File path or fully-qualified function name.
        target: String,
        /// Max traversal depth.
        depth: usize,
    },
    /// Top-N most-connected concepts. Supervisor path runs the same
    /// degree query the CLI's direct-DB fallback runs.
    GodNodes {
        /// Project root whose `graph.db` shard to query.
        project: PathBuf,
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
    /// (v0.3) Queue a supervisor-mediated job. Maps to supervisor's
    /// `DispatchJob` — the CLI uses this in `mneme build --dispatch`
    /// to hand parse/scan/embed work to the worker pool.
    DispatchJob {
        /// The job to queue (parse/scan/embed/ingest).
        job: common::jobs::Job,
    },
    /// (v0.3) Fetch a snapshot of the supervisor's job queue. Used by
    /// the CLI build watchdog to know when all dispatched work is done.
    JobQueueStatus,
    /// (v0.3.2, SD-1) Hook write: append a turn row through the
    /// supervisor's shared per-shard writer task. The CLI's
    /// `hook_writer.rs` tries this first; on Err it falls back to the
    /// direct-DB write so daemon-down scenarios still record state.
    WriteTurn {
        /// Project root the hook resolved its CWD to.
        project: PathBuf,
        /// Session id assigned by the host.
        session_id: String,
        /// Role label (`user` | `assistant` | `session_end` | …).
        role: String,
        /// Raw turn content.
        content: String,
    },
    /// (v0.3.2, SD-1) Hook write: append a ledger entry row through
    /// the supervisor's shared per-shard writer task.
    WriteLedgerEntry {
        /// Project root the hook resolved its CWD to.
        project: PathBuf,
        /// Session id assigned by the host.
        session_id: String,
        /// Ledger entry kind (`decision` | `note` | …).
        kind: String,
        /// Short summary text.
        summary: String,
        /// Optional rationale.
        #[serde(default)]
        rationale: Option<String>,
    },
    /// (v0.3.2, SD-1) Hook write: append a tool-call row through the
    /// supervisor's shared per-shard writer task.
    WriteToolCall {
        /// Project root the hook resolved its CWD to.
        project: PathBuf,
        /// Session id assigned by the host.
        session_id: String,
        /// Tool name.
        tool: String,
        /// Verbatim JSON-encoded params.
        params_json: String,
        /// Verbatim JSON-encoded result.
        result_json: String,
    },
    /// (v0.3.2, SD-1) Hook write: append a file-event row through the
    /// supervisor's shared per-shard writer task.
    WriteFileEvent {
        /// Project root the hook resolved its CWD to.
        project: PathBuf,
        /// File path the event references.
        file_path: String,
        /// Event kind (`pre_write` | `post_write` | …).
        event_type: String,
        /// Actor label (the tool name, typically).
        actor: String,
    },
}

/// One response from the supervisor. Wire format mirrors the supervisor's
/// own `ControlResponse` enum (tag = "response") so the CLI can speak the
/// native supervisor protocol without a translation layer.
///
/// ## SD-3: forward-compatibility
///
/// This enum mirrors `supervisor::ipc::ControlResponse` and is marked
/// `#[non_exhaustive]` so that adding a new variant on the supervisor
/// side never silently breaks downstream `match` arms in the CLI. Every
/// `match resp` over an `IpcResponse` MUST also include a `_ =>` default
/// arm (see `commands::build::handle_response`) so a newer supervisor
/// talking to an older CLI degrades gracefully instead of panicking on
/// "non-exhaustive match" at compile time or surfacing as a runtime
/// `serde_json::Error` decoded into `Error { message: "unknown variant" }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
#[non_exhaustive]
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
    /// Successful pool dispatch — carries the worker name the job was
    /// routed to. (Mirror of `ControlResponse::Dispatched`.)
    Dispatched {
        /// Worker that accepted the job.
        worker: String,
    },
    /// (v0.3) Job accepted by the supervisor.
    JobQueued {
        /// Supervisor-minted job id.
        job_id: common::jobs::JobId,
    },
    /// (v0.3) Snapshot of the supervisor job queue.
    JobQueue {
        /// Snapshot shape mirrors `supervisor::JobQueueSnapshot`.
        snapshot: serde_json::Value,
    },
    /// (v0.3.1) Supervisor-mediated recall results.
    RecallResults {
        /// Hits in supervisor-ranked order (FTS rank / LIKE length).
        hits: Vec<RecallHit>,
    },
    /// (v0.3.1) Supervisor-mediated blast-radius results.
    BlastResults {
        /// Dependents grouped by BFS depth layer.
        impacted: Vec<BlastItem>,
    },
    /// (v0.3.1) Supervisor-mediated god-node results.
    GodNodesResults {
        /// Top-N nodes by total degree.
        nodes: Vec<GodNode>,
    },
    /// (v0.3.1, NEW-019) Result of `GraphifyCorpus` — count of jobs queued.
    GraphifyCorpusQueued {
        /// Number of `Job::Ingest` items queued.
        queued: usize,
        /// Project root the supervisor enumerated.
        project: PathBuf,
    },
    /// (v0.3.1, NEW-019) Combined snapshot of workers + queue.
    SnapshotCombined {
        /// Per-child snapshots.
        children: Vec<serde_json::Value>,
        /// Job-queue stats.
        jobs: serde_json::Value,
        /// Echoed scope so the caller knows what it asked for.
        scope: String,
    },
    /// (v0.3.1, NEW-019) Result of `Rebuild`.
    RebuildAcked {
        /// Worker names killed.
        workers: Vec<String>,
        /// Whether `force` was honoured.
        force: bool,
    },
    /// Error payload.
    Error {
        /// Human-readable error.
        message: String,
    },
    /// Generic "this RPC isn't supported by this build" reply. Lets a
    /// newer supervisor signal capability gaps without reusing `Error`
    /// (which means a *runtime* failure).
    BadRequest {
        /// Diagnostic message.
        message: String,
    },
}

/// Async client. Construct with [`IpcClient::connect`] and call
/// [`IpcClient::request`] for each round trip.
#[derive(Debug)]
pub struct IpcClient {
    /// Fallback path used when there is no discovery file or the
    /// discovery file is unreadable. Set at construction.
    socket_path: PathBuf,
    /// Bug K (postmortem 2026-04-29 §12.2): the discovery file (e.g.
    /// `~/.mneme/supervisor.pipe`) the client should re-read on every
    /// connect attempt. When the supervisor respawns it rewrites this
    /// file with a new pipe name; clients that cached a single
    /// `socket_path` from boot would dial the dead pipe forever.
    /// Re-reading on every `request` is cheap (one `read_to_string`
    /// per call) and robust against arbitrarily many daemon respawns.
    ///
    /// `None` means no re-resolution — the client uses `socket_path`
    /// directly. Set by [`IpcClient::default_path`] and
    /// [`IpcClient::from_discovery_file`].
    discovery_path: Option<PathBuf>,
    timeout: Duration,
    /// B-001/B-002: when set, [`IpcClient::request`] will NOT call
    /// [`spawn_daemon_detached`] on connect-failure — it returns
    /// `Err(CliError::Ipc)` immediately so the caller can run its own
    /// fallback (e.g. `mneme build`'s direct-subprocess audit path).
    /// See [`IpcClient::with_no_autospawn`].
    no_autospawn: bool,
}

impl IpcClient {
    /// Build a client. `socket_path` should be an absolute path to a
    /// Unix socket on Unix or a named pipe (`\\.\pipe\<name>`) on Windows.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            discovery_path: None,
            timeout: DEFAULT_IPC_TIMEOUT,
            no_autospawn: false,
        }
    }

    /// Build a client using the standard runtime dir from
    /// [`crate::runtime_dir`].
    ///
    /// Discovery order:
    ///   1. `<PathManager::default_root()>/supervisor.pipe` — Windows
    ///      PID-scoped pipe name written by the running supervisor at
    ///      boot. Routing through `PathManager` honors `MNEME_HOME`
    ///      (HOME-bypass-ipc fix from m-home cluster).
    ///   2. Fallback to `<runtime_dir>/<DEFAULT_IPC_SOCKET_NAME>` (the
    ///      legacy path before PID-scoped pipes; preserved for
    ///      backwards compat with old daemons).
    ///
    /// Bug K (2026-04-29): the resolved client also re-reads the
    /// discovery file on every connect attempt via `discovery_path`.
    /// The supervisor rewrites `supervisor.pipe` whenever it respawns
    /// with a new PID; without the re-read, long-lived clients (the
    /// MCP server `_client` singleton, build pipelines, etc.) would
    /// dial a dead pipe forever.
    pub fn default_path() -> Self {
        let root = common::paths::PathManager::default_root().root().to_path_buf();
        let disco = root.join("supervisor.pipe");
        // Try to resolve the path NOW so the first attempt has a
        // realistic socket path, but ALSO retain the disco path
        // so we can re-read on connect failure.
        if let Ok(content) = std::fs::read_to_string(&disco) {
            let p = content.trim();
            if !p.is_empty() {
                let mut c = Self::new(std::path::PathBuf::from(p));
                c.discovery_path = Some(disco);
                return c;
            }
        }
        // Discovery file present but empty/unreadable — fall back
        // to runtime_dir but still set discovery_path so a later
        // supervisor boot that writes the file is picked up.
        let mut c =
            Self::new(crate::runtime_dir().join(crate::DEFAULT_IPC_SOCKET_NAME));
        c.discovery_path = Some(disco);
        c
    }

    /// Bug K test seam: build a client whose discovery file lives at
    /// `path` instead of the standard `~/.mneme/supervisor.pipe`.
    ///
    /// Production code should call [`Self::default_path`]; this is for
    /// tests that want to simulate daemon respawns by rewriting the
    /// file from the test harness without touching the dev's real
    /// `~/.mneme/`.
    ///
    /// `socket_path` is initialised by reading `path` once at
    /// construction (matching `default_path`'s eager resolution); on
    /// every subsequent `request` it is re-resolved from the file.
    pub fn from_discovery_file(path: PathBuf) -> Self {
        let initial = match std::fs::read_to_string(&path) {
            Ok(content) => {
                let trimmed = content.trim().to_string();
                if trimmed.is_empty() {
                    crate::runtime_dir().join(crate::DEFAULT_IPC_SOCKET_NAME)
                } else {
                    PathBuf::from(trimmed)
                }
            }
            Err(_) => crate::runtime_dir().join(crate::DEFAULT_IPC_SOCKET_NAME),
        };
        let mut c = Self::new(initial);
        c.discovery_path = Some(path);
        c
    }

    /// Bug K: re-resolve the supervisor pipe / socket path from
    /// [`Self::discovery_path`] at the moment of a connect attempt.
    /// Falls back to the cached `socket_path` if the file is missing,
    /// empty, or unreadable.
    fn current_socket_path(&self) -> PathBuf {
        if let Some(disco) = &self.discovery_path {
            if let Ok(content) = std::fs::read_to_string(disco) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return PathBuf::from(trimmed);
                }
            }
        }
        self.socket_path.clone()
    }

    /// Override the per-request timeout.
    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// B-001/B-002: disable the auto-spawn-and-retry fallback in
    /// [`Self::request`]. With this flag set, a connect-failure on the
    /// first attempt returns `Err(CliError::Ipc)` immediately — no
    /// `spawn_daemon_detached()`, no `wait_for_supervisor` poll, no
    /// retry round-trip. Use this in flows that have their OWN
    /// fallback path (e.g. `mneme build`'s direct-subprocess audit) or
    /// that explicitly opted out of starting a second daemon.
    ///
    /// Without this flag the historical behaviour applies: a connect
    /// failure that *looks* like a dead daemon (per a small string
    /// heuristic) triggers a detached `mneme daemon start`, a 3-second
    /// supervisor-readiness poll, then a retry. That path is fine for
    /// hooks (which need a live daemon to inject context) but it is
    /// the wrong default for `mneme build`: the build pipeline already
    /// has a direct subprocess fallback, and a stray second daemon
    /// process pollutes the user's process tree (B-002) and can trip
    /// the inline build into hanging on the retry round-trip (B-001).
    pub fn with_no_autospawn(mut self) -> Self {
        self.no_autospawn = true;
        self
    }

    /// Connect, send `request`, await a single response, disconnect.
    ///
    /// Returns `Err(CliError::Ipc)` if the supervisor isn't running, the
    /// socket can't be opened, or the timeout is exceeded.
    ///
    /// ## Bug K — pipe re-resolution on connect failure
    ///
    /// Every connect attempt re-reads [`Self::discovery_path`] (the
    /// `~/.mneme/supervisor.pipe` discovery file) so a daemon respawn
    /// that rewrote the file with a new PID-scoped pipe name is
    /// picked up *between* requests. Additionally, on the first
    /// connect-failure inside a single `request()` call, we re-read
    /// once more before the autospawn / retry branches — covering the
    /// race where the supervisor respawns mid-call.
    pub async fn request(&self, request: IpcRequest) -> CliResult<IpcResponse> {
        let body = serde_json::to_vec(&request)?;
        let payload = framed(&body);

        // First attempt — re-resolve the socket path now (Bug K).
        let socket_path = self.current_socket_path();
        let first =
            tokio::time::timeout(self.timeout, round_trip(&socket_path, payload.clone())).await;
        match first {
            Ok(Ok(resp)) => return Ok(resp),
            Ok(Err(e)) => {
                // Bug K: before bubbling the error up, re-read the
                // discovery file once more — the supervisor may have
                // respawned during the time we held the cached path.
                // If the freshly-resolved path differs, retry on it
                // before falling through to autospawn / no_autospawn.
                if self.discovery_path.is_some() {
                    let refreshed = self.current_socket_path();
                    if refreshed != socket_path {
                        tracing::debug!(
                            stale = %socket_path.display(),
                            fresh = %refreshed.display(),
                            "supervisor.pipe changed mid-call; retrying with fresh path (Bug K)"
                        );
                        let r2 =
                            tokio::time::timeout(self.timeout, round_trip(&refreshed, payload.clone()))
                                .await;
                        if let Ok(Ok(resp)) = r2 {
                            return Ok(resp);
                        }
                        // If even the refreshed path fails, fall through
                        // to the original error-handling branches below
                        // with the new error message.
                        if let Ok(Err(e2)) = r2 {
                            return self
                                .handle_connect_failure(e2, &refreshed, payload, &request)
                                .await;
                        }
                        // Timeout on the refreshed retry — surface as a
                        // descriptive timeout error.
                        return Err(CliError::Ipc(format!(
                            "timeout after {:?} talking to supervisor at {} (after pipe re-resolve)",
                            self.timeout,
                            refreshed.display()
                        )));
                    }
                }
                self.handle_connect_failure(e, &socket_path, payload, &request).await
            }
            Err(_) => Err(CliError::Ipc(format!(
                "timeout after {:?} talking to supervisor at {}",
                self.timeout,
                socket_path.display()
            ))),
        }
    }

    /// Shared post-first-attempt error path. Either bails (no_autospawn,
    /// non-connect error, Stop/Ping) or runs the autospawn-then-retry
    /// branch using the freshest socket path.
    async fn handle_connect_failure(
        &self,
        e: CliError,
        socket_path: &Path,
        payload: Vec<u8>,
        request: &IpcRequest,
    ) -> CliResult<IpcResponse> {
        // B-001/B-002: when the caller opted out of auto-spawn,
        // connect failures bubble up immediately. The caller owns the
        // fallback (e.g. build.rs's direct-subprocess audit) and a
        // stray second daemon would pollute the process tree and risk
        // hanging the retry round-trip.
        if self.no_autospawn {
            return Err(e);
        }
        // Connection errors (pipe missing / socket absent) are the
        // signal that the daemon is dead. Auto-spawn once and retry.
        // Any other error (framing, timeout inside round_trip,
        // response parse) is NOT an excuse to spawn — those would
        // re-occur on a live daemon.
        let msg = format!("{e}");
        let looks_like_dead_daemon = msg.contains("could not connect")
            || msg.contains("No such file")
            || msg.contains("cannot find");
        if !looks_like_dead_daemon {
            return Err(e);
        }
        // Don't auto-spawn when the caller IS the spawn — avoid
        // infinite re-launch loops on daemon-internal commands.
        if matches!(request, IpcRequest::Stop | IpcRequest::Ping) {
            return Err(e);
        }
        tracing::warn!(
            socket = %socket_path.display(),
            "supervisor unreachable — auto-starting daemon"
        );
        if let Err(se) = spawn_daemon_detached() {
            tracing::warn!(error = %se, "could not spawn daemon; giving up");
            return Err(e);
        }
        // Give the daemon ~3s to come up + write its pipe. Bug K:
        // wait_for_supervisor polls the freshly-resolved path on each
        // tick so a brand-new daemon writing a brand-new pipe name to
        // the discovery file is picked up immediately.
        wait_for_supervisor_with_resolver(self, Duration::from_secs(3)).await;

        // Retry once, with the same timeout budget. Bug K: re-resolve
        // the path one more time so the retry uses whatever the new
        // supervisor wrote into the discovery file.
        let retry_path = self.current_socket_path();
        let retried = tokio::time::timeout(self.timeout, round_trip(&retry_path, payload))
            .await
            .map_err(|_| {
                CliError::Ipc(format!(
                    "timeout after {:?} talking to supervisor at {} (after auto-start)",
                    self.timeout,
                    retry_path.display()
                ))
            })??;
        Ok(retried)
    }

    /// Connect, ping, disconnect. Returns `true` iff the supervisor is up.
    pub async fn is_running(&self) -> bool {
        matches!(self.request(IpcRequest::Ping).await, Ok(IpcResponse::Pong))
    }
}

/// One round-trip: connect, write framed request, read framed response,
/// disconnect. Pulled out of `IpcClient::round_trip` (which used to be a
/// `&self` method reading `self.socket_path`) so the Bug K re-resolve
/// branch in [`IpcClient::request`] can target a different `socket_path`
/// without copying the body.
async fn round_trip(socket_path: &Path, framed_request: Vec<u8>) -> CliResult<IpcResponse> {
    let mut stream = connect_stream(socket_path).await?;

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
            .unwrap_or("mneme-supervisor")
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

/// M5 (D-window): Windows process-creation flags for the auto-spawn
/// fallback's `mneme daemon start` child.
///
/// Composition (kernel32 ABI, stable):
/// - DETACHED_PROCESS           (0x0000_0008) — no console at all.
/// - CREATE_NEW_PROCESS_GROUP   (0x0000_0200) — Ctrl+C in caller does not
///   propagate.
/// - CREATE_BREAKAWAY_FROM_JOB  (0x0100_0000) — survive shell-job teardown
///   (mirrors `commands/daemon.rs::spawn_detached`).
/// - CREATE_NO_WINDOW           (0x0800_0000) — suppress the transient
///   console flash that the OS otherwise creates while DETACHED_PROCESS
///   is still being applied. Without this flag, when the auto-spawn path
///   runs from a hidden parent (Claude Code hook spawned windowless),
///   a `cmd.exe`-style window can flicker before the child detaches.
///
/// Total: `0x0900_0208`.
///
/// Extracted as a `pub(crate)` fn so the unit test can assert the bit
/// composition without a real spawn.
#[cfg(windows)]
pub(crate) fn windows_daemon_detached_flags() -> u32 {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB | CREATE_NO_WINDOW
}

/// Spawn `mneme daemon start` fully detached so the supervisor keeps
/// running after the current CLI process exits. Used by the auto-start
/// fallback in [`IpcClient::request`].
fn spawn_daemon_detached() -> std::io::Result<()> {
    use std::process::{Command, Stdio};
    // We shell out to `mneme daemon start` (the same wrapper `main.rs`
    // dispatches) rather than directly to `mneme-daemon.exe`, so the
    // binary-discovery + `start` subcommand logic in commands/daemon.rs
    // stays the single source of truth.
    let mut cmd = Command::new("mneme");
    cmd.args(["daemon", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_daemon_detached_flags());
    }

    cmd.spawn()?;
    Ok(())
}

/// Poll for the supervisor's pipe/socket until it accepts a connection
/// or `deadline` elapses. Non-blocking on failure — returns either way.
#[allow(dead_code)] // kept for downstream callers; current crate uses
                    // `wait_for_supervisor_with_resolver` instead.
async fn wait_for_supervisor(socket_path: &std::path::Path, deadline: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < deadline {
        tokio::time::sleep(Duration::from_millis(150)).await;
        if connect_stream(socket_path).await.is_ok() {
            return;
        }
    }
}

/// Bug K variant of [`wait_for_supervisor`]: re-resolves the socket path
/// from the client's `discovery_path` on every poll tick. When the
/// supervisor we just spawned writes a new pipe name to
/// `~/.mneme/supervisor.pipe`, the next tick picks it up — so a
/// post-respawn name change doesn't strand us polling the old one.
async fn wait_for_supervisor_with_resolver(client: &IpcClient, deadline: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < deadline {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let socket_path = client.current_socket_path();
        if connect_stream(&socket_path).await.is_ok() {
            return;
        }
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
            project: PathBuf::from("/tmp/proj"),
            query: "auth flow".into(),
            limit: 10,
            filter_type: Some("decision".into()),
        };
        let bytes = serde_json::to_vec(&req).unwrap();
        let back: IpcRequest = serde_json::from_slice(&bytes).unwrap();
        match back {
            IpcRequest::Recall { project, query, limit, filter_type } => {
                assert_eq!(project, PathBuf::from("/tmp/proj"));
                assert_eq!(query, "auth flow");
                assert_eq!(filter_type.as_deref(), Some("decision"));
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

    /// B-001/B-002: `with_no_autospawn()` MUST cause `request` to return
    /// `Err` immediately on connect failure, WITHOUT calling
    /// `spawn_daemon_detached()` and WITHOUT waiting `wait_for_supervisor`'s
    /// 3s budget. We verify by:
    ///   1. Pointing at a path that cannot connect.
    ///   2. Issuing a request that is NOT `Stop`/`Ping` (those are
    ///      already excluded from auto-spawn — see line 449 in `request`).
    ///   3. Asserting the call returns Err in well under 3 seconds. A
    ///      live auto-spawn path would `wait_for_supervisor(... 3s)` and
    ///      then retry, so a sub-second return proves the autospawn
    ///      branch was bypassed entirely.
    #[test]
    fn ipc_client_with_no_autospawn_returns_err_on_connect_failure_without_spawning() {
        // Use a socket path that is guaranteed missing on this host. The
        // exact failure variant differs by platform (Unix: ENOENT, Windows:
        // pipe not-found), but BOTH go through the connect-failure branch
        // that would normally trigger `spawn_daemon_detached()`.
        let bogus = if cfg!(windows) {
            // A pipe name that doesn't exist; `connect_stream` will return
            // a "could not connect" error.
            PathBuf::from("\\\\.\\pipe\\mneme-no-autospawn-test-does-not-exist")
        } else {
            PathBuf::from("/tmp/mneme-no-autospawn-test-does-not-exist.sock")
        };

        let client = IpcClient::new(bogus)
            .with_timeout(Duration::from_secs(10)) // generous, we're testing speed not timeout
            .with_no_autospawn();

        // Use a non-Stop/non-Ping request so the existing exclusion
        // (line 449) does NOT mask the no-autospawn fix.
        let req = IpcRequest::Audit {
            scope: "full".into(),
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let started = std::time::Instant::now();
        let result = rt.block_on(client.request(req));
        let elapsed = started.elapsed();

        // Must be Err (connect failed; no daemon to spawn).
        assert!(
            matches!(result, Err(CliError::Ipc(_))),
            "expected CliError::Ipc on missing socket with no_autospawn, got: {result:?}"
        );

        // The hot path: spawn_daemon_detached() + wait_for_supervisor(3s)
        // would have us above 3s. With no_autospawn the call must return
        // immediately on connect failure — well under 1s on any sane
        // host. Use 2s as a generous CI cushion.
        assert!(
            elapsed < Duration::from_secs(2),
            "with_no_autospawn must skip the 3s wait_for_supervisor branch; \
             elapsed={elapsed:?} (expected < 2s)"
        );
    }

    /// B-002 cousin: `with_no_autospawn` is a builder; calling
    /// `.with_no_autospawn()` should chain cleanly with other builders
    /// and not regress the existing `with_timeout` behavior.
    #[test]
    fn ipc_client_with_no_autospawn_chains_with_with_timeout() {
        let client = IpcClient::new(PathBuf::from("/nope.sock"))
            .with_timeout(Duration::from_millis(50))
            .with_no_autospawn();
        // We just need to assert the builder returns a usable client
        // (i.e. the chain compiles + runs). The behavioural assertion
        // lives in the previous test.
        let _ = format!("{client:?}");
    }

    /// Bug K (postmortem 2026-04-29 §12.2): `IpcClient::default_path()`
    /// reads `~/.mneme/supervisor.pipe` exactly once and caches the
    /// resolved name in `self.socket_path`. When the supervisor
    /// respawns with a fresh PID it rewrites the discovery file with
    /// a new pipe name (e.g. `mneme-supervisor-19148` ⇒
    /// `mneme-supervisor-22501`). Long-lived clients (the MCP server
    /// `_client` singleton, daemon-process workers, build pipelines)
    /// keep the cached old name and fail with `cannot find file
    /// (os error 2)` forever — even though the discovery file *does*
    /// have the right name written by the new daemon.
    ///
    /// The fix: on connect failure, re-read the discovery file and
    /// retry once with the fresh name. Only if THAT also fails do we
    /// fall through to the existing autospawn / give-up branches.
    ///
    /// This test simulates the daemon respawn:
    ///
    /// 1. Write `tempdir/supervisor.pipe` containing pipe name X
    ///    (a bogus name guaranteed to fail connect).
    /// 2. Build the client with `from_discovery_file(tempdir/...)`
    ///    so we don't depend on `~/.mneme` (which the dev's real
    ///    daemon may be live in). Combine with `with_no_autospawn`
    ///    to skip the spawn_daemon_detached branch — we're testing
    ///    re-resolution, not autospawn.
    /// 3. First `request()` attempt fails with name X.
    /// 4. Update the discovery file to contain pipe name Y (also
    ///    bogus, also guaranteed to fail).
    /// 5. Issue a second `request()`. The client should re-read the
    ///    discovery file and produce an error mentioning Y, NOT X.
    ///    On the buggy code the error still mentions X (the cached
    ///    socket_path).
    #[test]
    fn ipc_re_resolves_pipe_name_on_connect_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let disco = tmp.path().join("supervisor.pipe");

        // Phase 1 — write the OLD pipe name.
        let name_x = if cfg!(windows) {
            "\\\\.\\pipe\\mneme-bug-k-test-OLD-pipe-name"
        } else {
            // Use an absolute path under tempdir so the resolve
            // fails with ENOENT; same failure shape as on Unix
            // production.
            "/tmp/mneme-bug-k-test-OLD-pipe-name.sock"
        };
        std::fs::write(&disco, name_x).expect("write old pipe name");

        let client = IpcClient::from_discovery_file(disco.clone())
            .with_no_autospawn()
            .with_timeout(Duration::from_secs(2));

        let req = IpcRequest::Audit {
            scope: "k-test-1".into(),
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let r1 = rt.block_on(client.request(req));
        assert!(
            matches!(r1, Err(CliError::Ipc(_))),
            "first request must fail (X is bogus); got: {r1:?}"
        );
        // The error should mention X, since X is what we tried.
        if let Err(CliError::Ipc(msg)) = &r1 {
            assert!(
                msg.contains("OLD"),
                "first attempt error should reference the OLD pipe name; got: {msg}"
            );
        }

        // Phase 2 — daemon "respawned" with a new pipe name. Update
        // the discovery file.
        let name_y = if cfg!(windows) {
            "\\\\.\\pipe\\mneme-bug-k-test-NEW-pipe-name"
        } else {
            "/tmp/mneme-bug-k-test-NEW-pipe-name.sock"
        };
        std::fs::write(&disco, name_y).expect("write new pipe name");

        let req = IpcRequest::Audit {
            scope: "k-test-2".into(),
        };
        let r2 = rt.block_on(client.request(req));
        assert!(
            matches!(r2, Err(CliError::Ipc(_))),
            "second request must also fail (Y is also bogus); got: {r2:?}"
        );
        // Bug K: the error should now mention Y, NOT X. Pre-fix the
        // client cached the old socket_path and would still report X.
        if let Err(CliError::Ipc(msg)) = &r2 {
            assert!(
                msg.contains("NEW"),
                "second attempt must re-resolve to NEW pipe name (Bug K); got: {msg}"
            );
            assert!(
                !msg.contains("OLD"),
                "second attempt must NOT use the stale OLD pipe name (Bug K); got: {msg}"
            );
        }
    }

    /// M5 (D-window): `spawn_daemon_detached` MUST set `CREATE_NO_WINDOW`
    /// (`0x0800_0000`) along with the existing detach flags so that when
    /// the auto-spawn fallback runs from a hidden parent (a Claude Code
    /// hook spawned windowless), the briefly-living `mneme daemon start`
    /// child does not flash a console window.
    ///
    /// The full required composition is:
    ///   DETACHED_PROCESS           = 0x0000_0008
    /// | CREATE_NEW_PROCESS_GROUP   = 0x0000_0200
    /// | CREATE_BREAKAWAY_FROM_JOB  = 0x0100_0000
    /// | CREATE_NO_WINDOW           = 0x0800_0000
    ///   ─────────────────────────────────────────
    ///   total                      = 0x0900_0208
    #[cfg(windows)]
    #[test]
    fn windows_daemon_detached_flags() {
        let flags = super::windows_daemon_detached_flags();
        // CREATE_NO_WINDOW bit MUST be set — this is the M5 fix.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        assert!(
            flags & CREATE_NO_WINDOW == CREATE_NO_WINDOW,
            "M5: spawn_daemon_detached must include CREATE_NO_WINDOW \
             (0x0800_0000); got 0x{flags:08x}"
        );
        // And the original detach flags must still be there (regression
        // guard against a fix that overwrites instead of OR-ing).
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        assert!(flags & DETACHED_PROCESS == DETACHED_PROCESS);
        assert!(flags & CREATE_NEW_PROCESS_GROUP == CREATE_NEW_PROCESS_GROUP);
        assert!(flags & CREATE_BREAKAWAY_FROM_JOB == CREATE_BREAKAWAY_FROM_JOB);
    }
}
