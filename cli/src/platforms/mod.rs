//! The 18 AI-platform integration matrix.
//!
//! Each platform module under this dir implements a small set of
//! adapters that the [`install`](crate::commands::install) and
//! [`uninstall`](crate::commands::uninstall) commands call:
//!
//! ```text
//! detect()           -> bool      // is this platform installed for the user?
//! manifest_path()    -> PathBuf   // where to write AGENTS.md / CLAUDE.md / etc.
//! mcp_config_path()  -> PathBuf   // where the MCP server registry lives
//! write_manifest()                // marker-injects mneme's section
//! write_mcp_config()              // backs up + merges mneme MCP entry
//! write_hooks()                   // platforms that support hooks only
//! ```
//!
//! All MCP configs are written via family-aware merge: most platforms expect
//! a JSON object under `mcpServers`, Continue uses a JSON array, Codex uses
//! TOML. See design §21.4 + §21.4.1.
//!
//! ## Adding a new platform
//!
//! 1. add a module file under `src/platforms/`
//! 2. implement [`PlatformAdapter`] for it
//! 3. add the variant to [`Platform`]
//! 4. register it in [`PlatformDetector::all_known`] and the `match` arms in
//!    [`Platform::adapter`]

pub mod aider;
pub mod antigravity;
pub mod claude_code;
pub mod continue_dev;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod factory_droid;
pub mod gemini_cli;
pub mod hermes;
pub mod kiro;
pub mod openclaw;
pub mod opencode;
pub mod qoder;
pub mod qwen;
pub mod trae;
pub mod windsurf;
pub mod zed;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{CliError, CliResult};

/// Install scope from `--scope=...`. Determines where manifests live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    /// Affects only the active project (`<repo>/.claude/`, `<repo>/AGENTS.md`).
    Project,
    /// Affects the current OS user across all their projects.
    User,
    /// Affects every user on the machine. Requires admin/root for some paths.
    Global,
}

impl std::str::FromStr for InstallScope {
    type Err = CliError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "project" => Ok(InstallScope::Project),
            "user" => Ok(InstallScope::User),
            "global" => Ok(InstallScope::Global),
            other => Err(CliError::InvalidScope(other.to_string())),
        }
    }
}

/// One platform supported by `mneme install`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    /// Anthropic Claude Code.
    ClaudeCode,
    /// OpenAI Codex.
    Codex,
    /// Cursor.
    Cursor,
    /// Codeium Windsurf.
    Windsurf,
    /// Zed editor.
    Zed,
    /// Continue.dev.
    ContinueDev,
    /// SST OpenCode.
    OpenCode,
    /// Google Antigravity.
    Antigravity,
    /// Google Gemini CLI.
    GeminiCli,
    /// Aider.
    Aider,
    /// GitHub Copilot CLI / VS Code.
    Copilot,
    /// Factory Droid.
    FactoryDroid,
    /// Trae / Trae-CN.
    Trae,
    /// AWS Kiro.
    Kiro,
    /// Qoder.
    Qoder,
    /// OpenClaw.
    OpenClaw,
    /// Hermes.
    Hermes,
    /// Alibaba Qwen Code.
    Qwen,
}

