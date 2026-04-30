//! Google Antigravity adapter.
//!
//! Manifest: `AGENTS.md` + `GEMINI.md`
//! MCP: `~/.gemini/antigravity/mcp_config.json`
//! Hooks: built-in agent runtime (no per-event hooks).

use std::path::PathBuf;

use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct Antigravity;

impl PlatformAdapter for Antigravity {
    fn platform(&self) -> Platform {
        Platform::Antigravity
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".gemini").join("antigravity").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        // We use AGENTS.md as the canonical marker host; GEMINI.md is also
        // updated by the gemini_cli adapter. Avoiding a second update here
        // keeps a single source of truth.
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home
            .join(".gemini")
            .join("antigravity")
            .join("mcp_config.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
