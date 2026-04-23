//! `datatree` — the user-facing command-line tool.
//!
//! The binary is intentionally thin. Every subcommand maps 1:1 to a
//! handler in `crate::commands::<name>::run`. Errors bubble up as
//! [`CliError`], which has a stable [`CliError::exit_code`] that hooks
//! and shells can branch on.
//!
//! ```text
//! datatree install [--platform=<name>] [--dry-run] [--scope=...]
//! datatree uninstall [--platform=<name>] [--scope=...]
//! datatree build [project_path]
//! datatree update [project_path]
//! datatree status
//! datatree view [--web]
//! datatree audit [--scope=theme|security|all]
//! datatree recall <query> [--type=...] [--limit=N]
//! datatree blast <target> [--depth=N]
//! datatree graphify
//! datatree godnodes [--n=N]
//! datatree drift [--severity=...]
//! datatree history <query> [--since=...]
//! datatree snap
//! datatree doctor
//! datatree rebuild
//! datatree step <op> [arg]
//! datatree inject  --prompt=... --session-id=... --cwd=...
//! datatree session-prime --project=... --session-id=...
//! datatree pre-tool   --tool=... --params=... --session-id=...
//! datatree post-tool  --tool=... --result-file=... --session-id=...
//! datatree turn-end   --session-id=... [--pre-compact|--subagent]
//! datatree session-end --session-id=...
//! datatree daemon <op>
//! ```

use clap::{Parser, Subcommand};
use datatree_cli::commands;
use datatree_cli::error::{CliError, CliResult};
use std::path::PathBuf;
use std::process::ExitCode;
use tracing::error;

/// Top-level CLI args.
#[derive(Debug, Parser)]
#[command(
    name = "datatree",
    version,
    about = "Datatree — the AI superbrain. Persistent per-project memory + 14-view graph + drift detector + 30+ MCP tools.",
    long_about = None,
    propagate_version = true,
)]
struct Cli {
    /// Lower the log threshold (-v=info, -vv=debug, -vvv=trace).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Force JSON-formatted log output (otherwise pretty).
    #[arg(long, global = true, env = "DATATREE_LOG_JSON")]
    log_json: bool,

    /// Override the supervisor IPC socket path. Useful for tests.
    #[arg(long, global = true, env = "DATATREE_SOCKET")]
    socket: Option<PathBuf>,

    /// The subcommand to run.
    #[command(subcommand)]
    cmd: Command,
}

/// Every datatree subcommand.
#[derive(Debug, Subcommand)]
enum Command {
    /// Install datatree into one or more AI platforms.
    Install(commands::install::InstallArgs),
    /// Reverse of `install`.
    Uninstall(commands::uninstall::UninstallArgs),
    /// Initial full project ingest.
    Build(commands::build::BuildArgs),
    /// Incremental update.
    Update(commands::update::UpdateArgs),
    /// Print graph stats / drift findings count / last build time.
    Status(commands::status::StatusArgs),
    /// Open the vision app (Tauri or browser).
    View(commands::view::ViewArgs),
    /// Run all configured scanners.
    Audit(commands::audit::AuditArgs),
    /// Semantic recall against history / decisions / concepts / files.
    Recall(commands::recall::RecallArgs),
    /// Blast radius for a file or function.
    Blast(commands::blast::BlastArgs),
    /// Multimodal extraction pass (PDF, image, audio, video, .ipynb...).
    Graphify(commands::graphify::GraphifyArgs),
    /// Top-N most-connected concepts.
    Godnodes(commands::godnodes::GodNodesArgs),
    /// Show current drift findings.
    Drift(commands::drift::DriftArgs),
    /// Search the conversation history.
    History(commands::history::HistoryArgs),
    /// Take a manual snapshot of the active shard.
    Snap(commands::snap::SnapArgs),
    /// Self-test the running daemon and shards.
    Doctor(commands::doctor::DoctorArgs),
    /// Drop everything and re-parse from scratch.
    Rebuild(commands::rebuild::RebuildArgs),
    /// Step Ledger operations.
    Step(commands::step::StepArgs),
    /// Hook entry: UserPromptSubmit (emits JSON additional_context).
    Inject(commands::inject::InjectArgs),
    /// Hook entry: SessionStart.
    #[command(name = "session-prime")]
    SessionPrime(commands::session_prime::SessionPrimeArgs),
    /// Hook entry: PreToolUse.
    #[command(name = "pre-tool")]
    PreTool(commands::pre_tool::PreToolArgs),
    /// Hook entry: PostToolUse.
    #[command(name = "post-tool")]
    PostTool(commands::post_tool::PostToolArgs),
    /// Hook entry: Stop (between turns).
    #[command(name = "turn-end")]
    TurnEnd(commands::turn_end::TurnEndArgs),
    /// Hook entry: SessionEnd.
    #[command(name = "session-end")]
    SessionEnd(commands::session_end::SessionEndArgs),
    /// Daemon control: start | stop | restart | status | logs.
    Daemon(commands::daemon::DaemonArgs),
    /// Launch the Bun MCP server (used by Claude Code / Codex / etc.
    /// to talk to datatree via stdio). `datatree mcp stdio`.
    Mcp {
        /// Transport mode. Currently only `stdio` is supported.
        #[arg(default_value = "stdio")]
        transport: String,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    init_tracing(cli.verbose, cli.log_json);

    let result = dispatch(cli).await;

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Hook outputs are JSON; print the error there too so the host
            // can display something. For interactive commands we use the
            // tracing logger.
            error!(error = %err, exit_code = err.exit_code(), "datatree command failed");
            eprintln!("error: {err}");
            ExitCode::from(err.exit_code() as u8)
        }
    }
}

