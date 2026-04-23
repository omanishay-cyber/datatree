//! Anthropic Claude Code adapter.
//!
//! Manifest: `CLAUDE.md` + `.claude/settings.json`
//! MCP: `.mcp.json` (project-scope) or `~/.claude.json` (user-scope)
//! Hooks: full hook surface — written into `.claude/settings.json`
//!
//! Claude Code is the only platform that gets the *full* 7-event hook map.

use serde_json::json;
use std::path::PathBuf;

use crate::error::CliResult;
use crate::platforms::{
    backup_then_write, AdapterContext, InstallScope, McpFormat, Platform, PlatformAdapter,
};

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

    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf {
        match ctx.scope {
            InstallScope::Project => ctx.project_root.join("CLAUDE.md"),
            InstallScope::User | InstallScope::Global => ctx.home.join("CLAUDE.md"),
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

    /// Claude Code reads hooks out of `.claude/settings.json`. We write the
    /// six events from design §6, plus PreCompact (§4) for compaction-aware
    /// step-ledger flushing.
    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        let settings_path = match ctx.scope {
            InstallScope::Project => ctx.project_root.join(".claude").join("settings.json"),
            InstallScope::User | InstallScope::Global => {
                ctx.home.join(".claude").join("settings.json")
            }
        };

        if let Some(parent) = settings_path.parent() {
            if !ctx.dry_run {
                std::fs::create_dir_all(parent)
                    .map_err(|e| crate::error::CliError::io(parent, e))?;
            }
        }

        let existing = if settings_path.exists() {
            std::fs::read_to_string(&settings_path)
                .map_err(|e| crate::error::CliError::io(&settings_path, e))?
        } else {
            "{}".into()
        };

        let mut value: serde_json::Value =
            serde_json::from_str(if existing.trim().is_empty() { "{}" } else { &existing })?;
        let root = value.as_object_mut().ok_or_else(|| {
            crate::error::CliError::Other("settings.json root is not an object".into())
        })?;

        let hooks = root
            .entry("hooks".to_string())
            .or_insert_with(|| json!({}));
        let hooks_obj = hooks
            .as_object_mut()
            .ok_or_else(|| crate::error::CliError::Other("`hooks` is not an object".into()))?;

        // The full Claude Code hook map for datatree.
        let dt_hooks = json!({
            "SessionStart":     [{ "command": "datatree session-prime", "owner": "datatree" }],
            "UserPromptSubmit": [{ "command": "datatree inject",       "owner": "datatree" }],
            "PreToolUse":       [{ "command": "datatree pre-tool",     "owner": "datatree" }],
            "PostToolUse":      [{ "command": "datatree post-tool",    "owner": "datatree" }],
            "Stop":             [{ "command": "datatree turn-end",     "owner": "datatree" }],
            "SessionEnd":       [{ "command": "datatree session-end",  "owner": "datatree" }],
            "PreCompact":       [{ "command": "datatree turn-end --pre-compact", "owner": "datatree" }],
            "SubagentStop":     [{ "command": "datatree turn-end --subagent",    "owner": "datatree" }]
        });

        for (event, hook_array) in dt_hooks.as_object().unwrap() {
            let target = hooks_obj
                .entry(event.clone())
                .or_insert_with(|| json!([]));
            let arr = target.as_array_mut().ok_or_else(|| {
                crate::error::CliError::Other(format!("hooks.{event} is not an array"))
            })?;
            // Drop any prior datatree-owned entry to keep this idempotent.
            arr.retain(|e| e.get("owner").and_then(|o| o.as_str()) != Some("datatree"));
            for entry in hook_array.as_array().unwrap() {
                arr.push(entry.clone());
            }
        }

        let serialized = serde_json::to_string_pretty(&value)? + "\n";
        if !ctx.dry_run {
            backup_then_write(&settings_path, serialized.as_bytes())?;
        }
        Ok(Some(settings_path))
    }
}
