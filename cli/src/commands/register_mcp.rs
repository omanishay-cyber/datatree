//! `mneme register-mcp` / `mneme unregister-mcp` — minimal MCP wiring.
//!
//! These commands exist to give the one-line installer (and anyone who
//! just wants the MCP tools without a CLAUDE.md manifest block) a clean
//! first-class entry point. They are thin wrappers around
//! [`crate::commands::install`] / [`crate::commands::uninstall`] with
//! `--skip-manifest` and `--skip-hooks` preset.
//!
//! Why a separate command?
//!
//!   * The full install command is multi-step (manifest + MCP + hooks)
//!     and power-user-shaped. New users don't want to remember
//!     `--skip-manifest --skip-hooks`.
//!   * `scripts/install.ps1` calls `mneme register-mcp --platform
//!     claude-code` which reads cleaner than the skip-flag incantation
//!     and keeps the install pipeline documented at one site.
//!   * Makes the v0.3.1 promise explicit: "installer only writes the
//!     MCP entry, nothing else". The command name IS the promise.

use clap::Args;
use std::path::PathBuf;

use crate::commands::{install, uninstall};
use crate::error::CliResult;

/// Shared args — both register and unregister accept the same platform /
/// scope / dry-run surface.
#[derive(Debug, Args)]
pub struct RegisterMcpArgs {
    /// Platform to register with. Defaults to `claude-code`.
    #[arg(long, default_value = "claude-code")]
    pub platform: String,

    /// Print what would change but write nothing.
    #[arg(long)]
    pub dry_run: bool,

    /// Install scope (user | project | global). Defaults to `user`.
    #[arg(long, default_value = "user")]
    pub scope: String,

    /// Project root override. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// Entry point for `mneme register-mcp`.
///
/// Writes ONLY an `mcpServers.<name>` entry to the host's MCP config
/// file (Claude Code: `~/.claude.json`; Cursor: `~/.cursor/mcp.json`;
/// etc). Does NOT touch the host's settings.json, does NOT inject any
/// hook, does NOT write a CLAUDE.md / AGENTS.md manifest.
///
/// Internally this delegates to `mneme install` with `--skip-manifest`
/// and `--skip-hooks` set, so platform adapters stay DRY.
pub async fn register(args: RegisterMcpArgs) -> CliResult<()> {
    let install_args = install::InstallArgs {
        platform: Some(args.platform),
        dry_run: args.dry_run,
        scope: args.scope,
        project: args.project,
        force: false,
        skip_mcp: false,     // WE WANT the MCP write — this is the whole point.
        skip_hooks: true,    // Never register hooks via this path.
        skip_manifest: true, // Never write CLAUDE.md via this path.
    };
    install::run(install_args).await
}

/// Entry point for `mneme unregister-mcp`. Inverse of `register`.
///
/// Removes the mneme `mcpServers` entry without touching manifests /
/// hooks. Delegates to `mneme uninstall`, which is already tight
/// (manifest removal is marker-aware + MCP removal is key-scoped, so
/// other servers stay intact).
pub async fn unregister(args: RegisterMcpArgs) -> CliResult<()> {
    let uninstall_args = uninstall::UninstallArgs {
        platform: Some(args.platform),
        dry_run: args.dry_run,
        scope: args.scope,
        project: args.project,
    };
    uninstall::run(uninstall_args).await
}
