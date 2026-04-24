//! `mneme install` — provision mneme into one or more AI platforms.
//!
//! Behaviour
//! =========
//!
//! - `mneme install` (no args)            → auto-detect every installed
//!                                              platform and configure each
//! - `mneme install --platform=cursor`    → configure exactly one
//! - `mneme install --dry-run`            → print what would change, do
//!                                              not write anything
//! - `mneme install --scope=project`      → write into the active project
//!                                              (default: user)
//! - `mneme install --force`              → overwrite even if the user
//!                                              edited mneme's marker block
//!
//! Per design §21.4.1 / §25.5: every write is marker-wrapped (idempotent),
//! every config write makes a `.bak` first, and the operation is safe to
//! re-run. See [`crate::markers`].

use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

use crate::error::CliResult;
use crate::platforms::{
    AdapterContext, InstallScope, Platform, PlatformDetector,
};

/// CLI args for `mneme install`.
#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Restrict installation to a single platform (id from the matrix in
    /// design §21.4 — e.g. `claude-code`, `cursor`, `codex`). When omitted,
    /// every detected platform is configured.
    #[arg(long)]
    pub platform: Option<String>,

    /// Print what would change but write nothing.
    #[arg(long)]
    pub dry_run: bool,

    /// Where to install — defaults to `user`.
    #[arg(long, default_value = "user")]
    pub scope: String,

    /// Project root override. Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// Overwrite mneme's marker block even if the user has hand-edited it.
    #[arg(long)]
    pub force: bool,

    /// Skip the MCP-config writes (manifest + hooks only). Useful when the
    /// user is registering MCP through their platform UI.
    #[arg(long)]
    pub skip_mcp: bool,

    /// Skip the hook writes (manifest + MCP only). In v0.3.1+ no platform
    /// actually registers hooks in the host's settings.json anymore (see
    /// `platforms/claude_code.rs` module docstring for why). This flag is
    /// kept for forward-compat with future platforms that will wire hooks
    /// against their own hook files, not Claude Code's settings.json.
    #[arg(long)]
    pub skip_hooks: bool,

    /// Skip the CLAUDE.md / AGENTS.md manifest write (MCP + hooks only).
    /// The one-line installer (`scripts/install.ps1`) sets this so a
    /// clean install touches only the platform's MCP registry and
    /// nothing else. Power users who want the manifest block can run
    /// `mneme install --platform=claude-code` without this flag later.
    #[arg(long)]
    pub skip_manifest: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: InstallArgs) -> CliResult<()> {
    let scope: InstallScope = args.scope.parse()?;
    let project_root = args
        .project
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let ctx = AdapterContext::new(scope, project_root.clone())
        .with_dry_run(args.dry_run)
        .with_force(args.force);

    let targets: Vec<Platform> = match args.platform {
        Some(id) => vec![Platform::from_id(&id)?],
        None => {
            let detected =
                PlatformDetector::detect_installed(scope, &project_root);
            info!(count = detected.len(), "auto-detected platforms");
            detected
        }
    };

    if targets.is_empty() {
        warn!("no platforms detected; nothing to install");
        return Ok(());
    }

    let bar = make_bar(targets.len() as u64);
    let mut report: Vec<InstallReport> = Vec::with_capacity(targets.len());

    for platform in targets {
        bar.set_message(platform.display_name().to_string());
        let r = install_one(
            platform,
            &ctx,
            args.skip_mcp,
            args.skip_hooks,
            args.skip_manifest,
        );
        report.push(InstallReport {
            platform,
            outcome: match &r {
                Ok(_) => "ok".into(),
                Err(e) => format!("error: {e}"),
            },
        });
        if let Err(e) = r {
            warn!(platform = platform.id(), error = %e, "install failed for platform");
        }
        bar.inc(1);
    }

    bar.finish_with_message("done");

    println!();
    println!(
        "{:<14}  {:<8}  {}",
        "platform", "scope", "result"
    );
    for entry in &report {
        println!(
            "{:<14}  {:<8}  {}",
            entry.platform.id(),
            scope_label(scope),
            entry.outcome
        );
    }
    if args.dry_run {
        println!("\n(dry-run: no files were written)");
    }

    Ok(())
}

/// Run one platform's adapter. Order matters: manifest first (so the user
/// sees mneme even if MCP/hook write fails), then MCP, then hooks.
fn install_one(
    platform: Platform,
    ctx: &AdapterContext,
    skip_mcp: bool,
    skip_hooks: bool,
    skip_manifest: bool,
) -> CliResult<()> {
    let adapter = platform.adapter();
    if !skip_manifest {
        let manifest = adapter.write_manifest(ctx)?;
        info!(platform = platform.id(), path = %manifest.display(), "manifest written");
    } else {
        info!(platform = platform.id(), "manifest skipped (--skip-manifest)");
    }

    if !skip_mcp {
        let mcp = adapter.write_mcp_config(ctx)?;
        info!(platform = platform.id(), path = %mcp.display(), "mcp config written");
    }
    if !skip_hooks {
        if let Some(hooks) = adapter.write_hooks(ctx)? {
            info!(platform = platform.id(), path = %hooks.display(), "hooks written");
        }
    }
    Ok(())
}

fn make_bar(n: u64) -> ProgressBar {
    let bar = ProgressBar::new(n);
    bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.cyan} [{bar:24.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    bar.enable_steady_tick(Duration::from_millis(80));
    bar
}

fn scope_label(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::Project => "project",
        InstallScope::User => "user",
        InstallScope::Global => "global",
    }
}

/// One row of the per-platform install report. Kept private to the module
/// so we can change its shape freely.
#[derive(Debug)]
struct InstallReport {
    platform: Platform,
    outcome: String,
}
