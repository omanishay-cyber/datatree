//! `mneme pretool-edit-write` — Wave 1E Layer 2 hook entry.
//!
//! Claude Code's settings.json registers this hook to fire on every
//! PreToolUse for Edit / Write / MultiEdit. Design intent (per
//! `mcp/src/hooks/pretool-edit-write.ts`) is to enforce a
//! blast_radius freshness window — block the edit until the AI has
//! seen the impact context.
//!
//! ## BUG-NEW-Q fix (2026-05-05)
//!
//! HOOK_SPECS in `cli/src/platforms/claude_code.rs` registered this
//! command in Wave 1E but the Rust CLI subcommand was never wired up.
//! Every PreToolUse for Edit/Write fired `mneme.exe pretool-edit-write`
//! → clap-errored → silent failure of the gate. This file ships the
//! missing subcommand. For v0.4.0 it's a SKELETON: always-approve.
//! Real recency check + blast_radius auto-run land in v0.4.1 once the
//! symbol resolver exists and recall is trustworthy enough to gate on.
//!
//! Hook output protocol (Claude Code spec for PreToolUse):
//!   `{ "hook_specific": { "decision": "approve" | "block", "reason"?: "..." } }`
//!   on stdout. Exit 0 always — fail-open.

use clap::Args;
use serde_json::json;
use std::io::{self, Read};

use crate::error::CliResult;

/// CLI args for `mneme pretool-edit-write`. All optional — payload
/// comes from stdin per Claude Code's hook contract.
#[derive(Debug, Args, Default)]
pub struct PretoolEditWriteArgs {}

/// Entry point — wired into `cli/src/main.rs`.
///
/// v0.4.0 skeleton: always-approve. We DO drain stdin so Claude Code
/// doesn't hit a broken-pipe writing the payload, but we don't act on
/// it yet. The TS sibling (`mcp/src/hooks/pretool-edit-write.ts`)
/// holds the real logic — Item #120 already trimmed it (small-file
/// bypass + 1500-char cap on injected reason). v0.4.1 wires the same
/// behaviour into Rust, plumbed through the IPC client to the
/// supervisor's blast_radius handler.
pub async fn run(_args: PretoolEditWriteArgs) -> CliResult<()> {
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
