//! Cursor adapter.
//!
//! Manifest: `.cursor/rules/*.mdc` + `AGENTS.md`
//! MCP: `.cursor/mcp.json`
//! Hooks: **not written** (v0.3.1 — closes NEW-004).
//!
//! Architecture note (v0.3.1 — 2026-04-25)
//! ========================================
//! Prior versions wrote `{ "command": ..., "owner": "mneme" }` entries
//! into `~/.cursor/hooks.json`. That is the EXACT shape that triggered
//! the v0.3.0 Claude-Code install catastrophe (see
//! `platforms/claude_code.rs` module docstring + report-002.md §F-011):
//! the schema validator rejects the unknown shape and silently inerts
//! every other hook the user had configured.
//!
//! Cursor's mneme surface (recall, blast, step ledger, etc.) is
//! delivered via the MCP server, which Cursor calls on demand from its
//! own UI. No hooks are required for that flow to work.
//!
//! See `platforms/claude_code.rs` for the detailed checklist of what
//! must land before any platform's hook adapter is restored. Same five
//! bullets apply here.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor;

impl PlatformAdapter for Cursor {
    fn platform(&self) -> Platform {
        Platform::Cursor
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".cursor").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        // We write into AGENTS.md (the universal base) — .cursor/rules/*.mdc
        // is per-rule and not where we want a marker block.
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".cursor").join("mcp.json"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".cursor").join("mcp.json")
            }
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    // `write_hooks` intentionally omitted — falls through to the trait's
    // default `Ok(None)` no-op. See module docstring above for the full
    // rationale (NEW-004: schema-poisoning regression risk).
}
