//! OpenClaw adapter.
//!
//! Manifest: `CLAUDE.md` / `AGENTS.md`
//! MCP: `.mcp.json`
//! Hooks: none (sequential).

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenClaw;

impl PlatformAdapter for OpenClaw {
    fn platform(&self) -> Platform {
        Platform::OpenClaw
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".openclaw").exists() || ctx.project_root.join(".openclaw").exists()
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
            InstallScope::User | InstallScope::Global => ctx.home.join(".mcp.json"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }
}
