//! Zed adapter.
//!
//! Manifest: `AGENTS.md`
//! MCP: `~/.config/zed/settings.json` `context_servers`
//! Hooks: Zed extension API (no file-based hook config).

use serde_json::json;
use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    backup_then_write, AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct Zed;

impl Zed {
    fn settings_path(home: &std::path::Path) -> PathBuf {
        // Zed reads from ~/.config/zed/settings.json on Unix and the
        // platform-equivalent on macOS / Windows. We stick to the .config
        // path which Zed itself respects on every platform.
        home.join(".config").join("zed").join("settings.json")
    }
}

impl PlatformAdapter for Zed {
    fn platform(&self) -> Platform {
        Platform::Zed
    }

    fn detect(&self, ctx: &AdapterContext) -> bool {
        Self::settings_path(&ctx.home).exists() || ctx.home.join(".config").join("zed").exists()
    }

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("AGENTS.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("AGENTS.md"),
        }
    }

    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf {
        Self::settings_path(&ctx.home)
    }

    fn mcp_format(&self) -> McpFormat {
        // Zed nests under `context_servers`, but the family is JSON object.
        // We override write_mcp_config below to put it under the right key.
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
            "{}".into()
        };
        let mut value: serde_json::Value = serde_json::from_str(if existing.trim().is_empty() {
            "{}"
        } else {
            &existing
        })?;
        let root = value
            .as_object_mut()
            .ok_or_else(|| crate::error::CliError::Other("settings.json not object".into()))?;
        let servers = root
            .entry("context_servers".to_string())
            .or_insert_with(|| json!({}));
        let servers_obj = servers.as_object_mut().ok_or_else(|| {
            crate::error::CliError::Other("`context_servers` is not an object".into())
        })?;
        // I-1: write the absolute path of the running mneme binary so
        // Zed doesn't depend on `mneme` being on PATH at launch time.
        let exe = ctx.exe_path.to_string_lossy().into_owned();
        servers_obj.insert(
            "mneme".into(),
            json!({
                "command": { "path": exe, "args": ["mcp", "stdio"] },
                "settings": {}
            }),
        );
        let serialized = serde_json::to_string_pretty(&value)? + "\n";
        if !ctx.dry_run {
            backup_then_write(&path, serialized.as_bytes())?;
        }
        Ok(path)
    }
}
