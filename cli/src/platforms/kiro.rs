//! AWS Kiro adapter.
//!
//! Manifest: `.kiro/steering/*.md`
//! MCP: `.kiro/settings/mcp.json`
//! Hooks: `.kiro/hooks/*.kiro.hook`.

use serde_json::json;
use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

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
                .join("datatree.md"),
            InstallScope::User | InstallScope::Global => ctx
                .home
                .join(".kiro")
                .join("steering")
                .join("datatree.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx
                .project_root
                .join(".kiro")
                .join("settings")
                .join("mcp.json"),
            InstallScope::User | InstallScope::Global => ctx
                .home
                .join(".kiro")
                .join("settings")
                .join("mcp.json"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        let hooks_dir = match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".kiro").join("hooks"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".kiro").join("hooks"),
        };
        if !ctx.dry_run {
            std::fs::create_dir_all(&hooks_dir)
                .map_err(|e| crate::error::CliError::io(&hooks_dir, e))?;
        }
        let hook_path = hooks_dir.join("datatree.kiro.hook");
        let body = json!({
            "name": "datatree",
            "events": ["onSessionStart", "onFileEdited", "onTurnEnd"],
            "command": "datatree",
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
