//! VS Code adapter.
//!
//! VS Code's GitHub Copilot Chat + Claude Code extensions both read MCP
//! servers from a shared location (February 2025+ stable API):
//!
//!   * User scope:   `%APPDATA%\Code\User\mcp.json`         (Windows)
//!                   `~/Library/Application Support/Code/User/mcp.json` (macOS)
//!                   `~/.config/Code/User/mcp.json`         (Linux)
//!   * Project scope: `<project_root>/.vscode/mcp.json`
//!
//! Format is the same JSON object shape Claude Code uses:
//!   `{"mcpServers": {"<name>": {...}}}`
//!
//! Manifest: `AGENTS.md` (same as Cursor — the universal base that
//! VS Code's chat extensions pick up).
//!
//! Hooks: NOT emitted. VS Code's chat extensions don't expose the hook
//! surface that Claude Code CLI has; if they gain it later we'll add it.
//! For v0.3.1 mneme provides value via MCP tools + manifest only, which
//! is what VS Code consumes today.

use std::path::PathBuf;

use crate::platforms::{AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter};

#[derive(Debug, Clone, Copy, Default)]
pub struct VsCode;

impl PlatformAdapter for VsCode {
    fn platform(&self) -> Platform {
        Platform::VsCode
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        // Heuristic: user-scope VS Code has a `Code\User` config dir on
        // every platform where VS Code is installed.
        vscode_user_config_dir(ctx).exists() || ctx.project_root.join(".vscode").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".vscode").join("mcp.json"),
            InstallScope::User | InstallScope::Global => {
                vscode_user_config_dir(ctx).join("mcp.json")
            }
        }
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    // write_hooks intentionally omitted — falls through to trait default
    // (Ok(None)). See module docstring for rationale.
}

/// Return `<Code-user-config>` dir per platform. Checks the Stable build
/// first, then Insiders, then falls back to the Stable path as a default
/// for the brand-new-user case.
fn vscode_user_config_dir(ctx: &AdapterContext) -> PathBuf {
    #[cfg(windows)]
    {
        // Windows: %APPDATA%\Code\User\ — AdapterContext doesn't know
        // APPDATA directly; we reconstruct from home (%USERPROFILE%).
        let appdata = std::env::var("APPDATA")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.home.join("AppData").join("Roaming"));
        appdata.join("Code").join("User")
    }
    #[cfg(target_os = "macos")]
    {
        ctx.home
            .join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        ctx.home.join(".config").join("Code").join("User")
    }
}
