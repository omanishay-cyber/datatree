//! Cursor adapter.
//!
//! Manifest: `.cursor/rules/*.mdc` + `AGENTS.md`
//! MCP: `.cursor/mcp.json`
//! Hooks: `~/.cursor/hooks.json` — afterFileEdit, sessionStart, beforeShellExecution.

use serde_json::json;
use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    backup_then_write, AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

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

    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        let path = ctx.home.join(".cursor").join("hooks.json");
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
        let mut value: serde_json::Value =
            serde_json::from_str(if existing.trim().is_empty() { "{}" } else { &existing })?;
        let root = value
            .as_object_mut()
            .ok_or_else(|| crate::error::CliError::Other("hooks.json root is not object".into()))?;

        let entries = json!({
            "sessionStart":          [{ "command": "mneme session-prime", "owner": "mneme" }],
            "afterFileEdit":         [{ "command": "mneme post-tool",     "owner": "mneme" }],
            "beforeShellExecution":  [{ "command": "mneme pre-tool",      "owner": "mneme" }]
        });
        for (event, arr) in entries.as_object().unwrap() {
            let target = root.entry(event.clone()).or_insert_with(|| json!([]));
            let target_arr = target.as_array_mut().ok_or_else(|| {
                crate::error::CliError::Other(format!("{event} is not an array"))
            })?;
            target_arr.retain(|e| e.get("owner").and_then(|o| o.as_str()) != Some("mneme"));
            for entry in arr.as_array().unwrap() {
                target_arr.push(entry.clone());
            }
        }

        let serialized = serde_json::to_string_pretty(&value)? + "\n";
        if !ctx.dry_run {
            backup_then_write(&path, serialized.as_bytes())?;
        }
        Ok(Some(path))
    }
}
