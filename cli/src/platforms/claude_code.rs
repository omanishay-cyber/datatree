//! Anthropic Claude Code adapter.
//!
//! Manifest: `CLAUDE.md` (optional @include line, opt-in via `mneme link-claude-md`)
//! MCP:      `~/.claude.json` (user-scope) or `.mcp.json` (project-scope)
//! Hooks:    **not written**. Mneme never touches `~/.claude/settings.json`.
//!
//! Architecture note (v0.3.1 — 2026-04-24)
//! ========================================
//! Prior versions wrote an 8-event hook map into `~/.claude/settings.json`.
//! That code emitted a flat `{command, owner}` shape which Claude Code's
//! schema validator rejected, causing the validator to discard the entire
//! file (not just the malformed entries). Every unrelated hook, permission,
//! and plugin the user had configured became silently inert on next boot.
//!
//! The self-trap was amplified because mneme's hook binaries required
//! `--tool / --params / --session-id` CLI flags, while Claude Code delivers
//! payload on STDIN as JSON. Every PreToolUse call exited non-zero, which
//! Claude Code correctly interpreted as BLOCK — locking the agent out of
//! every tool, including the ones it needed to roll mneme back.
//!
//! Fix (v0.3.1): we simply do not register hooks with Claude Code anymore.
//! The critical mneme surface (persistent memory, recall, blast, step
//! ledger) is delivered via the MCP server, which Claude Code calls on
//! demand. The hook binaries (`mneme pre-tool`, `mneme inject`, etc.) still
//! exist in the CLI so power users can wire them manually once we ship
//! STDIN-parsing support in v0.3.2, but they are **not** auto-registered.
//!
//! What this means for the user:
//!   - `mneme install --platform claude-code` writes ONLY two things:
//!       1. an MCP server entry in `~/.claude.json` (tiny, schema-validated)
//!       2. optionally (behind `--link-claude-md`) a single @include line in
//!          `~/.claude/CLAUDE.md` pointing at `~/.mneme/CLAUDE.md`
//!   - `~/.claude/settings.json` is never touched.
//!   - No hook schema mismatch is possible because no hook is emitted.
//!
//! See `report-002.md §F-011 / §F-012` in the mneme install-report for the
//! forensic record of the v0.3.0 incident this fix prevents.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

/// Adapter struct (zero-sized; all behaviour is in the impl).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeCode;

impl PlatformAdapter for ClaudeCode {
    fn platform(&self) -> Platform {
        Platform::ClaudeCode
    }

    /// Per design §21.4.2, Claude Code is "always tried" — the detector
    /// returns true unconditionally so even fresh users get configured.
    fn detect(&self, _ctx: &AdapterContext) -> bool {
        true
    }

    /// Manifest location — v0.3.1 fix for F-008 / F-017.
    ///
    /// Claude Code reads user-scope instructions from `~/.claude/CLAUDE.md`.
    /// Earlier mneme versions wrote to `~/CLAUDE.md` (the user's home
    /// directory root), which is a different file — it's only loaded by
    /// Claude Code when it happens to be the current working directory
    /// project file. That caused the manifest to be picked up
    /// inconsistently and left a stray `CLAUDE.md` in user homes across
    /// projects. Correct target is `~/.claude/CLAUDE.md`.
    ///
    /// Project-scope stays `<project_root>/CLAUDE.md` (that IS the
    /// correct per-project file).
    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("CLAUDE.md"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".claude").join("CLAUDE.md")
            }
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".mcp.json"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".claude.json"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    // `write_hooks` intentionally omitted — falls through to the trait's
    // default `Ok(None)` no-op. See the module docstring above for the
    // full architectural rationale. DO NOT restore hook-injection into
    // `~/.claude/settings.json` without first fixing all of:
    //   (a) hook JSON schema (must use `{matcher?, hooks:[{type,command}]}`)
    //   (b) hook binary STDIN contract (payload via stdin, not CLI flags)
    //   (c) rollback receipt (sha256 snapshot + byte-for-byte uninstall)
    //   (d) `settings.json` lock — refuse write if Claude Code is running
    //   (e) a `--no-hooks` escape hatch for recovery
    // All five land together, or none do. See `NOT-PROPERLY-DONE.md`.
}
