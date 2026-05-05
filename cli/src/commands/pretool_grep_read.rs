//! `mneme pretool-grep-read` — Wave 1E Layer 3 hook entry.
//!
//! Claude Code's settings.json registers this hook to fire on every
//! PreToolUse for Grep / Read / Glob. Design intent (per
//! `mcp/src/hooks/pretool-grep-read.ts`) is the redirect mode — when
//! the AI calls Grep("foo"), check whether mneme_recall("foo") was
//! already run; if not, suggest it (v0.4.0 skeleton) or actually
//! redirect (v0.4.x once recall is trustworthy via the symbol
//! resolver).
//!
//! ## BUG-NEW-Q fix (2026-05-05)
//!
//! HOOK_SPECS in `cli/src/platforms/claude_code.rs` registered this
//! command in Wave 1E but the Rust CLI subcommand was never wired up.
//! This file ships the missing subcommand. v0.4.0 is the
//! always-approve skeleton documented in the original Wave 1E
//! comment; v0.4.x flips on the actual redirect once Item #122
//! (Redirect mode) lands AND the symbol resolver makes recall good
//! enough to substitute for grep.
//!
//! Hook output protocol (Claude Code spec for PreToolUse):
//!   `{ "hook_specific": { "decision": "approve" } }`
//!   on stdout. Exit 0 always — fail-open.

use clap::Args;
use serde_json::json;
use std::io::{self, Read};

use crate::error::CliResult;

/// CLI args for `mneme pretool-grep-read`. All optional — payload
/// comes from stdin per Claude Code's hook contract.
#[derive(Debug, Args, Default)]
pub struct PretoolGrepReadArgs {}

/// Entry point — wired into `cli/src/main.rs`.
///
/// v0.4.0 skeleton: always-approve. Drain stdin so Claude Code's
/// payload write doesn't broken-pipe; ignore the content for now.
pub async fn run(_args: PretoolGrepReadArgs) -> CliResult<()> {
    let _ = drain_stdin();
    let out = json!({
        "hook_specific": { "decision": "approve" },
    });
    println!("{}", out);
    Ok(())
}

fn drain_stdin() -> io::Result<()> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(())
}
