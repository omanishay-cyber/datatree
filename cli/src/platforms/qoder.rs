//! Qoder adapter.
//!
//! Manifest: `QODER.md`
//! MCP: `.qoder/mcp.json`
//! Hooks: **not written** (v0.3.1 — closes NEW-004).
//!
//! Per design §21.4.2: Qoder is "always tried" (returns true from detect).
//!
//! Architecture note (v0.3.1 — 2026-04-25)
//! ========================================
//! Prior versions wrote `{ "command": ..., "owner": "mneme" }` entries
//! into `.qoder/settings.json hooks`. That is the EXACT shape that
//! triggered the v0.3.0 Claude-Code install catastrophe (see
//! `platforms/claude_code.rs` module docstring + report-002.md §F-011):
//! the schema validator rejects the unknown shape and silently inerts
//! every other hook the user had configured.
//!
//! Qoder's mneme surface is delivered via the MCP server, which Qoder
//! calls on demand. No hook injection is required for that flow.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Qoder;

impl PlatformAdapter for Qoder {
    fn platform(&self) -> Platform {
        Platform::Qoder
    }

    /// Qoder is always tried — see design §21.4.2.
    fn detect(&self, _ctx: &AdapterContext) -> bool {
        true
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("QODER.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("QODER.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".qoder").join("mcp.json"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".qoder").join("mcp.json"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    // `write_hooks` intentionally omitted — falls through to the trait's
    // default `Ok(None)` no-op. See module docstring above (NEW-004).
}
