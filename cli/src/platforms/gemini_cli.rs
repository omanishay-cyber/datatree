//! Google Gemini CLI adapter.
//!
//! Manifest: `GEMINI.md`
//! MCP: `~/.gemini/settings.json`
//! Hooks: custom commands TOML in `~/.gemini/commands/`.

use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct GeminiCli;

impl PlatformAdapter for GeminiCli {
    fn platform(&self) -> Platform {
        Platform::GeminiCli
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".gemini").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("GEMINI.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("GEMINI.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        ctx.home.join(".gemini").join("settings.json")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    /// Drop a small TOML command that surfaces datatree under `/datatree`
    /// in Gemini CLI.
    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        let cmd_dir = ctx.home.join(".gemini").join("commands");
        if !ctx.dry_run {
            std::fs::create_dir_all(&cmd_dir)
                .map_err(|e| crate::error::CliError::io(&cmd_dir, e))?;
        }
        let cmd_path = cmd_dir.join("datatree.toml");
        let body = "name = \"datatree\"\n\
                    description = \"Run a datatree command (status / recall / blast / step ...)\"\n\
                    command = \"datatree\"\n\
                    args = [\"$@\"]\n";
        if !ctx.dry_run {
            std::fs::write(&cmd_path, body)
                .map_err(|e| crate::error::CliError::io(&cmd_path, e))?;
        }
        Ok(Some(cmd_path))
    }
}