impl Platform {
    /// Stable kebab-case identifier (used in CLI flags + log lines).
    pub fn id(self) -> &'static str {
        match self {
            Platform::ClaudeCode => "claude-code",
            Platform::Codex => "codex",
            Platform::Cursor => "cursor",
            Platform::Windsurf => "windsurf",
            Platform::Zed => "zed",
            Platform::ContinueDev => "continue",
            Platform::OpenCode => "opencode",
            Platform::Antigravity => "antigravity",
            Platform::GeminiCli => "gemini-cli",
            Platform::Aider => "aider",
            Platform::Copilot => "copilot",
            Platform::FactoryDroid => "factory-droid",
            Platform::Trae => "trae",
            Platform::Kiro => "kiro",
            Platform::Qoder => "qoder",
            Platform::OpenClaw => "openclaw",
            Platform::Hermes => "hermes",
            Platform::Qwen => "qwen",
        }
    }

    /// Pretty display name for log output.
    pub fn display_name(self) -> &'static str {
        match self {
            Platform::ClaudeCode => "Claude Code",
            Platform::Codex => "Codex",
            Platform::Cursor => "Cursor",
            Platform::Windsurf => "Windsurf",
            Platform::Zed => "Zed",
            Platform::ContinueDev => "Continue.dev",
            Platform::OpenCode => "OpenCode",
            Platform::Antigravity => "Antigravity",
            Platform::GeminiCli => "Gemini CLI",
            Platform::Aider => "Aider",
            Platform::Copilot => "GitHub Copilot",
            Platform::FactoryDroid => "Factory Droid",
            Platform::Trae => "Trae",
            Platform::Kiro => "Kiro",
            Platform::Qoder => "Qoder",
            Platform::OpenClaw => "OpenClaw",
            Platform::Hermes => "Hermes",
            Platform::Qwen => "Qwen Code",
        }
    }

    /// Look up by stable id from `--platform=<id>`.
    pub fn from_id(id: &str) -> CliResult<Self> {
        for p in Self::all_known() {
            if p.id().eq_ignore_ascii_case(id) {
                return Ok(*p);
            }
        }
        Err(CliError::UnknownPlatform(id.to_string()))
    }

    /// Static list of every platform mneme v1.0 supports. Kept in lock-
    /// step with the table in design §21.4.
    pub fn all_known() -> &'static [Platform] {
        &[
            Platform::ClaudeCode,
            Platform::Codex,
            Platform::Cursor,
            Platform::Windsurf,
            Platform::Zed,
            Platform::ContinueDev,
            Platform::OpenCode,
            Platform::Antigravity,
            Platform::GeminiCli,
            Platform::Aider,
            Platform::Copilot,
            Platform::FactoryDroid,
            Platform::Trae,
            Platform::Kiro,
            Platform::Qoder,
            Platform::OpenClaw,
            Platform::Hermes,
            Platform::Qwen,
        ]
    }

    /// Dispatch to the per-platform adapter.
    pub fn adapter(self) -> Box<dyn PlatformAdapter> {
        match self {
            Platform::ClaudeCode => Box::new(claude_code::ClaudeCode),
            Platform::Codex => Box::new(codex::Codex),
            Platform::Cursor => Box::new(cursor::Cursor),
            Platform::Windsurf => Box::new(windsurf::Windsurf),
            Platform::Zed => Box::new(zed::Zed),
            Platform::ContinueDev => Box::new(continue_dev::ContinueDev),
            Platform::OpenCode => Box::new(opencode::OpenCode),
            Platform::Antigravity => Box::new(antigravity::Antigravity),
            Platform::GeminiCli => Box::new(gemini_cli::GeminiCli),
            Platform::Aider => Box::new(aider::Aider),
            Platform::Copilot => Box::new(copilot::Copilot),
            Platform::FactoryDroid => Box::new(factory_droid::FactoryDroid),
            Platform::Trae => Box::new(trae::Trae),
            Platform::Kiro => Box::new(kiro::Kiro),
            Platform::Qoder => Box::new(qoder::Qoder),
            Platform::OpenClaw => Box::new(openclaw::OpenClaw),
            Platform::Hermes => Box::new(hermes::Hermes),
            Platform::Qwen => Box::new(qwen::Qwen),
        }
    }
}

/// Auto-detection helper. Mirrors design §21.4.2.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformDetector;

