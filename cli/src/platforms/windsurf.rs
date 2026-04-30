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

    /// Drop a workflow file that wraps `mneme status` so users can
    /// invoke mneme from Windsurf's workflow palette.
    ///
    /// **M1 (audit DEEP-AUDIT-2026-04-29.md §M1):** Honors
    /// `ctx.enable_hooks=false` exactly the way `claude_code.rs:211`
    /// does. Without this gate, `mneme install --no-hooks --platform
    /// windsurf` would still write `.windsurf/workflows/mneme.md`,
    /// breaking the K1 contract documented at
    /// `cli/src/platforms/mod.rs:291`.
    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        if !ctx.enable_hooks {
            return Ok(None);
        }
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
        let workflow_path = workflows_dir.join("mneme.md");
        let body = "# Mneme Quick Status\n\n\
                    Run `mneme status` from this workflow to refresh the cached \
                    project graph and surface drift findings without leaving Windsurf.\n";
        if !ctx.dry_run {
            std::fs::write(&workflow_path, body)
                .map_err(|e| crate::error::CliError::io(&workflow_path, e))?;
        }
        Ok(Some(workflow_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// M1 (audit DEEP-AUDIT-2026-04-29.md §M1): non-Claude adapters must
    /// honor `ctx.enable_hooks=false` exactly the way `claude_code.rs:211`
    /// does. Without the gate, `mneme install --no-hooks --platform
    /// windsurf` would still write `.windsurf/workflows/mneme.md`.
    #[test]
    fn write_hooks_no_op_when_not_enabled() {
        let dir = tempdir().unwrap();
        let mut ctx = AdapterContext::new(InstallScope::User, dir.path().to_path_buf());
        ctx.home = dir.path().to_path_buf();
        assert!(!ctx.enable_hooks, "default must be opt-out");

        let result = Windsurf.write_hooks(&ctx).unwrap();
        assert!(
            result.is_none(),
            "windsurf with enable_hooks=false must return Ok(None)"
        );
        let workflow_path = dir
            .path()
            .join(".windsurf")
            .join("workflows")
            .join("mneme.md");
        assert!(
            !workflow_path.exists(),
            "windsurf with enable_hooks=false must NOT write {}",
            workflow_path.display()
        );
    }

    #[test]
    fn write_hooks_writes_file_when_enabled() {
        let dir = tempdir().unwrap();
        let mut ctx = AdapterContext::new(InstallScope::User, dir.path().to_path_buf())
            .with_enable_hooks(true);
        ctx.home = dir.path().to_path_buf();

        let result = Windsurf.write_hooks(&ctx).unwrap();
        let workflow_path = dir
            .path()
            .join(".windsurf")
            .join("workflows")
            .join("mneme.md");
        assert_eq!(
            result.as_deref(),
            Some(workflow_path.as_path()),
            "windsurf with enable_hooks=true must return the workflow path"
        );
        assert!(
            workflow_path.exists(),
            "windsurf with enable_hooks=true must write {}",
            workflow_path.display()
        );
    }
}

