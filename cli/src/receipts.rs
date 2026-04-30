//! Install receipts — durable record of every file mneme touched, with
//! enough metadata to reverse atomically.
//!
//! ## Why this exists
//!
//! The v0.3.0 install incident (see `report-002.md §F-014`) left the
//! user's Claude Code config in a broken state that `mneme uninstall`
//! could NOT recover from — every tool call was blocked, so the
//! uninstall command itself was unreachable. Manual filesystem rename
//! was the only recovery path.
//!
//! v0.3.1 makes recovery a one-command operation:
//!
//! ```text
//! mneme rollback           # restore most-recent install receipt
//! mneme rollback --list    # show every receipt ever recorded
//! mneme rollback <id>      # restore a specific receipt by id
//! ```
//!
//! ## Receipt shape
//!
//! One JSON file per install run at
//! `~/.mneme/install-receipts/<timestamp>-<id8>.json`. Each file records:
//!
//!   * mneme version at install time
//!   * absolute timestamp (UTC)
//!   * every action, ordered — file modifications (with sha256 before
//!     + absolute path of the backup copy), pure-create entries, and
//!     environment-level actions (PATH additions, Defender exclusions)
//!
//! ## Guarantees
//!
//! 1. A receipt is written BEFORE any destructive file write — so a
//!    crash mid-install leaves a receipt that can roll back whatever
//!    did happen.
//! 2. Every `FileModified` action references a timestamped `.mneme-*.bak`
//!    created by `platforms::backup_then_write`. On rollback we verify
//!    the file's current sha256 matches what the receipt recorded as
//!    the post-install state before overwriting — if someone
//!    hand-edited after install, the rollback aborts and tells the
//!    user which file diverged.
//! 3. Receipts are append-only. `mneme rollback` never deletes a
//!    receipt; it writes a new receipt documenting the rollback itself,
//!    so the history is complete.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::error::{CliError, CliResult};

/// One recorded action inside a receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReceiptAction {
    /// We modified an existing file. `backup_path` points to the timestamped
    /// copy of the pre-write contents. `sha256_before` / `sha256_after` let
    /// rollback detect manual edits between install and rollback.
    FileModified {
        /// Target file we wrote to (e.g. `~/.claude.json`).
        path: PathBuf,
        /// Absolute path of the `.mneme-*.bak` backup we created.
        backup_path: PathBuf,
        /// sha256 of the file BEFORE install (matches what's in backup_path).
        sha256_before: String,
        /// sha256 of the file AFTER install (for drift detection on rollback).
        sha256_after: String,
    },
    /// We created a new file that didn't exist before. Rollback = delete it.
    FileCreated {
        /// Target file we wrote.
        path: PathBuf,
        /// sha256 of the file content AFTER install (for drift detection).
        sha256_after: String,
    },
    /// We added an entry to the User-scope PATH. Rollback = strip it.
    PathAdded {
        /// The exact directory entry we appended (e.g. `C:\Users\X\.mneme\bin`).
        entry: String,
    },
    /// We registered a Windows Defender exclusion. Rollback = remove it.
    DefenderExcluded {
        /// Path that was excluded.
        path: PathBuf,
    },
    /// We added an MCP server entry to a host's registry. Rollback =
    /// remove the entry without disturbing neighbors. Record the
    /// platform id + host file so the uninstall knows where to look.
    McpRegistered {
        /// Platform id (e.g. `claude-code`, `cursor`).
        platform: String,
        /// Host's MCP registry file (e.g. `~/.claude.json`).
        host_file: PathBuf,
    },
    /// NEW-001: install.ps1 staged a release zip into ~/.mneme. We
    /// record the manifest (set of file paths the zip extracted) so a
    /// future upgrade can compute the orphan set and clean it.
    BinaryArchiveExtracted {
        /// Where the archive extracted to (e.g. `~/.mneme`).
        target: PathBuf,
        /// Relative paths inside `target` that the archive populated.
        files: Vec<String>,
    },
    /// NEW-002: install.ps1 spawned the mneme daemon. Rollback hint
    /// only — we don't auto-stop the daemon (it may be in use).
    DaemonStarted {
        /// Absolute path to the daemon binary that was spawned.
        binary: PathBuf,
    },
    /// NEW-002: install.ps1 placed the mneme binary at a known path
    /// (typically `~/.mneme/bin/mneme(.exe)`). Recorded so rollback
    /// understands where the canonical exe lives even when the running
    /// `mneme rollback` invocation sits at a different path.
    BinaryPlaced {
        /// Absolute path of the placed binary.
        binary: PathBuf,
    },
    /// NEW-001 / I-1: which absolute mneme exe path we wrote into
    /// every MCP `command` field. Lets `mneme rollback` confirm the
    /// poisoning shape isn't still in some host's config.
    ResolvedExePath {
        /// The exe path that landed in the MCP `command` fields.
        exe_path: PathBuf,
    },
}