impl PlatformDetector {
    /// Walk every known platform and ask its adapter whether the platform's
    /// config dir / dotfile is present in the user's home. ClaudeCode and
    /// Qoder are always considered installed (per design §21.4.2 footnote).
    pub fn detect_installed(scope: InstallScope, project_root: &Path) -> Vec<Platform> {
        let ctx = AdapterContext::new(scope, project_root.to_path_buf());
        let mut found = Vec::new();
        for &p in Platform::all_known() {
            // ClaudeCode and Qoder are always tried; everyone else must
            // pass the per-adapter detect() probe.
            let always_on = matches!(p, Platform::ClaudeCode | Platform::Qoder);
            if always_on || p.adapter().detect(&ctx) {
                found.push(p);
            }
        }
        found
    }
}

/// MCP-config family. Drives serialization choice in
/// [`AdapterContext::mcp_format_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpFormat {
    /// Most platforms: `{"mcpServers": {"<name>": {...}}}`.
    JsonObject,
    /// Continue.dev: `{"mcpServers": [{"name": "<name>", ...}]}`.
    JsonArray,
    /// Codex: TOML `[mcp_servers.<name>]`.
    Toml,
}

/// Read-only view passed to every adapter. Lets adapters resolve paths
/// against the right home / project root and pick up dry-run / force flags
/// without parsing CLI args themselves.
#[derive(Debug, Clone)]
pub struct AdapterContext {
    /// Scope chosen by the user.
    pub scope: InstallScope,
    /// Resolved home dir (may equal project_root when scope=Project).
    pub home: PathBuf,
    /// Project root (CWD by default).
    pub project_root: PathBuf,
    /// If true, no files are written.
    pub dry_run: bool,
    /// If true, overwrite even if user has edited the marker block.
    pub force: bool,
}

impl AdapterContext {
    /// Build a context with the standard home dir.
    pub fn new(scope: InstallScope, project_root: PathBuf) -> Self {
        Self {
            scope,
            home: dirs::home_dir().unwrap_or_else(|| project_root.clone()),
            project_root,
            dry_run: false,
            force: false,
        }
    }

    /// Builder: enable dry-run.
    pub fn with_dry_run(mut self, dry: bool) -> Self {
        self.dry_run = dry;
        self
    }

    /// Builder: enable force-overwrite of edited marker blocks.
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Build the canonical AGENTS.md body mneme wants to inject.
    pub fn agents_md_body() -> String {
        // Kept short — full prompt-engineering lives in the supervisor.
        // This is the user-visible "what is mneme" stub.
        format!(
            "## Mneme (the AI superbrain)\n\
             \n\
             - Persistent per-project memory across sessions and compactions.\n\
             - 30+ MCP tools: see `mneme --help` or run `/mn-recall`, `/mn-blast`, `/mn-graphify`, `/mn-step`.\n\
             - Step Ledger guarantees Claude resumes the *correct* step after compaction.\n\
             - Drift detector enforces this project's rules from CLAUDE.md / .claude/rules.\n\
             - Local-only: zero network egress in the hot path.\n\
             - Vision app: `mneme view`\n\
             - Daemon: `mneme daemon status`\n\
             \n\
             v{version} — see https://github.com/anishtrivedi/mneme\n",
            version = env!("CARGO_PKG_VERSION")
        )
    }
}

/// Concrete behaviour every platform module exposes.
///
/// Default no-op implementations are provided for `write_hooks` and
/// `detect`, so the simpler platforms only need to override what's
/// distinctive about their config layout.
pub trait PlatformAdapter: Send + Sync {
    /// Which platform this is.
    fn platform(&self) -> Platform;

    /// Probe the filesystem for evidence the platform is installed.
    /// Default: false. Adapters override this to look at well-known dirs.
    fn detect(&self, _ctx: &AdapterContext) -> bool {
        false
    }

    /// Where the platform reads its prompt rules manifest (e.g. `AGENTS.md`,
    /// `CLAUDE.md`). Returned path may not exist yet.
    fn manifest_path(&self, ctx: &AdapterContext) -> PathBuf;

    /// Where the platform reads its MCP server registry.
    fn mcp_config_path(&self, ctx: &AdapterContext) -> PathBuf;

    /// JSON object / array / TOML.
    fn mcp_format(&self) -> McpFormat {
        McpFormat::JsonObject
    }

