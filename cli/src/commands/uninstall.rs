//! `mneme uninstall` — reverse of [`install`](super::install).
//!
//! Strips the marker block from each manifest and removes the `mneme`
//! entry from each MCP config. Safe to re-run; non-mneme content is
//! preserved verbatim.

use clap::Args;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::error::CliResult;
use crate::platforms::{
    AdapterContext, InstallScope, Platform, PlatformDetector,
};

/// CLI args for `mneme uninstall`.
#[derive(Debug, Args)]
pub struct UninstallArgs {
    /// Restrict to a single platform; otherwise every detected platform.
    #[arg(long)]
    pub platform: Option<String>,

    /// Print what would change but write nothing.
    #[arg(long)]
    pub dry_run: bool,

    /// Scope (must match what was used at install time).
    #[arg(long, default_value = "user")]
    pub scope: String,

    /// Project root override.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: UninstallArgs) -> CliResult<()> {
    let scope: InstallScope = args.scope.parse()?;
    let project_root = args
        .project
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let ctx = AdapterContext::new(scope, project_root.clone()).with_dry_run(args.dry_run);

    let targets: Vec<Platform> = match args.platform {
        Some(id) => vec![Platform::from_id(&id)?],
        None => PlatformDetector::detect_installed(scope, &project_root),
    };

    if targets.is_empty() {
        warn!("no platforms detected");
        return Ok(());
    }

    for platform in targets {
        let adapter = platform.adapter();
        if let Err(e) = adapter.remove_manifest(&ctx) {
            warn!(platform = platform.id(), error = %e, "remove_manifest failed");
        } else {
            info!(platform = platform.id(), "manifest cleaned");
        }
        if let Err(e) = adapter.remove_mcp_config(&ctx) {
            warn!(platform = platform.id(), error = %e, "remove_mcp_config failed");
        } else {
            info!(platform = platform.id(), "mcp entry removed");
        }
    }

    if args.dry_run {
        println!("(dry-run: no files were written)");
    }
    Ok(())
}
