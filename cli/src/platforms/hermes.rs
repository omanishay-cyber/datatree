//! Hermes adapter.
//!
//! Manifest: `AGENTS.md`
//! MCP: `.mcp.json` or `~/.hermes/mcp.json`
//! Hooks: Claude-compatible (we reuse the Claude Code event names).

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Hermes;

impl PlatformAdapter for Hermes {
    fn platform(&self) -> Platform {
        Platform::Hermes
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".hermes").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".mcp.json"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".hermes").join("mcp.json"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
