//! Factory Droid adapter.
//!
//! Manifest: `AGENTS.md`
//! MCP: `~/.factory/mcp.json`
//! Hooks: invoked via `Task` tool subagents.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct FactoryDroid;

impl PlatformAdapter for FactoryDroid {
    fn platform(&self) -> Platform {
        Platform::FactoryDroid
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".factory").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home.join(".factory").join("mcp.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