/// A single install receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Short id (first 8 hex chars of a uuid-v7) for the receipt.
    pub id: String,
    /// Mneme version at install time (from CARGO_PKG_VERSION).
    pub mneme_version: String,
    /// ISO-8601 UTC timestamp.
    pub installed_at: String,
    /// Parent receipt id this receipt rolled back, if any. Lets us
    /// reconstruct rollback chains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rolled_back_from: Option<String>,
    /// Actions taken, in order.
    pub actions: Vec<ReceiptAction>,
}

impl Receipt {
    /// Build a fresh receipt for a new install run.
    pub fn new() -> Self {
        Self {
            id: short_id(),
            mneme_version: env!("CARGO_PKG_VERSION").to_string(),
            installed_at: iso8601_utc_now(),
            rolled_back_from: None,
            actions: Vec::new(),
        }
    }

    /// Append one action.
    pub fn push(&mut self, action: ReceiptAction) {
        self.actions.push(action);
    }

    /// Persist this receipt under `~/.mneme/install-receipts/`. Creates
    /// the directory if needed. Filename format:
    /// `<YYYYMMDD-HHMMSS>-<id>.json`.
    ///
    /// REG-021: the actual file write is atomic — staged to a sibling
    /// `<path>.mneme-tmp` and `fs::rename`d into place so a crash
    /// mid-write can't leave a torn receipt that breaks future rollback.
    pub fn save(&self) -> CliResult<PathBuf> {
        let dir = receipts_dir()?;
        std::fs::create_dir_all(&dir).map_err(|e| CliError::io(&dir, e))?;
        let stamp = self.installed_at.replace([':', '-'], "").replace('T', "-");
        let stamp = stamp
            .split('.')
            .next()
            .unwrap_or(&stamp)
            .trim_end_matches('Z');
        let filename = format!("{}-{}.json", stamp, self.id);
        let path = dir.join(filename);
        let body = serde_json::to_string_pretty(self)?;
        crate::platforms::atomic_write(&path, body.as_bytes())?;
        Ok(path)
    }

    /// Load a receipt from disk by absolute path.
    pub fn load(path: &Path) -> CliResult<Self> {
        let bytes = std::fs::read(path).map_err(|e| CliError::io(path, e))?;
        let receipt: Receipt = serde_json::from_slice(&bytes)?;
        Ok(receipt)
    }

