//! Codeium Windsurf adapter.
//!
//! Manifest: `.windsurfrules` + global rules
//! MCP: `~/.codeium/windsurf/mcp_config.json`
//! Hooks: workflows in `.windsurf/workflows/*.md` (no per-event hook system).

use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct Windsurf;

impl PlatformAdapter for Windsurf {
    fn platform(&self) -> Platform {
        Platform::Windsurf
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".codeium").join("windsurf").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".windsurfrules"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".windsurfrules"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    /// Drop a workflow file that wraps `datatree status` so users can
    /// invoke datatree from Windsurf's workflow palette.
    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        let workflows_dir = match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".windsurf").join("workflows"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".windsurf").join("workflows")
            }
        };
        if !ctx.dry_run {
            std::fs::create_dir_all(&workflows_dir)
                .map_err(|e| crate::error::CliError::io(&workflows_dir, e))?;
        }
        let workflow_path = workflows_dir.join("datatree.md");
        let body = "# Datatree Quick Status\n\n\
                    Run `datatree status` from this workflow to refresh the cached \
                    project graph and surface drift findings without leaving Windsurf.\n";
        if !ctx.dry_run {
            std::fs::write(&workflow_path, body)
                .map_err(|e| crate::error::CliError::io(&workflow_path, e))?;
        }
        Ok(Some(workflow_path))
    }
}
