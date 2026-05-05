//! Supervisor configuration.
//!
//! Configuration is loaded from a single TOML file (default
//! `~/.mneme/supervisor.toml`). The CLI `--config <path>` flag overrides the
//! default location. If the file is missing the supervisor falls back to
//! [`SupervisorConfig::default_layout`] so a fresh install still boots.

use crate::child::{ChildSpec, RestartStrategy};
use crate::error::SupervisorError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Top-level supervisor configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    /// Root directory for mneme state (`~/.mneme`).
    pub root_dir: PathBuf,
    /// Directory holding child binaries (`~/.mneme/bin`).
    pub bin_dir: PathBuf,
    /// Directory for crash dumps and logs.
    pub log_dir: PathBuf,
    /// IPC socket / named-pipe path.
    pub ipc_socket_path: PathBuf,
    /// Port for the SLA dashboard. Always bound to `127.0.0.1`.
    pub health_port: u16,
    /// Frequency of the watchdog `/health` self-test.
    #[serde(with = "duration_secs")]
    pub health_check_interval: Duration,
    /// Default restart policy for children that don't specify one inline.
    pub default_restart_policy: RestartPolicy,
    /// All children to spawn at boot.
    pub children: Vec<ChildSpec>,
    /// Auto-update notification settings (Wave 2.4).
    #[serde(default)]
    pub auto_update: AutoUpdateConfig,
}

/// Configuration for the background update-check task (Wave 2.4).
///
/// Written to / read from `~/.mneme/config.toml` under the
/// `[auto_update]` section. Every field has a sane default so a missing
/// section behaves identically to an explicit all-defaults section.
///
/// ```toml
/// [auto_update]
/// enabled = true            # default ON in v0.4.0 — opt-out only
/// check_interval_hours = 24
/// apply = false             # default OFF — apply mode is Wave 2.5
/// include_prerelease = false
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoUpdateConfig {
    /// Whether the daemon should poll GitHub for new releases at all.
    ///
    /// Default `true`. Set to `false` to disable all update-check network
    /// activity. The `MNEME_NO_UPDATE_CHECK=1` environment variable is an
    /// equivalent opt-out that does not require editing the config file.
    #[serde(default = "default_update_enabled")]
    pub enabled: bool,

    /// How many hours to sleep between checks.
    ///
    /// Default 24. Minimum enforced at runtime to 1 to avoid hammering the
    /// GitHub API from a test environment with a very short interval.
    #[serde(default = "default_check_interval_hours")]
    pub check_interval_hours: u64,

    /// Whether to automatically apply updates (download + replace binaries).
    ///
    /// Default `false` — notify-only in v0.4.0. Wave 2.5 will flip this to
    /// an opt-in `true` once the apply path is hardened and SHA-256-verified.
    #[serde(default)]
    pub apply: bool,

    /// Include pre-release tags when determining "latest".
    ///
    /// Default `false`. Set to `true` to track release candidates and betas.
    /// GitHub's `/releases/latest` endpoint never returns a pre-release as
    /// "latest" anyway, but this gate is respected when iterating
    /// `/releases` in future apply-mode work.
    #[serde(default)]
    pub include_prerelease: bool,
}

fn default_update_enabled() -> bool {
    true
}

fn default_check_interval_hours() -> u64 {
    24
}

impl Default for AutoUpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_hours: 24,
            apply: false,
            include_prerelease: false,
        }
    }
}

/// Backoff + budget configuration shared by every child unless overridden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    /// Initial backoff after a crash.
    #[serde(with = "duration_millis")]
    pub initial_backoff: Duration,
    /// Maximum backoff value (cap of the exponential schedule).
    #[serde(with = "duration_millis")]
    pub max_backoff: Duration,
    /// Multiplier applied to the previous backoff (clamped to `max_backoff`).
    pub backoff_multiplier: f32,
    /// Maximum restarts per `budget_window`.
    pub max_restarts_per_window: u32,
    /// Length of the rolling window for the restart budget.
    #[serde(with = "duration_secs")]
    pub budget_window: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 5.0,
            max_restarts_per_window: 5,
            budget_window: Duration::from_secs(60),
        }
    }
}

