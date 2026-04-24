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
use crate::receipts::{sha256_of_file, Receipt, ReceiptAction};

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
    ///
    /// `--no-hooks` is accepted as a clearer alias for the same behaviour.
    #[arg(long, alias = "no-hooks")]
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

    let targets: Vec<Platform> = match args.platform.as_deref() {
        Some(id) => vec![Platform::from_id(id)?],
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

    // Gentle guardrail — warn if Claude Code appears to be running and
    // we're about to modify its files. In v0.3.1 MCP-only installs are
    // safe while CC is running (CC re-reads mcpServers on next launch),
    // but manifest writes can create stale cached state. Not a block;
    // just a heads-up.
    let claude_is_target = targets
        .iter()
        .any(|p| matches!(p, Platform::ClaudeCode));
    let writing_anything_but_mcp = !args.skip_manifest || !args.skip_hooks;
    if claude_is_target && writing_anything_but_mcp && !args.dry_run {
        if claude_code_likely_running() {
            warn!(
                "Claude Code appears to be running — close it before \
                 re-launching so it picks up mneme cleanly. Continuing."
            );
        }
    }

    let bar = make_bar(targets.len() as u64);
    let mut report: Vec<InstallReport> = Vec::with_capacity(targets.len());

    // Receipt — records every file write, MCP registration, etc.
    // Persisted at `~/.mneme/install-receipts/<stamp>-<id>.json` at the
    // end of the install so `mneme rollback` can reverse it atomically.
    // Only written on non-dry-run.
    let mut receipt = Receipt::new();

    for platform in targets {
        bar.set_message(platform.display_name().to_string());
        let r = install_one(
            platform,
            &ctx,
            args.skip_mcp,
            args.skip_hooks,
            args.skip_manifest,
            &mut receipt,
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

    // Persist the receipt so `mneme rollback` can reverse this install.
    // Skipped in dry-run — no writes happened so no rollback is possible.
    if !args.dry_run && !receipt.actions.is_empty() {
        match receipt.save() {
            Ok(path) => info!(path = %path.display(), "install receipt written"),
            Err(e) => warn!(error = %e, "failed to write install receipt (install succeeded but rollback will be manual)"),
        }
    }

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
///
/// Each write records a [`ReceiptAction`] into `receipt` so
/// `mneme rollback` can reverse this install later.
fn install_one(
    platform: Platform,
    ctx: &AdapterContext,
    skip_mcp: bool,
    skip_hooks: bool,
    skip_manifest: bool,
    receipt: &mut Receipt,
) -> CliResult<()> {
    let adapter = platform.adapter();

    if !skip_manifest {
        let target = adapter.manifest_path(ctx);
        let existed_before = target.exists();
        let sha_before = if existed_before { sha256_of_file(&target) } else { String::new() };

        let manifest = adapter.write_manifest(ctx)?;
        info!(platform = platform.id(), path = %manifest.display(), "manifest written");

        if !ctx.dry_run {
            let sha_after = sha256_of_file(&manifest);
            if existed_before {
                if let Some(backup) = find_latest_mneme_bak(&manifest) {
                    receipt.push(ReceiptAction::FileModified {
                        path: manifest.clone(),
                        backup_path: backup,
                        sha256_before: sha_before,
                        sha256_after: sha_after,
                    });
                }
            } else {
                receipt.push(ReceiptAction::FileCreated {
                    path: manifest.clone(),
                    sha256_after: sha_after,
                });
            }
        }
    } else {
        info!(platform = platform.id(), "manifest skipped (--skip-manifest)");
    }

    if !skip_mcp {
        let target = adapter.mcp_config_path(ctx);
        let existed_before = target.exists();
        let sha_before = if existed_before { sha256_of_file(&target) } else { String::new() };

        let mcp = adapter.write_mcp_config(ctx)?;
        info!(platform = platform.id(), path = %mcp.display(), "mcp config written");

        if !ctx.dry_run {
            let sha_after = sha256_of_file(&mcp);
            if existed_before {
                if let Some(backup) = find_latest_mneme_bak(&mcp) {
                    receipt.push(ReceiptAction::FileModified {
                        path: mcp.clone(),
                        backup_path: backup,
                        sha256_before: sha_before,
                        sha256_after: sha_after,
                    });
                }
            } else {
                receipt.push(ReceiptAction::FileCreated {
                    path: mcp.clone(),
                    sha256_after: sha_after,
                });
            }
            // Also record the MCP registration semantically — lets
            // `mneme rollback` strip the mneme entry specifically
            // without touching neighbors in mcp_config_path.
            receipt.push(ReceiptAction::McpRegistered {
                platform: platform.id().to_string(),
                host_file: mcp.clone(),
            });
        }
    }

    if !skip_hooks {
        if let Some(hooks) = adapter.write_hooks(ctx)? {
            info!(platform = platform.id(), path = %hooks.display(), "hooks written");
            // If any platform adapter starts writing hooks in the future,
            // record them here. In v0.3.1 the ClaudeCode adapter returns
            // None (see `platforms/claude_code.rs` module docstring) so
            // this branch is effectively unreachable for Claude Code.
            if !ctx.dry_run {
                let sha_after = sha256_of_file(&hooks);
                if let Some(backup) = find_latest_mneme_bak(&hooks) {
                    receipt.push(ReceiptAction::FileModified {
                        path: hooks.clone(),
                        backup_path: backup,
                        sha256_before: String::new(),
                        sha256_after: sha_after,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Find the newest `<target>.mneme-YYYYMMDD-HHMMSS.bak` alongside `target`
/// — that's the timestamped backup `backup_then_write` just created.
/// Returns None if no such file exists (e.g. target is a fresh create).
fn find_latest_mneme_bak(target: &std::path::Path) -> Option<PathBuf> {
    let parent = target.parent()?;
    let stem = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|name| {
                    // Match <stem>.mneme-<timestamp>.bak where stem = target's filename
                    // or the pre-extension stem (we accept both shapes).
                    name.starts_with(&format!("{stem}.mneme-"))
                        || name.starts_with(&format!(
                            "{}.mneme-",
                            target
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                        ))
                })
                .unwrap_or(false)
        })
        .collect();
    candidates.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    candidates.into_iter().next()
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

/// Lightweight probe: is Claude Code likely running on this host?
///
/// Windows: shells out to `tasklist /FI "IMAGENAME eq Claude.exe"` and
/// checks stdout for the executable name. Falsy on any error (the probe
/// is advisory, not authoritative).
///
/// Unix: shells out to `pgrep -f claude` and checks the exit code. Same
/// falsy-on-error semantics.
///
/// This is deliberately a *warning* not a *block* — authoritative
/// process checks require more plumbing (sysinfo crate, elevated lookups
/// on Windows) than the v0.3.1 scope allows. The architectural fix for
/// the v0.3.0 settings.json poisoning (see `platforms/claude_code.rs`)
/// means even running-Claude-Code installs are safe today; this probe
/// exists to prevent the cosmetic "stale config in memory" issue.
fn claude_code_likely_running() -> bool {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Claude.exe", "/NH", "/FO", "CSV"])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            // tasklist emits `INFO: No tasks are running which match...`
            // on stdout when nothing matches. A positive hit contains
            // the image name in one of the CSV fields.
            return s.contains("Claude.exe") || s.contains("claude.exe");
        }
        false
    }
    #[cfg(not(windows))]
    {
        let out = std::process::Command::new("pgrep")
            .args(["-f", "claude"])
            .output();
        if let Ok(o) = out {
            return o.status.success() && !o.stdout.is_empty();
        }
        false
    }
}
