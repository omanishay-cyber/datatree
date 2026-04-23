//! GitHub Copilot CLI / VS Code adapter.
//!
//! Manifest: `.github/copilot-instructions.md`
//! MCP: `.vscode/mcp.json` or `~/.config/github-copilot/mcp.json`
//! Hooks: VS Code task hooks (no datatree-managed file).

use std::path::PathBuf;

use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct Copilot;

impl PlatformAdapter for Copilot {
    fn platform(&self) -> Platform {
        Platform::Copilot
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".config").join("github-copilot").exists()
            || ctx.home.join(".vscode").exists()
            || ctx.project_root.join(".vscode").exists()
            || ctx.project_root.join(".github").join("copilot-instructions.md").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx
                .project_root
                .join(".github")
                .join("copilot-instructions.md"),
            InstallScope::User | InstallScope::Global => ctx
                .home
                .join(".config")
                .join("github-copilot")
                .join("copilot-instructions.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".vscode").join("mcp.json"),
            InstallScope::User | InstallScope::Global => ctx
                .home
                .join(".config")
                .join("github-copilot")
                .join("mcp.json"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
