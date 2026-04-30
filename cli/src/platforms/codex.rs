//! OpenAI Codex adapter.
//!
//! Manifest: `AGENTS.md`
//! MCP: `~/.codex/config.toml` (TOML, not JSON)
//! Hooks: subagent dispatch via `multi_agent=true` — no per-event hooks.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Codex;

impl PlatformAdapter for Codex {
    fn platform(&self) -> Platform {
        Platform::Codex
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".codex").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home.join(".codex").join("config.toml")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::Toml
    }
}