async fn dispatch(cli: Cli) -> CliResult<()> {
    let socket_override = cli.socket.clone();
    match cli.cmd {
        Command::Install(args) => commands::install::run(args).await,
        Command::Uninstall(args) => commands::uninstall::run(args).await,
        Command::Build(args) => commands::build::run(args, socket_override).await,
        Command::Update(args) => commands::update::run(args, socket_override).await,
        Command::Status(args) => commands::status::run(args, socket_override).await,
        Command::View(args) => commands::view::run(args).await,
        Command::Audit(args) => commands::audit::run(args, socket_override).await,
        Command::Recall(args) => commands::recall::run(args, socket_override).await,
        Command::Blast(args) => commands::blast::run(args, socket_override).await,
        Command::Graphify(args) => commands::graphify::run(args, socket_override).await,
        Command::Godnodes(args) => commands::godnodes::run(args, socket_override).await,
        Command::Drift(args) => commands::drift::run(args, socket_override).await,
        Command::History(args) => commands::history::run(args, socket_override).await,
        Command::Snap(args) => commands::snap::run(args, socket_override).await,
        Command::Doctor(args) => commands::doctor::run(args, socket_override).await,
        Command::Rebuild(args) => commands::rebuild::run(args, socket_override).await,
        Command::Step(args) => commands::step::run(args, socket_override).await,
        Command::Inject(args) => commands::inject::run(args, socket_override).await,
        Command::SessionPrime(args) => commands::session_prime::run(args, socket_override).await,
        Command::PreTool(args) => commands::pre_tool::run(args, socket_override).await,
        Command::PostTool(args) => commands::post_tool::run(args, socket_override).await,
        Command::TurnEnd(args) => commands::turn_end::run(args, socket_override).await,
        Command::SessionEnd(args) => commands::session_end::run(args, socket_override).await,
        Command::Daemon(args) => commands::daemon::run(args, socket_override).await,
        Command::Mcp { transport } => launch_mcp(transport).await,
    }
}

/// Exec into the Bun MCP server. Searches for `mcp/index.ts` at:
///   1. $DATATREE_MCP_PATH (env var)
///   2. ~/.datatree/mcp/index.ts (production install)
///   3. ./mcp/index.ts (development, relative to cwd)
async fn launch_mcp(transport: String) -> CliResult<()> {
    if transport != "stdio" {
        return Err(CliError::Other(format!(
            "only stdio transport supported, got {transport:?}"
        )));
    }
    let candidates: Vec<PathBuf> = [
        std::env::var("DATATREE_MCP_PATH").ok().map(PathBuf::from),
        dirs::home_dir().map(|h| h.join(".datatree").join("mcp").join("src").join("index.ts")),
        dirs::home_dir().map(|h| h.join(".datatree").join("mcp").join("index.ts")),
        Some(PathBuf::from("mcp/src/index.ts")),
        Some(PathBuf::from("mcp/index.ts")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mcp_path = candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .ok_or_else(|| {
            CliError::Other(
                "mcp/index.ts not found — set DATATREE_MCP_PATH or install the MCP server".into(),
            )
        })?;
    let bun = which_bun();
    let status = std::process::Command::new(&bun)
        .arg(mcp_path)
        .status()
        .map_err(|e| CliError::Other(format!("failed to spawn bun: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::Other(format!(
            "bun exited with {}",
            status.code().unwrap_or(-1)
        )))
    }
}

fn which_bun() -> String {
    // Prefer explicit env, then common Windows locations, then "bun" on PATH.
    if let Ok(p) = std::env::var("DATATREE_BUN") {
        return p;
    }
    #[cfg(windows)]
    {
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            let candidate =
                std::path::Path::new(&la).join(r"Microsoft\WinGet\Links\bun.exe");
            if candidate.exists() {
                return candidate.to_string_lossy().into();
            }
        }
    }
    "bun".into()
}

fn init_tracing(verbose: u8, json: bool) {
    use tracing_subscriber::filter::EnvFilter;
    use tracing_subscriber::fmt;

    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let env_filter =
        EnvFilter::try_from_env("DATATREE_LOG").unwrap_or_else(|_| EnvFilter::new(level));

    if json {
        let _ = fmt()
            .with_env_filter(env_filter)
            .json()
            .with_writer(std::io::stderr)
            .try_init();
    } else {
        let _ = fmt()
            .with_env_filter(env_filter)
            .with_writer(std::io::stderr)
            .try_init();
    }
}

// Make the binary still buildable as `cargo build -p datatree-cli` even
// when the workspace's `common` crate isn't yet present.
#[allow(unused_imports)]
use datatree_cli as _;
