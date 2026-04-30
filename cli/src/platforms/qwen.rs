//! Alibaba Qwen Code adapter.
//!
//! Manifest: `QWEN.md`
//! MCP: `~/.qwen/settings.json`
//! Hooks: none.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Qwen;

impl PlatformAdapter for Qwen {
    fn platform(&self) -> Platform {
        Platform::Qwen
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".qwen").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("QWEN.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("QWEN.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home.join(".qwen").join("settings.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
