//! Aider adapter.
//!
//! Manifest: `.aider.conf.yml` + `CONVENTIONS.md`
//! MCP: `.aider.conf.yml mcp_servers:` (YAML — but we treat as TOML-equivalent
//!      via the JSON-object family since Aider's mcp_servers list is fed as
//!      a YAML mapping; we still write the manifest body via markers in
//!      CONVENTIONS.md and leave the YAML mcp wiring for users to opt in).
//! Hooks: git hooks only (out of scope for the CLI installer).

use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct Aider;

impl PlatformAdapter for Aider {
    fn platform(&self) -> Platform {
        Platform::Aider
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        ctx.home.join(".aider.conf.yml").exists()
            || ctx.project_root.join(".aider.conf.yml").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("CONVENTIONS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("CONVENTIONS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".aider.conf.yml"),
            InstallScope::User | InstallScope::Global => ctx.home.join(".aider.conf.yml"),
        }
    }

    fn mcp_format(&self) -> McpFormat {
        // Aider's MCP is YAML-flavored; our default JSON-object merger is
        // unsafe here, so we intentionally do nothing in the default
        // `write_mcp_config` and override below to write a tiny YAML stub.
        McpFormat::JsonObject
    }

    fn write_mcp_config(&self, ctx: &AdapterContext) -> CliResult<PathBuf> {
        let path = self.mcp_config_path(ctx);
        if let Some(parent) = path.parent() {
            if !ctx.dry_run {
                std::fs::create_dir_all(parent)
                    .map_err(|e| crate::error::CliError::io(parent, e))?;
            }
        }
        let existing = if path.exists() {
            std::fs::read_to_string(&path).map_err(|e| crate::error::CliError::io(&path, e))?
        } else {
            String::new()
        };
        let snippet = "\n# mneme MCP server (added by `mneme install`)\n\
                       mcp_servers:\n  \
                         mneme:\n    \
                           command: mneme\n    \
                           args: [mcp, stdio]\n    \
                           env:\n      MNEME_LOG: info\n";
        if existing.contains("# mneme MCP server") {
            return Ok(path);
        }
        let merged = format!("{existing}{snippet}");
        if !ctx.dry_run {
            crate::platforms::backup_then_write(&path, merged.as_bytes())?;
        }
        Ok(path)
    }
}
