//! `mneme-hook` — windowless dispatcher for Claude Code hook commands.
//!
//! ## Why this exists (BUG-NEW-H true fix, 2026-05-05)
//!
//! When Claude Code (a desktop app, no console) fires a hook entry from
//! `~/.claude/settings.json`, Windows allocates a **transient console
//! window** for the spawned process if that process is built with the
//! CONSOLE subsystem. `mneme.exe` is built that way (the user runs
//! `mneme build .` from a terminal — stdout / stderr / Ctrl-C all
//! depend on it). So every UserPromptSubmit + PreToolUse + PostToolUse
//! flashes a black cmd-window-shaped rectangle for ~50 ms before the
//! hook process actually attaches and detaches. Anish observed this as
//! "mneme.exe spawns visible terminal window randomly" — it's not
//! random; it's every single hook fire.
//!
//! Fix: ship a SECOND binary built with `windows_subsystem = "windows"`
//! and route hook invocations through that one. Console binaries keep
//! getting the console; hook spawns get nothing.
//!
//! ## Why a separate crate (not a second [[bin]])
//!
//! `#![windows_subsystem = "windows"]` is a CRATE-level attribute. If
//! we added a second `[[bin]]` to `mneme-cli` with this attribute, BOTH
//! binaries would be GUI-subsystem — breaking `mneme.exe`'s ability to
//! print to stdout from a terminal. So `mneme-hook` lives as its own
//! workspace member that depends on `mneme-cli`'s library surface and
//! delegates every subcommand to the same handler functions.
//!
//! ## Subcommands routed
//!
//! Mirrors the `args` field of every [`HookSpec`] entry in
//! `cli/src/platforms/claude_code.rs`. Anything Claude Code's
//! settings.json registers as a hook command MUST be in this list.
//! Adding a new HookSpec without adding a matching subcommand here is
//! BUG-NEW-Q recurring; tests in this binary's CI guard against it.
//!
//!   - userprompt-submit  (Layer 1: smart context injection)
//!   - inject             (Layer 0 legacy: shard-driven injection)
//!   - pre-tool           (Layer 0 legacy: PreToolUse → tool_cache.db)
//!   - pretool-edit-write (Layer 2: blast_radius gate skeleton)
//!   - pretool-grep-read  (Layer 3: redirect skeleton)
//!   - post-tool          (PostToolUse → tool_cache.db)
//!   - turn-end           (Stop / PreCompact / SubagentStop)
//!   - session-prime      (SessionStart)
//!   - session-end        (SessionEnd)
//!
//! Authors: Anish Trivedi & Kruti Trivedi. Apache-2.0.

// CRATE-LEVEL: only mneme-hook is GUI subsystem. mneme-cli stays
// console subsystem (the entire point of this whole exercise).
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use mneme_cli::commands;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "mneme-hook",
    about = "Windowless hook dispatcher for Claude Code. Same subcommands as `mneme.exe` for hook events; built without a console subsystem so hook spawns don't flash a terminal on Windows.",
    long_about = None,
    disable_help_subcommand = true,
)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Wave 1E Layer 1: smart context injection.
    #[command(name = "userprompt-submit")]
    UserpromptSubmit(commands::userprompt_submit::UserPromptSubmitArgs),
    /// Layer 0 (legacy): shard-driven inject. Runs ALONGSIDE
    /// userprompt-submit per Wave 1E plan.
    Inject(commands::inject::InjectArgs),
    /// Wave 1E Layer 2: blast_radius gate for Edit/Write/MultiEdit.
    #[command(name = "pretool-edit-write")]
    PretoolEditWrite(commands::pretool_edit_write::PretoolEditWriteArgs),
    /// Wave 1E Layer 3: Grep/Read/Glob redirect skeleton.
    #[command(name = "pretool-grep-read")]
    PretoolGrepRead(commands::pretool_grep_read::PretoolGrepReadArgs),
    /// Layer 0 (legacy): PreToolUse → tool_cache.db.
    #[command(name = "pre-tool")]
    PreTool(commands::pre_tool::PreToolArgs),
    /// PostToolUse → tool_cache.db.
    #[command(name = "post-tool")]
    PostTool(commands::post_tool::PostToolArgs),
    /// Stop / PreCompact / SubagentStop (between turns).
    #[command(name = "turn-end")]
    TurnEnd(commands::turn_end::TurnEndArgs),
    /// SessionStart.
    #[command(name = "session-prime")]
    SessionPrime(commands::session_prime::SessionPrimeArgs),
    /// SessionEnd.
    #[command(name = "session-end")]
    SessionEnd(commands::session_end::SessionEndArgs),
}