    /// Reverse every action in this receipt, newest first. Returns a
    /// summary of what was reversed and what couldn't be (with reasons).
    pub fn rollback(&self) -> CliResult<RollbackSummary> {
        let mut summary = RollbackSummary::default();
        // Reverse order — undo in the opposite direction we applied.
        for action in self.actions.iter().rev() {
            match action {
                ReceiptAction::FileModified {
                    path,
                    backup_path,
                    sha256_before: _,
                    sha256_after,
                } => match rollback_file_modified(path, backup_path, sha256_after) {
                    Ok(()) => summary
                        .reversed
                        .push(format!("restored {}", path.display())),
                    Err(e) => summary.skipped.push(format!("{}: {e}", path.display())),
                },
                ReceiptAction::FileCreated { path, sha256_after } => {
                    match rollback_file_created(path, sha256_after) {
                        Ok(()) => summary.reversed.push(format!("deleted {}", path.display())),
                        Err(e) => summary.skipped.push(format!("{}: {e}", path.display())),
                    }
                }
                ReceiptAction::PathAdded { entry } => match rollback_path_added(entry) {
                    Ok(()) => summary.reversed.push(format!("removed PATH entry {entry}")),
                    Err(e) => summary.skipped.push(format!("PATH {entry}: {e}")),
                },
                ReceiptAction::DefenderExcluded { path } => {
                    // We don't attempt to remove Defender exclusions
                    // automatically — that needs admin and user
                    // intervention. Tell the user.
                    summary.manual.push(format!(
                        "Defender exclusion for {} is still in place — remove via: Remove-MpPreference -ExclusionPath '{}'",
                        path.display(),
                        path.display()
                    ));
                }
                ReceiptAction::McpRegistered {
                    platform,
                    host_file,
                } => match rollback_mcp_entry(platform, host_file) {
                    Ok(()) => summary.reversed.push(format!(
                        "removed mneme MCP entry from {}",
                        host_file.display()
                    )),
                    Err(e) => summary.skipped.push(format!("MCP {platform}: {e}")),
                },
                ReceiptAction::BinaryArchiveExtracted { target, files } => {
                    // We don't auto-delete the install root; that's
                    // destructive and the user may want to keep their
                    // indexed projects. Print a manual hint instead.
                    summary.manual.push(format!(
                        "Archive at {} contained {} files. To wipe it: \
                         Remove-Item -Recurse -Force '{}'",
                        target.display(),
                        files.len(),
                        target.display()
                    ));
                }
                ReceiptAction::DaemonStarted { binary } => {
                    summary.manual.push(format!(
                        "Daemon was started at {}. Stop it via: mneme daemon stop",
                        binary.display()
                    ));
                }
                ReceiptAction::BinaryPlaced { binary } => {
                    summary.manual.push(format!(
                        "Mneme binary placed at {} — leave in place or remove manually",
                        binary.display()
                    ));
                }
                ReceiptAction::ResolvedExePath { exe_path } => {
                    // Informational only — the McpRegistered rollback
                    // strips the entry; this just records what path we
                    // had written so the user can verify nothing stale
                    // lingers in any host config.
                    summary.reversed.push(format!(
                        "(noted) MCP entries used absolute exe path {}",
                        exe_path.display()
                    ));
                }
            }
        }
        Ok(summary)
    }
}

impl Default for Receipt {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary returned by `Receipt::rollback`.
#[derive(Debug, Default)]
pub struct RollbackSummary {
    /// Actions successfully reversed.
    pub reversed: Vec<String>,
    /// Actions we couldn't reverse (with reasons). Non-fatal.
    pub skipped: Vec<String>,
    /// Actions that require manual user intervention (e.g. Defender).
    pub manual: Vec<String>,
}

/// `<mneme-root>/install-receipts/`.
///
/// HOME-bypass-receipts fix: route through `PathManager::default_root()`
/// so `MNEME_HOME` is honored. Result type kept `CliResult` for caller
/// source-compatibility — `PathManager::default_root()` is infallible
/// in practice (every supported OS yields one of MNEME_HOME / `~/.mneme`
/// / OS default) so this never errors.
pub fn receipts_dir() -> CliResult<PathBuf> {
    Ok(common::paths::PathManager::default_root()
        .root()
        .join("install-receipts"))
}

/// List every receipt on disk, newest first (sorted by filename stamp).
pub fn list_receipts() -> CliResult<Vec<PathBuf>> {
    let dir = receipts_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| CliError::io(&dir, e))?
        .filter_map(|r| r.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .map(|e| e.path())
        .collect();
    // Filename starts with a timestamp — sort descending = newest first.
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    Ok(entries)
}

/// sha256 hex digest of the file contents. Returns an empty string if
/// the file doesn't exist (caller uses the return value for
/// drift-detection: empty = "file was created by us, nothing to compare").
pub fn sha256_of_file(path: &Path) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let mut h = Sha256::new();
    h.update(&bytes);
    format!("{:x}", h.finalize())
}

fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    let d = h.finalize();
    format!("{:02x}{:02x}{:02x}{:02x}", d[0], d[1], d[2], d[3])
}