    /// Write the marker-wrapped mneme section into the manifest.
    /// Default implementation handles the read-modify-write dance via
    /// [`crate::markers::MarkerInjector`].
    fn write_manifest(&self, ctx: &AdapterContext) -> CliResult<PathBuf> {
        let path = self.manifest_path(ctx);
        let body = AdapterContext::agents_md_body();
        write_marker_manifest(&path, &body, ctx)?;
        Ok(path)
    }

    /// Write the MCP config entry. Default uses the format from
    /// [`Self::mcp_format`] and a backup-then-merge strategy.
    fn write_mcp_config(&self, ctx: &AdapterContext) -> CliResult<PathBuf> {
        let path = self.mcp_config_path(ctx);
        write_mcp_config_default(&path, self.mcp_format(), ctx)?;
        Ok(path)
    }

    /// Write hook config (where the platform supports hooks). Default: no-op.
    fn write_hooks(&self, _ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        Ok(None)
    }

    /// Reverse of [`Self::write_manifest`]. Strips the marker block.
    fn remove_manifest(&self, ctx: &AdapterContext) -> CliResult<()> {
        let path = self.manifest_path(ctx);
        if !path.exists() {
            return Ok(());
        }
        let existing = std::fs::read_to_string(&path).map_err(|e| CliError::io(&path, e))?;
        let cleaned = crate::markers::MarkerInjector::remove(&existing);
        if !ctx.dry_run {
            backup_then_write(&path, cleaned.as_bytes())?;
        }
        Ok(())
    }

    /// Reverse of [`Self::write_mcp_config`]. Removes mneme's entry.
    fn remove_mcp_config(&self, ctx: &AdapterContext) -> CliResult<()> {
        let path = self.mcp_config_path(ctx);
        if !path.exists() {
            return Ok(());
        }
        remove_mcp_entry(&path, self.mcp_format(), ctx)
    }
}

/// The canonical mneme MCP entry written into JSON-object configs.
/// Kept as `serde_json::Value` so individual adapters can mutate fields
/// before serializing (e.g. Cursor wants `transport: "stdio"`).
pub fn mneme_mcp_entry() -> serde_json::Value {
    serde_json::json!({
        "command": "mneme",
        "args": ["mcp", "stdio"],
        "env": {
            "MNEME_LOG": "info"
        },
        "transport": "stdio"
    })
}

/// Write `body` into the mneme marker block of the file at `path`,
/// creating intermediate directories and the file itself if needed.
pub fn write_marker_manifest(
    path: &Path,
    body: &str,
    ctx: &AdapterContext,
) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        if !ctx.dry_run {
            std::fs::create_dir_all(parent).map_err(|e| CliError::io(parent, e))?;
        }
    }
    let existing = if path.exists() {
        std::fs::read_to_string(path).map_err(|e| CliError::io(path, e))?
    } else {
        String::new()
    };
    let new_text = crate::markers::MarkerInjector::inject(&existing, body, path, ctx.force)?;
    if ctx.dry_run {
        tracing::info!(path = %path.display(), bytes = new_text.len(), "dry-run: would write manifest");
        return Ok(());
    }
    backup_then_write(path, new_text.as_bytes())?;
    Ok(())
}

/// Default merge strategy for MCP configs. Reads existing config (if any),
/// inserts/updates the `mneme` entry, writes back. Always backs up to
/// `<file>.bak` first.
pub fn write_mcp_config_default(
    path: &Path,
    format: McpFormat,
    ctx: &AdapterContext,
) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        if !ctx.dry_run {
            std::fs::create_dir_all(parent).map_err(|e| CliError::io(parent, e))?;
        }
    }

    let existing = if path.exists() {
        std::fs::read_to_string(path).map_err(|e| CliError::io(path, e))?
    } else {
        match format {
            McpFormat::JsonObject => "{}".to_string(),
            McpFormat::JsonArray => "{\"mcpServers\": []}".to_string(),
            McpFormat::Toml => "".to_string(),
        }
    };

    let merged = match format {
        McpFormat::JsonObject => merge_mcp_json_object(&existing)?,
        McpFormat::JsonArray => merge_mcp_json_array(&existing)?,
        McpFormat::Toml => merge_mcp_toml(&existing)?,
    };

    if ctx.dry_run {
        tracing::info!(path = %path.display(), bytes = merged.len(), "dry-run: would write MCP config");
        return Ok(());
    }
    backup_then_write(path, merged.as_bytes())?;
    Ok(())
}

