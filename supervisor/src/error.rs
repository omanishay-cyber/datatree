//! Supervisor-wide error type.
//!
//! Every fallible function in this crate returns `Result<T, SupervisorError>`.
//! Errors are `thiserror`-derived so each variant is an explicit failure mode
//! callers can match on. `From` conversions exist for the most common
//! third-party error types we touch.

use std::io;
use thiserror::Error;

/// All errors produced by the supervisor.
#[derive(Debug, Error)]
pub enum SupervisorError {
    /// Failed to spawn or interact with a child process.
    #[error("child '{name}' spawn failed: {source}")]
    Spawn {
        /// Child name that could not be spawned.
        name: String,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// A child exited and exceeded its restart budget.
    #[error("child '{name}' exceeded restart budget ({restarts} in {window_secs}s)")]
    RestartBudgetExceeded {
        /// Child name.
        name: String,
        /// Number of restarts observed in the rolling window.
        restarts: u32,
        /// Length of the rolling window in seconds.
        window_secs: u64,
    },

    /// Watchdog detected a missed heartbeat past the deadline.
    #[error("child '{name}' missed heartbeat for {missed_ms}ms (limit {limit_ms}ms)")]
    HeartbeatMissed {
        /// Child name.
        name: String,
        /// Milliseconds since last heartbeat.
        missed_ms: u64,
        /// Limit before force-kill.
        limit_ms: u64,
    },

    /// Boot-time worker binary version mismatch (Bug I defensive fix).
    ///
    /// Surfaced by the boot-time `--version` probe in
    /// `manager::probe_worker_versions`. When a worker exe advertises a
    /// version that does not match `env!("CARGO_PKG_VERSION")` of the
    /// supervisor (e.g. a partial install left v0.3.0 binaries beside a
    /// v0.3.2 supervisor), refuse to spawn rather than risk an IPC
    /// schema mismatch that would crash-loop with an opaque
    /// `STATUS_CONTROL_C_EXIT` (-1073741510 on Windows). The message
    /// names the offending worker and instructs the operator to
    /// reinstall mneme.
    #[error(
        "child '{worker}' binary version skew: supervisor is {expected}, worker is {actual} \
         — reinstall mneme to fix"
    )]
    BinaryVersionSkew {
        /// Worker name (the `ChildSpec.name`, not the path).
        worker: String,
        /// `CARGO_PKG_VERSION` baked into the supervisor binary.
        expected: String,
        /// Version string the worker exe printed in response to `--version`.
        actual: String,
    },

    /// Generic I/O error (file system, sockets, pipes).
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// JSON (de)serialization error from the IPC layer.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML deserialization error from the config loader.
    #[error("toml deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),

    /// HTTP server error from the SLA dashboard.
    #[error("http server error: {0}")]
    Http(String),

    /// IPC error from the control plane.
    #[error("ipc error: {0}")]
    Ipc(String),

    /// Configuration validation error.
    #[error("invalid configuration: {0}")]
    Config(String),

    /// Catch-all for anyhow-bubbled errors at higher layers.
    #[error("internal error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for SupervisorError {
    fn from(value: anyhow::Error) -> Self {
        SupervisorError::Other(value.to_string())
    }
}
