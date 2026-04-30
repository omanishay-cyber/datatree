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

    /// Drop a small TOML command that surfaces mneme under `/mneme`
    /// in Gemini CLI.
    ///
    /// **M1 (audit DEEP-AUDIT-2026-04-29.md §M1):** Honors
    /// `ctx.enable_hooks=false` exactly the way `claude_code.rs:211`
    /// does. Without this gate, `mneme install --no-hooks --platform
    /// gemini-cli` would still write `~/.gemini/commands/mneme.toml`,
    /// breaking the K1 contract documented at
    /// `cli/src/platforms/mod.rs:291` ("false → write_hooks is a
    /// no-op").
    fn write_hooks(&self, ctx: &AdapterContext) -> CliResult<Option<PathBuf>> {
        if !ctx.enable_hooks {
            return Ok(None);
        }
        let cmd_dir = ctx.home.join(".gemini").join("commands");
        if !ctx.dry_run {
            std::fs::create_dir_all(&cmd_dir)
                .map_err(|e| crate::error::CliError::io(&cmd_dir, e))?;
        }
        let cmd_path = cmd_dir.join("mneme.toml");
        // I-1: write the absolute path so Gemini CLI doesn't depend on
        // `mneme` being on PATH. Quote both literal strings so the
        // emitted TOML is well-formed (the prior version had a quoting
        // bug that emitted `name = mneme"` — invalid TOML).
        let exe = ctx.exe_path.to_string_lossy();
        let body = format!(
            "name = \"mneme\"\n\
             description = \"Run a mneme command (status / recall / blast / step ...)\"\n\
             command = \"{exe}\"\n\
             args = [\"$@\"]\n"
        );
        if !ctx.dry_run {
            std::fs::write(&cmd_path, body.as_bytes())
                .map_err(|e| crate::error::CliError::io(&cmd_path, e))?;
        }
        Ok(Some(cmd_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// M1 (audit DEEP-AUDIT-2026-04-29.md §M1): non-Claude adapters must
    /// honor `ctx.enable_hooks=false` exactly the way `claude_code.rs:211`
    /// does — no hook artefact may be written when the user opts out via
    /// `mneme install --no-hooks` / `--skip-hooks`. Mirrors
    /// `platforms::claude_code::tests::write_hooks_no_op_when_not_enabled`.
    #[test]
    fn write_hooks_no_op_when_not_enabled() {
        let dir = tempdir().unwrap();
        let mut ctx = AdapterContext::new(InstallScope::User, dir.path().to_path_buf());
        ctx.home = dir.path().to_path_buf();
        assert!(!ctx.enable_hooks, "default must be opt-out");

        let result = GeminiCli.write_hooks(&ctx).unwrap();
        assert!(
            result.is_none(),
            "gemini-cli with enable_hooks=false must return Ok(None)"
        );
        let cmd_path = dir.path().join(".gemini").join("commands").join("mneme.toml");
        assert!(
            !cmd_path.exists(),
            "gemini-cli with enable_hooks=false must NOT write {}",
            cmd_path.display()
        );
    }

    #[test]
    fn write_hooks_writes_file_when_enabled() {
        let dir = tempdir().unwrap();
        let mut ctx = AdapterContext::new(InstallScope::User, dir.path().to_path_buf())
            .with_enable_hooks(true);
        ctx.home = dir.path().to_path_buf();

        let result = GeminiCli.write_hooks(&ctx).unwrap();
        let cmd_path = dir.path().join(".gemini").join("commands").join("mneme.toml");
        assert_eq!(
            result.as_deref(),
            Some(cmd_path.as_path()),
            "gemini-cli with enable_hooks=true must return the cmd path"
        );
        assert!(
            cmd_path.exists(),
            "gemini-cli with enable_hooks=true must write {}",
            cmd_path.display()
        );
    }
}