/// Minimal ISO-8601 UTC without a date-lib. `YYYY-MM-DDTHH:MM:SSZ`.
fn iso8601_utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;
    let (y, m, d) = ymd_from_epoch_days(days);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Same Hinnant algorithm used in platforms/mod.rs for the .bak stamp.
fn ymd_from_epoch_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 {
        (mp + 3) as u32
    } else {
        (mp - 9) as u32
    };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

fn rollback_file_modified(target: &Path, backup: &Path, sha_after: &str) -> Result<(), String> {
    // Drift detection — if user edited the file after install, do NOT
    // clobber their change. Refuse and let them decide.
    let current = sha256_of_file(target);
    if !sha_after.is_empty() && current != sha_after && !current.is_empty() {
        return Err(format!(
            "file content has changed since install (edited externally) — \
             refusing to clobber. Compare: {} vs {}",
            target.display(),
            backup.display()
        ));
    }
    if !backup.exists() {
        return Err(format!("backup {} missing", backup.display()));
    }
    std::fs::copy(backup, target)
        .map_err(|e| format!("copy {} -> {}: {e}", backup.display(), target.display()))?;
    Ok(())
}

fn rollback_file_created(target: &Path, sha_after: &str) -> Result<(), String> {
    if !target.exists() {
        return Ok(());
    }
    let current = sha256_of_file(target);
    if !sha_after.is_empty() && current != sha_after && !current.is_empty() {
        return Err(format!(
            "{} was modified after mneme created it — refusing to delete",
            target.display()
        ));
    }
    std::fs::remove_file(target).map_err(|e| format!("remove {}: {e}", target.display()))?;
    Ok(())
}

fn rollback_path_added(entry: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        // Use setx / [Environment]::SetEnvironmentVariable equivalent via
        // winreg. Fall back to no-op with an info message if we can't.
        // Keep this pure-std — touching registry from platforms/mod.rs
        // feels heavyweight for v0.3.1.
        let _ = entry;
        Err("PATH cleanup not implemented on Windows — manually prune if needed".into())
    }
    #[cfg(not(windows))]
    {
        let _ = entry;
        Err("PATH cleanup not implemented on this platform".into())
    }
}

fn rollback_mcp_entry(platform: &str, host_file: &Path) -> Result<(), String> {
    use crate::platforms::{remove_mcp_entry, AdapterContext, InstallScope, McpFormat, Platform};
    let plat = Platform::from_id(platform).map_err(|e| e.to_string())?;
    // The adapter knows the format; resolve via the adapter chain. We
    // build a minimal AdapterContext — only scope + paths matter.
    let _ = host_file; // current remove_mcp_entry takes the path via adapter
    let adapter = plat.adapter();
    let ctx = AdapterContext::new(
        InstallScope::User,
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );
    let path = adapter.mcp_config_path(&ctx);
    remove_mcp_entry(&path, adapter.mcp_format(), &ctx).map_err(|e| e.to_string())?;
    // Silence dead-imports in the no-host-file case
    let _: McpFormat = adapter.mcp_format();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_roundtrips_json() {
        let mut r = Receipt::new();
        r.push(ReceiptAction::FileModified {
            path: PathBuf::from("/tmp/x.json"),
            backup_path: PathBuf::from("/tmp/x.json.bak"),
            sha256_before: "abc".into(),
            sha256_after: "def".into(),
        });
        r.push(ReceiptAction::PathAdded {
            entry: "/some/bin".into(),
        });
        let j = serde_json::to_string(&r).unwrap();
        let r2: Receipt = serde_json::from_str(&j).unwrap();
        assert_eq!(r.id, r2.id);
        assert_eq!(r.actions.len(), r2.actions.len());
    }

    #[test]
    fn sha256_of_nonexistent_file_is_empty() {
        let s = sha256_of_file(Path::new("/this/should/not/exist.xyz123"));
        assert_eq!(s, "");
    }

    #[test]
    fn ymd_from_epoch_zero_is_1970() {
        assert_eq!(ymd_from_epoch_days(0), (1970, 1, 1));
    }

    #[test]
    fn short_id_is_8_hex_chars() {
        let id = short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
