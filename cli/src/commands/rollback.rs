//! `mneme rollback` — reverse a previous install using its receipt.
//!
//! Closes report-002.md §F-014: v0.3.0 left users with no in-agent
//! recovery path when an install broke Claude Code. v0.3.1 ships a
//! first-class `mneme rollback` that reads the most recent install
//! receipt from `~/.mneme/install-receipts/` and reverses every file
//! write, PATH addition, Defender exclusion, and MCP registration it
//! recorded — with sha256 drift detection so hand-edits aren't
//! clobbered.

use clap::Args;

use crate::error::{CliError, CliResult};
use crate::receipts::{list_receipts, Receipt};

/// CLI args for `mneme rollback`.
#[derive(Debug, Args)]
pub struct RollbackArgs {
    /// Show every receipt on disk (newest first), do not roll back.
    #[arg(long)]
    pub list: bool,

    /// Roll back a specific receipt by id (8-hex short id). When absent,
    /// the most recent receipt is used.
    pub id: Option<String>,

    /// Print what would be reversed, do nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: RollbackArgs) -> CliResult<()> {
    let receipts = list_receipts()?;
    if receipts.is_empty() {
        println!("no install receipts found at ~/.mneme/install-receipts/");
        println!("(nothing to roll back — either no install has run yet, or you ran");
        println!(" an older mneme that didn't write receipts)");
        return Ok(());
    }

    if args.list {
        println!("{} install receipt(s), newest first:", receipts.len());
        for path in &receipts {
            match Receipt::load(path) {
                Ok(r) => println!(
                    "  {:>10}  v{:<8}  {}  ({} action(s))",
                    r.id,
                    r.mneme_version,
                    r.installed_at,
                    r.actions.len()
                ),
                Err(e) => println!("  {}  <unreadable: {}>", path.display(), e),
            }
        }
        return Ok(());
    }

    // Pick the receipt to roll back.
    let target = match args.id.as_deref() {
        None => receipts[0].clone(),
        Some(id) => {
            // Match by substring of filename — the id is part of it.
            receipts
                .iter()
                .find(|p| {
                    p.file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.contains(id))
                })
                .cloned()
                .ok_or_else(|| CliError::Other(format!("no receipt matching id '{id}'")))?
        }
    };

    let receipt = Receipt::load(&target)?;
    println!(
        "rolling back receipt {} (v{}, {}, {} action(s))",
        receipt.id,
        receipt.mneme_version,
        receipt.installed_at,
        receipt.actions.len()
    );

    if args.dry_run {
        println!();
        println!("(dry-run — no changes will be made)");
        for (i, action) in receipt.actions.iter().enumerate() {
            println!("  {}. {}", i + 1, describe_action(action));
        }
        return Ok(());
    }

    let summary = receipt.rollback()?;

    println!();
    if !summary.reversed.is_empty() {
        println!("reversed ({}):", summary.reversed.len());
        for line in &summary.reversed {
            println!("  - {line}");
        }
    }
    if !summary.skipped.is_empty() {
        println!();
        println!("skipped ({}):", summary.skipped.len());
        for line in &summary.skipped {
            println!("  - {line}");
        }
    }
    if !summary.manual.is_empty() {
        println!();
        println!("needs manual cleanup ({}):", summary.manual.len());
        for line in &summary.manual {
            println!("  - {line}");
        }
    }
    println!();
    println!("rollback complete.");
    Ok(())
}

fn describe_action(a: &crate::receipts::ReceiptAction) -> String {
    use crate::receipts::ReceiptAction;
    match a {
        ReceiptAction::FileModified {
            path, backup_path, ..
        } => format!("restore {} from {}", path.display(), backup_path.display()),
        ReceiptAction::FileCreated { path, .. } => {
            format!("delete created file {}", path.display())
        }
        ReceiptAction::PathAdded { entry } => format!("remove PATH entry {entry}"),
        ReceiptAction::DefenderExcluded { path } => format!(
            "print manual Defender-exclusion-removal command for {}",
            path.display()
        ),
        ReceiptAction::McpRegistered {
            platform,
            host_file,
        } => format!(
            "remove mneme MCP entry for {} from {}",
            platform,
            host_file.display()
        ),
        ReceiptAction::BinaryArchiveExtracted { target, files } => format!(
            "(manual) archive at {} contained {} files",
            target.display(),
            files.len()
        ),
        ReceiptAction::DaemonStarted { binary } => {
            format!("(manual) stop daemon at {}", binary.display())
        }
        ReceiptAction::BinaryPlaced { binary } => {
            format!("(manual) note binary placed at {}", binary.display())
        }
        ReceiptAction::ResolvedExePath { exe_path } => format!(
            "(info) MCP entries used absolute exe path {}",
            exe_path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipts::ReceiptAction;
    use std::path::PathBuf;

    #[test]
    fn describe_action_path_added() {
        let a = ReceiptAction::PathAdded {
            entry: "/usr/local/bin/mneme".to_string(),
        };
        assert!(describe_action(&a).contains("/usr/local/bin/mneme"));
    }

    #[test]
    fn describe_action_file_created() {
        let a = ReceiptAction::FileCreated {
            path: PathBuf::from("/tmp/x"),
            sha256_after: "deadbeef".into(),
        };
        let s = describe_action(&a);
        assert!(s.contains("delete"));
        assert!(s.contains("/tmp/x") || s.contains("\\tmp\\x"));
    }

    #[test]
    fn describe_action_mcp_registered() {
        let a = ReceiptAction::McpRegistered {
            platform: "claude-code".into(),
            host_file: PathBuf::from("/home/user/.claude.json"),
        };
        let s = describe_action(&a);
        assert!(s.contains("claude-code"));
    }
}
