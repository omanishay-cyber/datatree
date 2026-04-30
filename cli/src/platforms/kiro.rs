//! AWS Kiro adapter.
//!
//! Manifest: `.kiro/steering/*.md`
//! MCP: `.kiro/settings/mcp.json`
//! Hooks: `.kiro/hooks/*.kiro.hook`.

use serde_json::json;
use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Kiro;

impl PlatformAdapter for Kiro {
    fn platform(&self) -> Platform {
        Platform::Kiro
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.project_root.join(".kiro").exists() || ctx.home.join(".kiro").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx
                .project_root
                .join(".kiro")
                .join("steering")
                .join("mneme.md"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".kiro").join("steering").join("mneme.md")
            }
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx
                .project_root
                .join(".kiro")
                .join("settings")
                .join("mcp.json"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".kiro").join("settings").join("mcp.json")
            }
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    /// **M1 (audit DEEP-AUDIT-2026-04-29.md §M1):** Honors
    /// `ctx.enable_hooks=false` exactly the way `claude_code.rs:211`
    /// does. Without this gate, `mneme install --no-hooks --platform
    /// kiro` would still write `.kiro/hooks/mneme.kiro.hook`,
    /// breaking the K1 contract documented at
    /// `cli/src/platforms/mod.rs:291` ("false → write_hooks is a
    /// no-op").
    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        if !ctx.enable_hooks {
            return Ok(None);
        }
        let hooks_dir = match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".kiro").join("hooks"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".kiro").join("hooks"),
        };
        if !ctx.dry_run {
            std::fs::create_dir_all(&hooks_dir)
                .map_err(|e| crate::error::CliError::io(&hooks_dir, e))?;
        }
        let hook_path = hooks_dir.join("mneme.kiro.hook");
        // I-1: absolute path so Kiro doesn't depend on PATH.
        let exe = ctx.exe_path.to_string_lossy().into_owned();
        let body = json!({
            "name": "mneme",
            "events": ["onSessionStart", "onFileEdited", "onTurnEnd"],
            "command": exe,
            "argTemplate": ["session-prime"]
        });
        let serialized = serde_json::to_string_pretty(&body)? + "\n";
        if !ctx.dry_run {
            std::fs::write(&hook_path, serialized)
                .map_err(|e| crate::error::CliError::io(&hook_path, e))?;
        }
        Ok(Some(hook_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// M1 (audit DEEP-AUDIT-2026-04-29.md §M1): non-Claude adapters must
    /// honor `ctx.enable_hooks=false` exactly the way `claude_code.rs:211`
    /// does. Without the gate, `mneme install --no-hooks --platform
    /// kiro` would still write `.kiro/hooks/mneme.kiro.hook`.
    #[test]
    fn write_hooks_no_op_when_not_enabled() {
        let dir = tempdir().unwrap();
        let mut ctx = AdapterContext::new(InstallScope::User, dir.path().to_path_buf());
        ctx.home = dir.path().to_path_buf();
        assert!(!ctx.enable_hooks, "default must be opt-out");

        let result = Kiro.write_hooks(&ctx).unwrap();
        assert!(
            result.is_none(),
            "kiro with enable_hooks=false must return Ok(None)"
        );
        let hook_path = dir
            .path()
            .join(".kiro")
            .join("hooks")
            .join("mneme.kiro.hook");
        assert!(
            !hook_path.exists(),
            "kiro with enable_hooks=false must NOT write {}",
            hook_path.display()
        );
    }

    #[test]
    fn write_hooks_writes_file_when_enabled() {
        let dir = tempdir().unwrap();
        let mut ctx = AdapterContext::new(InstallScope::User, dir.path().to_path_buf())
            .with_enable_hooks(true);
        ctx.home = dir.path().to_path_buf();

        let result = Kiro.write_hooks(&ctx).unwrap();
        let hook_path = dir
            .path()
            .join(".kiro")
            .join("hooks")
            .join("mneme.kiro.hook");
        assert_eq!(
            result.as_deref(),
            Some(hook_path.as_path()),
            "kiro with enable_hooks=true must return the hook path"
        );
        assert!(
            hook_path.exists(),
            "kiro with enable_hooks=true must write {}",
            hook_path.display()
        );
    }
}