/// Reverse of [`write_mcp_config_default`]. Strips mneme from the config
/// while leaving every other server intact.
pub fn remove_mcp_entry(
    path: &Path,
    format: McpFormat,
    ctx: &AdapterContext,
) -> CliResult<()> {
    let existing = std::fs::read_to_string(path).map_err(|e| CliError::io(path, e))?;
    let stripped = match format {
        McpFormat::JsonObject => strip_mcp_json_object(&existing)?,
        McpFormat::JsonArray => strip_mcp_json_array(&existing)?,
        McpFormat::Toml => strip_mcp_toml(&existing)?,
    };
    if !ctx.dry_run {
        backup_then_write(path, stripped.as_bytes())?;
    }
    Ok(())
}

fn merge_mcp_json_object(existing: &str) -> CliResult<String> {
    let mut value: serde_json::Value = if existing.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(existing)?
    };
    let obj = value
        .as_object_mut()
        .ok_or_else(|| CliError::Other("MCP config root is not a JSON object".into()))?;
    let servers = obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let servers_obj = servers
        .as_object_mut()
        .ok_or_else(|| CliError::Other("`mcpServers` is not a JSON object".into()))?;
    servers_obj.insert("mneme".into(), mneme_mcp_entry());
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

fn strip_mcp_json_object(existing: &str) -> CliResult<String> {
    let mut value: serde_json::Value = serde_json::from_str(existing)?;
    if let Some(servers) = value
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    {
        servers.remove("mneme");
    }
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

fn merge_mcp_json_array(existing: &str) -> CliResult<String> {
    let mut value: serde_json::Value = if existing.trim().is_empty() {
        serde_json::json!({"mcpServers": []})
    } else {
        serde_json::from_str(existing)?
    };
    let servers = value
        .as_object_mut()
        .ok_or_else(|| CliError::Other("MCP config root is not a JSON object".into()))?
        .entry("mcpServers".to_string())
        .or_insert_with(|| serde_json::json!([]));
    let arr = servers
        .as_array_mut()
        .ok_or_else(|| CliError::Other("`mcpServers` is not a JSON array".into()))?;
    arr.retain(|s| s.get("name").and_then(|n| n.as_str()) != Some("mneme"));
    let mut entry = mneme_mcp_entry();
    entry
        .as_object_mut()
        .unwrap()
        .insert("name".into(), serde_json::json!("mneme"));
    arr.push(entry);
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

fn strip_mcp_json_array(existing: &str) -> CliResult<String> {
    let mut value: serde_json::Value = serde_json::from_str(existing)?;
    if let Some(arr) = value
        .get_mut("mcpServers")
        .and_then(|v| v.as_array_mut())
    {
        arr.retain(|s| s.get("name").and_then(|n| n.as_str()) != Some("mneme"));
    }
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

fn merge_mcp_toml(existing: &str) -> CliResult<String> {
    // Use toml::Value so we don't have to reason about formatting nuances —
    // we re-emit a deterministic representation.
    let mut doc: toml::Value = if existing.trim().is_empty() {
        toml::Value::Table(toml::value::Table::new())
    } else {
        existing.parse()?
    };

    let root = doc
        .as_table_mut()
        .ok_or_else(|| CliError::Other("TOML root is not a table".into()))?;

    let servers = root
        .entry("mcp_servers".to_string())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let servers_table = servers
        .as_table_mut()
        .ok_or_else(|| CliError::Other("`mcp_servers` is not a TOML table".into()))?;

    let mut entry = toml::value::Table::new();
    entry.insert("command".into(), toml::Value::String("mneme".into()));
    entry.insert(
        "args".into(),
        toml::Value::Array(vec![
            toml::Value::String("mcp".into()),
            toml::Value::String("stdio".into()),
        ]),
    );
    let mut env = toml::value::Table::new();
    env.insert("MNEME_LOG".into(), toml::Value::String("info".into()));
    entry.insert("env".into(), toml::Value::Table(env));
    servers_table.insert("mneme".into(), toml::Value::Table(entry));

    Ok(toml::to_string_pretty(&doc)?)
}

fn strip_mcp_toml(existing: &str) -> CliResult<String> {
    let mut doc: toml::Value = existing.parse()?;
    if let Some(servers) = doc
        .as_table_mut()
        .and_then(|t| t.get_mut("mcp_servers"))
        .and_then(|v| v.as_table_mut())
    {
        servers.remove("mneme");
    }
    Ok(toml::to_string_pretty(&doc)?)
}

/// Atomic-ish backup-then-write. Always writes a `<path>.bak` snapshot of
/// the current file (if any) before clobbering. Failure to write the
/// backup aborts the whole operation — better to refuse than to lose data.
pub fn backup_then_write(path: &Path, new_contents: &[u8]) -> CliResult<()> {
    if path.exists() {
        let bak = path.with_extension(extension_with_bak(path));
        std::fs::copy(path, &bak).map_err(|e| CliError::BackupFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
    }
    std::fs::write(path, new_contents).map_err(|e| CliError::io(path, e))
}

fn extension_with_bak(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.bak"),
        _ => "bak".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn install_scope_parses() {
        assert_eq!("project".parse::<InstallScope>().unwrap(), InstallScope::Project);
        assert_eq!("USER".parse::<InstallScope>().unwrap(), InstallScope::User);
        assert!(matches!(
            "wat".parse::<InstallScope>(),
            Err(CliError::InvalidScope(_))
        ));
    }

    #[test]
    fn all_known_has_18_platforms() {
        assert_eq!(Platform::all_known().len(), 18);
    }

    #[test]
    fn from_id_round_trip() {
        for &p in Platform::all_known() {
            let parsed = Platform::from_id(p.id()).unwrap();
            assert_eq!(parsed, p);
        }
    }

    #[test]
    fn detect_always_includes_claude_and_qoder() {
        let dir = tempdir().unwrap();
        let found = PlatformDetector::detect_installed(InstallScope::User, dir.path());
        assert!(found.contains(&Platform::ClaudeCode));
        assert!(found.contains(&Platform::Qoder));
    }

    #[test]
    fn merge_mcp_json_object_inserts_mneme() {
        let starting = r#"{"mcpServers":{"other":{"command":"other"}}}"#;
        let merged = merge_mcp_json_object(starting).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert!(parsed["mcpServers"]["mneme"].is_object());
        assert!(parsed["mcpServers"]["other"].is_object());
    }

    #[test]
    fn merge_mcp_json_array_dedupes_mneme() {
        let starting = r#"{"mcpServers":[{"name":"mneme","command":"old"}]}"#;
        let merged = merge_mcp_json_array(starting).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let arr = parsed["mcpServers"].as_array().unwrap();
        let dt: Vec<&serde_json::Value> = arr
            .iter()
            .filter(|v| v["name"] == "mneme")
            .collect();
        assert_eq!(dt.len(), 1);
        assert_eq!(dt[0]["command"], "mneme");
    }

    #[test]
    fn merge_mcp_toml_inserts_section() {
        let starting = "";
        let merged = merge_mcp_toml(starting).unwrap();
        assert!(merged.contains("[mcp_servers.mneme]"));
        assert!(merged.contains("command = \"mneme\""));
    }
}
