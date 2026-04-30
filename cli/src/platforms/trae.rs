//! Trae / Trae-CN adapter.
//!
//! Manifest: `AGENTS.md`
//! MCP: `~/.trae/mcp.json`
//! Hooks: none (no PreToolUse — AGENTS.md is always-on).

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Trae;

impl PlatformAdapter for Trae {
    fn platform(&self) -> Platform {
        Platform::Trae
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".trae").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home.join(".trae").join("mcp.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
