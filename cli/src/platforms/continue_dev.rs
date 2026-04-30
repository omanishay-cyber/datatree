//! Continue.dev adapter.
//!
//! Manifest: `.continuerc.json`
//! MCP: `~/.continue/config.json` `mcpServers` (JSON ARRAY)
//! Hooks: `.continue/hooks/` — limited set.

use std::path::PathBuf;

use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct ContinueDev;

impl PlatformAdapter for ContinueDev {
    fn platform(&self) -> Platform {
        Platform::ContinueDev
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".continue").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        // .continuerc.json is JSON, not Markdown — we use the universal
        // AGENTS.md as the marker host instead so we don't have to invent
        // a JSON-comment marker scheme.
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home.join(".continue").join("config.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonArray
    }
}