impl SupervisorConfig {
    /// Load config from disk. Returns the default layout if the file does not
    /// exist (a brand-new install).
    pub fn load(path: &Path) -> Result<Self, SupervisorError> {
        if !path.exists() {
            return Ok(Self::default_layout());
        }
        let text = std::fs::read_to_string(path)?;
        let cfg: SupervisorConfig = toml::from_str(&text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate cross-field invariants. Called automatically by [`load`].
    pub fn validate(&self) -> Result<(), SupervisorError> {
        if self.children.is_empty() {
            return Err(SupervisorError::Config(
                "no children configured".to_string(),
            ));
        }
        if self.health_port == 0 {
            return Err(SupervisorError::Config(
                "health_port must not be zero".to_string(),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for c in &self.children {
            if !seen.insert(c.name.clone()) {
                return Err(SupervisorError::Config(format!(
                    "duplicate child name '{}'",
                    c.name
                )));
            }
        }
        Ok(())
    }

    /// Default layout (used when no config file exists). Mirrors the process
    /// tree from §3.1 of the design doc.
    pub fn default_layout() -> Self {
        let home = home_dir();
        let root = home.join(".mneme");
        let bin = root.join("bin");

        let parser_pool_size = num_cpus::get().max(1);
        let scanner_pool_size = (num_cpus::get() / 2).max(1);

        let mut children = Vec::new();

        // Bug #233 (2026-05-04): every worker spec MUST keep
        // `heartbeat_deadline = None` until that worker class actually
        // wires a periodic `worker_ipc::heartbeat()` send. Workers in
        // this repo (parsers, scanners, brain, md-ingest, livebus,
        // store) currently emit ZERO heartbeats — `common::worker_ipc`
        // only exposes `report_complete`, no heartbeat helper exists.
        // The watchdog initialises `last_heartbeat` to spawn-time
        // (manager.rs:267) and only advances it when the
        // `ControlCommand::Heartbeat` IPC handler fires (manager.rs:867)
        // — and nothing in this repo sends that command. With a
        // `Some(60s)` deadline the watchdog's `heartbeat_pass` is
        // guaranteed to fire `force-kill` 60 s after every worker
        // spawns, dispatches in flight time out at the 10 s
        // STDIN_WRITE_TIMEOUT, and the monitor + restart loop drag
        // every worker through a perpetual kill→respawn cycle. Visible
        // as 40+ orphan parser-worker procs and 100+ supervisor
        // restarts in `mneme doctor` under sustained Claude Code tool
        // traffic.
        //
        // The S-PHASE NEW-055 contract on `watchdog::heartbeat_pass`
        // already documents `None` as the explicit "opt-out until
        // heartbeat-send is wired" path, and `pid_alive_pass`
        // (Bug F-9) provides genuine "process actually dead"
        // detection that does not depend on worker-side cooperation.
        // Re-enabling per-worker heartbeat enforcement is a v0.3.3
        // refactor that lands a `worker_ipc::heartbeat()` helper +
        // a tokio interval task in each worker's `main.rs` BEFORE the
        // matching spec flips its deadline back to `Some(...)`.
        //
        // The regression test
        // `tests::default_layout_workers_have_no_heartbeat_deadline_until_emit_is_wired`
        // pins this contract — flipping any of these to `Some(...)`
        // without a paired heartbeat-emit change will fail CI with a
        // pointer back here.
        children.push(ChildSpec {
            name: "store-worker".into(),
            command: bin.join("mneme-store").to_string_lossy().into(),
            args: vec![],
            env: vec![],
            restart: RestartStrategy::Permanent,
            rss_limit_mb: Some(512),
            cpu_limit_percent: Some(80),
            health_endpoint: Some("/health".into()),
            heartbeat_deadline: None,
        });

        for i in 0..parser_pool_size {
            children.push(ChildSpec {
                name: format!("parser-worker-{i}"),
                command: bin.join("mneme-parsers").to_string_lossy().into(),
                args: vec!["--worker-id".into(), i.to_string()],
                env: vec![],
                restart: RestartStrategy::Permanent,
                rss_limit_mb: Some(384),
                cpu_limit_percent: Some(75),
                health_endpoint: Some("/health".into()),
                heartbeat_deadline: None,
            });
        }

        for i in 0..scanner_pool_size {
            children.push(ChildSpec {
                name: format!("scanner-worker-{i}"),
                command: bin.join("mneme-scanners").to_string_lossy().into(),
                args: vec!["--worker-id".into(), i.to_string()],
                env: vec![],
                restart: RestartStrategy::Permanent,
                rss_limit_mb: Some(256),
                cpu_limit_percent: Some(60),
                health_endpoint: Some("/health".into()),
                heartbeat_deadline: None,
            });
        }

        children.push(ChildSpec {
            name: "md-ingest-worker".into(),
            command: bin.join("mneme-md-ingest").to_string_lossy().into(),
            args: vec![],
            env: vec![],
            restart: RestartStrategy::Permanent,
            rss_limit_mb: Some(192),
            cpu_limit_percent: Some(40),
            health_endpoint: Some("/health".into()),
            heartbeat_deadline: None,
        });

        // v0.2: multimodal extraction moved fully in-process. The
        // `mneme-multimodal` crate (pure Rust; no Python sidecar) is
        // driven directly by `mneme graphify`. No supervised child for
        // this path.

        children.push(ChildSpec {
            name: "brain-worker".into(),
            command: bin.join("mneme-brain").to_string_lossy().into(),
            args: vec![],
            env: vec![],
            restart: RestartStrategy::Permanent,
            rss_limit_mb: Some(2048),
            cpu_limit_percent: Some(90),
            health_endpoint: Some("/health".into()),
            heartbeat_deadline: None,
        });

        children.push(ChildSpec {
            name: "livebus-worker".into(),
            command: bin.join("mneme-livebus").to_string_lossy().into(),
            args: vec![],
            env: vec![],
            restart: RestartStrategy::Permanent,
            rss_limit_mb: Some(128),
            cpu_limit_percent: Some(40),
            health_endpoint: Some("/health".into()),
            heartbeat_deadline: None,
        });

        // v0.1: mcp-server and vision-server are SPAWNED ON DEMAND, not
        // supervised. The real MCP server is started by Claude Code itself
        // when it runs `mneme mcp stdio` — one instance per Claude-Code
        // window. The vision server launches via `mneme view` / the
        // Tauri app. Running them under the supervisor is redundant, and
        // they exit cleanly when stdin closes, which the supervisor reads
        // as "failed." Excluded from default children to keep every other
        // worker green.
        let _bun = resolve_bun();

        // health-watchdog is in-process (a tokio task in this crate) so it does
        // not appear here as an OS-level child.

        SupervisorConfig {
            root_dir: root.clone(),
            bin_dir: bin,
            log_dir: root.join("logs"),
            ipc_socket_path: default_ipc_path(&root),
            health_port: 7777,
            health_check_interval: Duration::from_secs(60),
            default_restart_policy: RestartPolicy::default(),
            children,
            auto_update: AutoUpdateConfig::default(),
        }
    }
}

#[cfg(windows)]
fn default_ipc_path(_root: &Path) -> PathBuf {
    // K10 test hook: per-test daemon socket. When `MNEME_TEST_SOCKET_NAME`
    // is set, use it verbatim as the named-pipe leaf — this lets the
    // chaos-test-suite spawn a daemon on a unique pipe so concurrent
    // test runs (and parallel `cargo test` jobs) don't collide on the
    // global PID-scoped name. Production users never set this var, so
    // the PID-scoped path below is taken — same behavior as before.
    if let Ok(custom) = std::env::var("MNEME_TEST_SOCKET_NAME") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(format!(r"\\.\pipe\{}", trimmed));
        }
    }
    // Named pipes linger briefly after the owning process dies, which
    // causes "Access denied" on rebind. We append the current PID so
    // a fresh supervisor always binds cleanly. CLI clients discover the
    // active pipe via `~/.mneme/supervisor.pipe-name` (written at boot).
    PathBuf::from(format!(r"\\.\pipe\mneme-supervisor-{}", std::process::id()))
}

/// Locate the Bun binary for child specs. Priority:
///   1. `MNEME_BUN` env var (absolute path)
///   2. `%LOCALAPPDATA%\Microsoft\WinGet\Links\bun.exe` (winget default)
///   3. `bun` / `bun.exe` on PATH
pub fn resolve_bun() -> String {
    if let Ok(p) = std::env::var("MNEME_BUN") {
        if Path::new(&p).exists() {
            return p;
        }
    }
    #[cfg(windows)]
    {
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            let candidate = Path::new(&la).join(r"Microsoft\WinGet\Links\bun.exe");
            if candidate.exists() {
                return candidate.to_string_lossy().into();
            }
        }
        "bun.exe".into()
    }
    #[cfg(not(windows))]
    {
        "bun".into()
    }
}

#[cfg(unix)]
fn default_ipc_path(root: &Path) -> PathBuf {
    // K10 test hook: per-test daemon socket. When `MNEME_TEST_SOCKET_NAME`
    // is set, the value is treated as the socket leaf name relative to
    // `<root>/`. This lets the chaos-test-suite spawn a daemon on a
    // unique socket so parallel `cargo test` jobs don't collide.
    if let Ok(custom) = std::env::var("MNEME_TEST_SOCKET_NAME") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return root.join(trimmed);
        }
    }
    root.join("supervisor.sock")
}

fn home_dir() -> PathBuf {
    if let Some(h) = std::env::var_os("MNEME_HOME") {
        return PathBuf::from(h);
    }
    #[cfg(windows)]
    {
        if let Some(h) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(h);
        }
    }
    #[cfg(unix)]
    {
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h);
        }
    }
    PathBuf::from(".")
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_secs().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        (d.as_millis() as u64).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}