fn main() -> ExitCode {
    // Build a single-threaded runtime — every hook handler is async,
    // but each invocation is short-lived and single-shot, so we don't
    // need the full multi-thread scheduler. Smaller binary, faster
    // start-up.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            // GUI-subsystem binaries have no stderr. Fail-open: emit
            // an approve/empty JSON to stdout (which Claude Code's
            // hook spawner DOES capture even on windowless processes)
            // so the user's flow isn't blocked.
            eprintln_via_stdout(&format!("hook runtime init failed: {e}"));
            return ExitCode::SUCCESS;
        }
    };

    let cli = match <Cli as Parser>::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // clap error → emit fail-open JSON so the hook surface
            // never blocks the user. Fully suppress the clap stderr
            // (which would normally render the help banner).
            eprintln_via_stdout(&format!("hook arg parse failed: {e}"));
            return ExitCode::SUCCESS;
        }
    };

    // socket_override is None — hooks always use the daemon's default
    // socket. The CLI accepts `--socket` for testing; we deliberately
    // do not expose it on the hook binary.
    let socket_override = None;

    let result: mneme_cli::CliResult<()> = runtime.block_on(async move {
        match cli.cmd {
            Command::UserpromptSubmit(a) => commands::userprompt_submit::run(a).await,
            Command::Inject(a) => commands::inject::run(a, socket_override).await,
            Command::PretoolEditWrite(a) => commands::pretool_edit_write::run(a).await,
            Command::PretoolGrepRead(a) => commands::pretool_grep_read::run(a).await,
            Command::PreTool(a) => commands::pre_tool::run(a, socket_override).await,
            Command::PostTool(a) => commands::post_tool::run(a, socket_override).await,
            Command::TurnEnd(a) => commands::turn_end::run(a, socket_override).await,
            Command::SessionPrime(a) => commands::session_prime::run(a, socket_override).await,
            Command::SessionEnd(a) => commands::session_end::run(a, socket_override).await,
        }
    });

    if let Err(e) = result {
        // Fail-open: handler errored. Emit a minimal JSON so the
        // hook surface still produces a valid Claude Code reply.
        // Exact shape varies per hook event but every event accepts
        // an empty `hook_specific` object as a no-op approval.
        eprintln_via_stdout(&format!("hook handler failed: {e}"));
    }

    // Always SUCCESS — we never block Claude Code on a hook failure.
    // If the handler emitted a JSON reply earlier, Claude Code reads
    // it; if it errored, we already wrote a fallback to stdout.
    ExitCode::SUCCESS
}

/// On Windows GUI-subsystem binaries there is no stderr handle by
/// default (it's `/dev/null`). To still surface a diagnostic, route
/// it through stdout wrapped in a JSON envelope Claude Code can parse
/// without crashing — `{"hook_specific":{},"_mneme_diag":"..."}`.
/// The `_mneme_diag` field is unknown to Claude Code's schema and is
/// ignored, but it lets a downstream debugger fish out the message.
fn eprintln_via_stdout(msg: &str) {
    // Best-effort. If stdout is also closed (extremely rare under
    // hook spawn), we silently drop — failing harder is worse than
    // missing one diagnostic.
    //
    // SEC-005 / REL-008 fix (2026-05-05): the previous version only
    // escaped `\\` and `"`, producing invalid JSON when `msg`
    // contained a literal newline / tab / control char. Tokio
    // runtime init errors and clap parse errors routinely include
    // multiline output, so the very paths designed to be fail-open
    // shipped invalid JSON to Claude Code, defeating the documented
    // contract. serde_json::to_string handles every escape the JSON
    // grammar requires (\n \r \t \" \\ \uXXXX for control chars).
    let envelope = serde_json::json!({
        "hook_specific": {},
        "_mneme_diag": msg,
    });
    let serialized = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        // Fallback for the impossible case where serde itself
        // chokes — emit a static-string envelope rather than no
        // envelope at all.
        Err(_) => r#"{"hook_specific":{},"_mneme_diag":"<serde-failure>"}"#.to_string(),
    };
    let _ = std::io::Write::write_all(&mut std::io::stdout().lock(), serialized.as_bytes());
}
