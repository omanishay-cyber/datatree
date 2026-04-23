//! Qoder adapter.
//!
//! Manifest: `QODER.md`
//! MCP: `.qoder/mcp.json`
//! Hooks: `.qoder/settings.json` hooks.
//!
//! Per design §21.4.2: Qoder is "always tried" (returns true from detect).

use serde_json::json;
use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    backup_then_write, AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

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

    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        let path = match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".qoder").join("settings.json"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".qoder").join("settings.json")
            }
        };
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
            .ok_or_else(|| crate::error::CliError::Other("settings.json not object".into()))?;
        let hooks = root
            .entry("hooks".to_string())
            .or_insert_with(|| json!({}));
        let hooks_obj = hooks
            .as_object_mut()
            .ok_or_else(|| crate::error::CliError::Other("`hooks` is not an object".into()))?;
        hooks_obj.insert(
            "onTurnEnd".into(),
            json!({ "command": "mneme turn-end", "owner": "mneme" }),
        );
        hooks_obj.insert(
            "onSessionStart".into(),
            json!({ "command": "mneme session-prime", "owner": "mneme" }),
        );
        let serialized = serde_json::to_string_pretty(&value)? + "\n";
        if !ctx.dry_run {
            backup_then_write(&path, serialized.as_bytes())?;
        }
        Ok(Some(path))
    }
}
