//! BUG-NEW-Q regression guard (L9 / M-12, 2026-05-05 audit).
//!
//! Wave 1E added three new hook entries to `HOOK_SPECS`
//! (`userprompt-submit`, `pretool-edit-write`, `pretool-grep-read`) but
//! forgot to mirror them in the `Cli` enum in `cli/src/main.rs`. The
//! result was that Claude Code spawned `mneme-hook userprompt-submit ...`
//! on every prompt, clap rejected the unknown subcommand with exit code
//! 64, and every prompt surfaced as a hook error.
//!
//! This test walks `HOOK_SPECS` and runs `mneme <args>... --help` for
//! every hook command. If clap doesn't recognise the subcommand it
//! exits with a parse error, the test fails, and CI catches the
//! regression before it ships.
//!
//! The `--help` short-circuits before any subcommand handler runs, so
//! the test stays hermetic — no daemon, no IPC socket, no shard reads.
//!
//! Pairs with the runtime guard at `cli/src/commands/userprompt_submit.rs`
//! header comment that points future contributors here.

use mneme_cli::platforms::claude_code::HOOK_SPECS;
use std::process::Command;

#[test]
fn every_hook_spec_has_a_clap_subcommand() {
    // Cargo sets CARGO_BIN_EXE_<name> for integration tests so we can
    // spawn the built binary without needing assert_cmd.
    let bin = env!("CARGO_BIN_EXE_mneme");

    for spec in HOOK_SPECS {
        // Guard: HOOK_SPECS entries must always have at least one arg
        // (the subcommand name). Empty args means a malformed entry —
        // fail loudly here so the cause is obvious in CI, not somewhere
        // deep in install.rs.
        let first = spec.args.first().unwrap_or_else(|| {
            panic!(
                "HOOK_SPECS entry for event {:?} has empty args[]; \
                 every hook must specify a subcommand name as args[0]",
                spec.event
            )
        });

        // Run `mneme <args>... --help`. clap exits 0 on --help match
        // and 64 (USAGE) on unknown subcommand. We accept any exit
        // status that proves clap recognised the subcommand by
        // checking that stderr does NOT contain the unrecognised
        // subcommand error string.
        let mut cmd = Command::new(bin);
        for arg in spec.args {
            cmd.arg(arg);
        }
        cmd.arg("--help");
        let out = cmd
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn {bin}: {e}"));

        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);

        // clap's "unrecognised subcommand" message is stable across
        // recent versions: "error: unrecognized subcommand". Catching
        // that string is sufficient because --help on a recognised
        // subcommand always succeeds (exit 0) and never emits that
        // string to either pipe.
        let combined = format!("{stderr}\n{stdout}");
        assert!(
            !combined.contains("unrecognized subcommand"),
            "HOOK_SPECS registers `{first}` for event {:?}, but the \
             `mneme` binary has no clap subcommand by that name. This \
             is the BUG-NEW-Q regression: every hook fire would error. \
             Add the subcommand to the `Cli` enum in cli/src/main.rs \
             and dispatch it in `dispatch()`. clap output:\n{}",
            spec.event,
            combined.trim()
        );

        // Belt-and-suspenders: --help should succeed (exit 0) when
        // clap recognises the subcommand. Some hook subcommands may
        // require a flag we haven't supplied, but --help fires before
        // arg validation.
        assert!(
            out.status.success(),
            "`mneme {} --help` should succeed but exited {:?}; \
             clap output:\n{}",
            spec.args.join(" "),
            out.status.code(),
            combined.trim()
        );
    }
}

#[test]
fn the_three_wave_1e_hooks_are_specifically_present() {
    // Pin the Wave 1E subcommands by name. If a future refactor
    // accidentally renames or removes them this test fails before
    // anyone ships a release that breaks Claude Code integration.
    let bin = env!("CARGO_BIN_EXE_mneme");
    for sub in [
        "userprompt-submit",
        "pretool-edit-write",
        "pretool-grep-read",
    ] {
        let out = Command::new(bin)
            .arg(sub)
            .arg("--help")
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn {bin} {sub} --help: {e}"));
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !format!("{stderr}\n{stdout}").contains("unrecognized subcommand"),
            "Wave 1E subcommand `{sub}` is not registered in the Cli enum. \
             This is BUG-NEW-Q. See cli/src/main.rs."
        );
        assert!(
            out.status.success(),
            "`mneme {sub} --help` exited {:?} — expected 0",
            out.status.code()
        );
    }
}
