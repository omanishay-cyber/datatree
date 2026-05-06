//! `mneme doctor` — health check / self-test.
//!
//! v0.3.1: human-readable summary first (closes F-006 from the
//! install-report — prior output was an unbounded raw-JSON dump),
//! optional `--json` for machine output. Diagnostics run in-process
//! (version, runtime/state dir writable, Windows MSVC build toolchain)
//! plus a live supervisor ping.
//! v0.3.1+: per-MCP-tool probe — spawns a fresh `mneme mcp stdio`
//! child, runs a JSON-RPC `initialize` + `tools/list` handshake, and
//! reports a ✓ for every tool the MCP server actually exposes.
//! v0.3.1++: Windows MSVC probe expanded to four signals (link.exe,
//! cl.exe, vswhere with VC.Tools.x86.x64 component, Windows SDK
//! kernel32.lib) plus a one-line PASS/FAIL summary row. Closes I-16.

// ── submodules ────────────────────────────────────────────────────────────────
mod daemon_probe;
mod hooks_probe;
mod mcp_probe;
mod models_probe;
mod render;
mod toolchain_probe;
mod update_probe;

// ── public re-exports (preserve the external API surface) ────────────────────
pub use daemon_probe::{check_daemon_pid_liveness, is_writable, which_on_path, DaemonPidState};
pub use hooks_probe::{compose_hooks_message, is_claude_code_running, render_hooks_registered_box};
pub use mcp_probe::{
    format_probe_failure, probe_mcp_tools, render_mcp_bridge_box, render_mcp_integrations_box,
    render_mcp_tool_probe_box,
};
pub use models_probe::{render_concepts_persistence_box, render_models_box};
pub use render::{format_available_row, line, print_banner, print_banner_line, utc_now_readable};
pub use toolchain_probe::{
    check_build_toolchain, install_hint_for, print_build_toolchain_section, probe_all_toolchain,
    probe_tool, probe_whisper_model, probe_whisper_runtime_summary, render_toolchain_box,
    run_strict, ToolProbe, ToolSeverity, ToolchainEntry, KNOWN_TOOLCHAIN, MSVC_INSTALL_HINT,
};
pub use update_probe::render_update_channel_box;

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::{IpcRequest, IpcResponse};

// ── module-level constants ────────────────────────────────────────────────────

/// Bug M10 (D-window class): canonical Windows process-creation flags
/// for the `mneme mcp stdio` probe spawned by `probe_mcp_tools`. Sets
/// `CREATE_NO_WINDOW` (`0x08000000`) so no console window flashes
/// when `mneme doctor` runs from a hook context (or as part of
/// `mneme audit --self-check`). The constant is exposed
/// unconditionally so pure-Rust unit tests can pin the contract on
/// every host platform — the `cmd.creation_flags(...)` call site is
/// `#[cfg(windows)]` only.
pub(crate) fn windows_doctor_mcp_probe_flags() -> u32 {
    /// CREATE_NO_WINDOW from `windows-sys`: suppresses console window
    /// allocation for the child process.
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    CREATE_NO_WINDOW
}

/// JSON-RPC `clientInfo.name` we identify as when probing the MCP
/// server during `mneme doctor`. Intentionally fixed and distinct from
/// real clients so server-side telemetry can recognise the probe.
/// Closes NEW-027.
pub const MCP_CLIENT_NAME: &str = "mneme-doctor";

// ── CLI arg types ─────────────────────────────────────────────────────────────

/// CLI args for `mneme doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Skip the live IPC probe (in-process diagnostics only).
    #[arg(long)]
    pub offline: bool,

    /// Dump the raw supervisor status JSON (default is the friendly
    /// summary only).
    #[arg(long)]
    pub json: bool,

    /// Skip the per-MCP-tool health probe (spawns a fresh
    /// `mneme mcp stdio` child to enumerate the live tool set).
    #[arg(long)]
    pub skip_mcp_probe: bool,

    /// G11: pre-flight verification mode. Runs the full toolchain probe
    /// and exits non-zero if any HIGH-severity tool is missing.
    #[arg(long)]
    pub strict: bool,
}

// ── shared data types ─────────────────────────────────────────────────────────

/// One row in a doctor section. `value` is rendered after a `:` and
/// padded to the box width.
#[derive(Debug, Clone)]
pub struct DoctorRow {
    pub label: String,
    pub value: String,
}

impl DoctorRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

// ── binary inventory ──────────────────────────────────────────────────────────

/// Return the list of mneme binary filenames the doctor expects to find
/// on disk next to `mneme(.exe)`. Order is stable so rows don't
/// reshuffle between runs.
pub fn expected_binary_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &[
            "mneme.exe",
            "mneme-daemon.exe",
            "mneme-brain.exe",
            "mneme-parsers.exe",
            "mneme-scanners.exe",
            "mneme-livebus.exe",
            "mneme-md-ingest.exe",
            "mneme-store.exe",
            "mneme-multimodal.exe",
        ]
    }
    #[cfg(not(windows))]
    {
        &[
            "mneme",
            "mneme-daemon",
            "mneme-brain",
            "mneme-parsers",
            "mneme-scanners",
            "mneme-livebus",
            "mneme-md-ingest",
            "mneme-store",
            "mneme-multimodal",
        ]
    }
}

/// Path to the MCP entry `index.ts` inside the user's mneme install.
/// Routes through `PathManager::default_root()` so `MNEME_HOME`
/// overrides are honored consistently with the rest of the CLI.
pub fn mcp_entry_path() -> Option<std::path::PathBuf> {
    Some(
        common::paths::PathManager::default_root()
            .root()
            .join("mcp")
            .join("src")
            .join("index.ts"),
    )
}

// ── entry point ───────────────────────────────────────────────────────────────

/// Entry point used by `main.rs`.
pub async fn run(args: DoctorArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // G11: --strict short-circuits the regular run with a focused
    // pre-flight verifier (toolchain probes + binary self-test). Exits
    // non-zero if any HIGH-severity tool is missing.
    if args.strict {
        let code = run_strict();
        if code != 0 {
            std::process::exit(code);
        }
        return Ok(());
    }

    print_banner();
    println!();
    println!("  {:<16}{}", "timestamp:", utc_now_readable());
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ mneme doctor · health check                             │");
    println!("├─────────────────────────────────────────────────────────┤");

    let runtime = crate::runtime_dir();
    let state = crate::state_dir();
    line("runtime dir", &runtime.display().to_string());
    line("state   dir", &state.display().to_string());
    let rt_ok = is_writable(&runtime);
    let st_ok = is_writable(&state);
    line("runtime writable", if rt_ok { "yes ✓" } else { "NO ✗" });
    line("state   writable", if st_ok { "yes ✓" } else { "NO ✗" });

    if args.offline {
        println!("└─────────────────────────────────────────────────────────┘");
        print_build_toolchain_section();
        // K1: hooks-registered check is filesystem-only — works offline.
        render_hooks_registered_box();
        // Bug C: filesystem-only too — works offline.
        render_models_box();
        render_concepts_persistence_box();
        // Wave 2.4: update channel box is filesystem-only (reads cached JSON).
        render_update_channel_box();
        return Ok(());
    }

    // B-017 v2 (concurrency-audit F6 fix, 2026-04-30): doctor MUST NOT
    // auto-spawn a daemon. The outer 3s timeout below would interrupt
    // the auto-spawn-then-retry path mid-poll, leaving an orphaned
    // `mneme daemon start` process.
    let client = make_client(socket_override).with_no_autospawn();
    // B-017/B-018: doctor must never hang. Two safeguards:
    //   1. If daemon.pid is stale, skip IPC entirely — a wedged stale
    //      named pipe can accept connects and then block read_exact.
    //   2. Even when the PID looks alive, cap the liveness probe at 3s.
    let pid_state = check_daemon_pid_liveness(&state);
    let is_up = match pid_state {
        DaemonPidState::AliveProbeFresh => {
            match tokio::time::timeout(std::time::Duration::from_secs(3), client.is_running()).await
            {
                Ok(up) => up,
                Err(_) => {
                    warn!(
                        "doctor: daemon PID is alive but supervisor did not answer Ping in 3s — treating as down"
                    );
                    false
                }
            }
        }
        DaemonPidState::Stale => {
            warn!("doctor: stale ~/.mneme/run/daemon.pid (process not alive) — supervisor is down");
            false
        }
        DaemonPidState::Missing => false,
    };
    let supervisor_label = match (is_up, pid_state) {
        (true, _) => "running ✓",
        (false, DaemonPidState::Stale) => "NOT RUNNING ✗ (stale PID file)",
        (false, DaemonPidState::AliveProbeFresh) => "NOT RESPONDING ✗ (3s ping timeout)",
        (false, DaemonPidState::Missing) => "NOT RUNNING ✗",
    };
    line("supervisor", supervisor_label);
    line(
        "query path",
        if is_up {
            "supervisor ✓"
        } else {
            "direct-db (supervisor down)"
        },
    );
    if !is_up {
        println!("└─────────────────────────────────────────────────────────┘");
        print_build_toolchain_section();
        println!();
        render_mcp_bridge_box();
        render_hooks_registered_box();
        render_models_box();
        render_concepts_persistence_box();
        if !args.skip_mcp_probe {
            render_mcp_tool_probe_box();
            render_mcp_integrations_box();
        }
        render_update_channel_box();
        println!();
        println!("start the daemon with:  mneme daemon start");
        return Ok(());
    }

    let resp = client.request(IpcRequest::Status { project: None }).await?;
    if let IpcResponse::Status { ref children } = resp {
        let total = children.len();
        let mut running = 0usize;
        let mut pending = 0usize;
        let mut failed = 0usize;
        let mut restarts = 0u64;
        for child in children {
            let status = child
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            // HIGH-4 + HIGH-5 fix (2026-05-06 audit): count `dead` and
            // `degraded` against the failed bucket.
            match status {
                "running" | "healthy" => running += 1,
                "pending" | "starting" => pending += 1,
                "failed" | "crashed" | "dead" | "degraded" => failed += 1,
                _ => {}
            }
            if let Some(r) = child.get("restart_count").and_then(|v| v.as_u64()) {
                restarts += r;
            }
        }
        line(
            "workers",
            &format!("{total} total  ({running} up, {pending} pending, {failed} failed)"),
        );
        line("restarts", &restarts.to_string());
        println!("└─────────────────────────────────────────────────────────┘");
        println!();

        println!("┌─────────────────────────────────────────────────────────┐");
        println!("│ per-worker health                                       │");
        println!("├─────────────────────────────────────────────────────────┤");
        for child in children {
            let name = child.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let status = child.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            let pid = child
                .get("pid")
                .and_then(|v| v.as_u64())
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            let uptime_ms = child
                .get("current_uptime_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let restarts = child
                .get("restart_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            // Bug L: surface dropped-restart count next to restarts.
            let dropped = child
                .get("restart_dropped_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            // HIGH-4 + HIGH-5 fix: treat `dead` and `degraded` as their
            // own glyphs so a stuck worker is visible at a glance.
            let mark = match status {
                "running" | "healthy" => "✓",
                "pending" | "starting" => "...",
                "failed" | "crashed" | "dead" => "✗",
                "degraded" => "⚠",
                _ => "?",
            };
            let uptime_str = if uptime_ms > 0 {
                format!("{}s", uptime_ms / 1000)
            } else {
                "-".to_string()
            };
            // B15 (2026-05-02): humanise per-worker latency.
            let p50_us = child.get("p50_us").and_then(|v| v.as_u64()).unwrap_or(0);
            let p99_us = child.get("p99_us").and_then(|v| v.as_u64()).unwrap_or(0);
            let latency_suffix = if p50_us > 0 || p99_us > 0 {
                format!(
                    "  typical={}ms  slow_tail={}ms",
                    p50_us / 1000,
                    p99_us / 1000
                )
            } else {
                String::new()
            };
            let queue_depth = child
                .get("queue_depth")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let queue_suffix = if queue_depth > 0 {
                format!("  queue={queue_depth}")
            } else {
                String::new()
            };
            // HIGH-4 fix: surface 24h cumulative restart count when it
            // diverges from the lifetime count.
            let restarts_24h = child
                .get("restart_count_24h")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let restart_24h_suffix =
                if restarts_24h > 0 && (restarts_24h != restarts || restarts >= 10) {
                    format!("  restarts_24h={restarts_24h}")
                } else {
                    String::new()
                };
            // HIGH-5 fix: surface Degraded-dwell time.
            let degraded_for_secs = child
                .get("degraded_for_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let degraded_suffix = if status == "degraded" && degraded_for_secs > 0 {
                let mins = degraded_for_secs / 60;
                format!("  degraded_for={mins}m")
            } else {
                String::new()
            };
            line(
                &format!("{mark} {name}"),
                &format!(
                    "status={status:<9}  pid={pid:<6}  uptime={uptime_str:<6}  restarts={restarts}  dropped={dropped}{restart_24h_suffix}{degraded_suffix}{queue_suffix}{latency_suffix}"
                ),
            );
        }
        println!("└─────────────────────────────────────────────────────────┘");

        println!();
        println!("┌─────────────────────────────────────────────────────────┐");
        println!("│ binaries on disk                                        │");
        println!("├─────────────────────────────────────────────────────────┤");
        let bin_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        if let Some(dir) = bin_dir {
            for b in expected_binary_names() {
                let p = dir.join(b);
                let ok = p.exists();
                let size = if ok {
                    std::fs::metadata(&p)
                        .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
                        .unwrap_or_else(|_| "?".to_string())
                } else {
                    "MISSING".to_string()
                };
                let mark = if ok { "✓" } else { "✗" };
                line(&format!("{mark} {b}"), &size);
            }
        }
        println!("└─────────────────────────────────────────────────────────┘");

        render_mcp_bridge_box();
        render_hooks_registered_box();
        render_models_box();
        render_concepts_persistence_box();

        if !args.skip_mcp_probe {
            render_mcp_tool_probe_box();
            render_mcp_integrations_box();
        }

        print_build_toolchain_section();
        render_update_channel_box();
        if args.json {
            println!();
            println!("raw status:");
            println!("{}", serde_json::to_string_pretty(&children)?);
        }
    } else {
        println!("└─────────────────────────────────────────────────────────┘");
        print_build_toolchain_section();
        render_update_channel_box();
        warn!(?resp, "supervisor returned non-status response");
    }
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_on_path_finds_known_tool() {
        assert!(
            which_on_path("cargo").is_some(),
            "cargo must be on PATH when running cargo test"
        );
    }

    #[test]
    fn which_on_path_missing_tool_returns_none() {
        let needle = "this-binary-should-not-exist-on-any-developer-machine-12345";
        assert!(which_on_path(needle).is_none());
    }

    #[test]
    fn doctor_row_constructs_with_string_slice() {
        let row = DoctorRow::new("label", "value");
        assert_eq!(row.label, "label");
        assert_eq!(row.value, "value");
    }

    #[test]
    fn format_probe_failure_includes_exit_code_when_nonzero() {
        let out = format_probe_failure("timed out after 10s", Some(7), &[]);
        assert!(out.contains("(exit=7)"), "expected '(exit=7)' in: {out}");
        assert!(out.contains("timed out after 10s"));
    }

    #[test]
    fn format_probe_failure_omits_exit_code_when_zero_or_unknown() {
        let out_zero = format_probe_failure("ok-but-malformed", Some(0), &[]);
        assert!(!out_zero.contains("exit="), "got: {out_zero}");
        let out_none = format_probe_failure("killed", None, &[]);
        assert!(!out_none.contains("exit="), "got: {out_none}");
    }

    #[test]
    fn format_probe_failure_appends_stderr_tail() {
        let stderr = b"line one\nline two\nline three\n";
        let out = format_probe_failure("bad json", Some(1), stderr);
        assert!(out.contains("stderr tail:"), "got: {out}");
        assert!(out.contains("line three"), "got: {out}");
        assert!(out.contains("(exit=1)"), "got: {out}");
    }

    #[test]
    fn format_probe_failure_skips_stderr_tail_when_empty() {
        let out = format_probe_failure("nothing", Some(2), &[]);
        assert!(!out.contains("stderr tail:"), "got: {out}");
    }

    #[test]
    fn format_probe_failure_caps_stderr_to_last_20_lines() {
        let mut stderr = String::new();
        for i in 1..=30 {
            stderr.push_str(&format!("L{i}\n"));
        }
        let out = format_probe_failure("boom", Some(1), stderr.as_bytes());
        assert!(!out.contains("L1 |"), "L1 should have been trimmed: {out}");
        assert!(
            !out.contains("L10 |"),
            "L10 should have been trimmed: {out}"
        );
        assert!(out.contains("L11"), "L11 should remain: {out}");
        assert!(out.contains("L30"), "L30 should remain: {out}");
    }

    #[cfg(not(windows))]
    #[test]
    fn check_build_toolchain_empty_on_non_windows() {
        assert!(check_build_toolchain().is_empty());
    }

    #[test]
    fn copyright_constant_carries_both_names() {
        use super::render::COPYRIGHT;
        assert!(COPYRIGHT.contains("Anish Trivedi"));
        assert!(COPYRIGHT.contains("Kruti Trivedi"));
    }

    #[test]
    fn mcp_client_name_is_doctor_marker() {
        assert_eq!(MCP_CLIENT_NAME, "mneme-doctor");
    }

    #[test]
    fn known_toolchain_covers_g1_through_g12() {
        let ids: Vec<&str> = KNOWN_TOOLCHAIN.iter().map(|t| t.issue_id).collect();
        for expected in &[
            "G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8", "G9", "G10", "G11", "G12",
        ] {
            assert!(
                ids.contains(expected),
                "KNOWN_TOOLCHAIN missing entry for {expected}"
            );
        }
    }

    #[test]
    fn known_toolchain_severities_match_phase_a_priorities() {
        let by_id = |id: &str| {
            KNOWN_TOOLCHAIN
                .iter()
                .find(|t| t.issue_id == id)
                .unwrap()
                .severity
        };
        assert_eq!(by_id("G1"), ToolSeverity::High);
        assert_eq!(by_id("G2"), ToolSeverity::High);
        assert_eq!(by_id("G3"), ToolSeverity::High);
        assert_eq!(by_id("G4"), ToolSeverity::Medium);
        assert_eq!(by_id("G5"), ToolSeverity::High);
        assert_eq!(by_id("G6"), ToolSeverity::Medium);
        assert_eq!(by_id("G7"), ToolSeverity::Low);
        assert_eq!(by_id("G8"), ToolSeverity::Low);
        assert_eq!(by_id("G9"), ToolSeverity::Medium);
        assert_eq!(by_id("G10"), ToolSeverity::Low);
        assert_eq!(by_id("G11"), ToolSeverity::Low);
        assert_eq!(by_id("G12"), ToolSeverity::Low);
    }

    #[test]
    fn g12_whisper_probes_are_whisper_cli_names() {
        let g12 = KNOWN_TOOLCHAIN
            .iter()
            .find(|t| t.issue_id == "G12")
            .unwrap();
        for probe in g12.probes {
            assert!(
                !probe.ends_with(".bin"),
                "G12 probe '{probe}' must not be a .bin filename"
            );
        }
        assert!(
            g12.probes.contains(&"whisper-cli"),
            "G12 probes must include 'whisper-cli'"
        );
    }

    #[test]
    fn probe_whisper_model_returns_a_row_without_panicking() {
        let row = probe_whisper_model();
        assert!(!row.label.is_empty());
        assert!(!row.value.is_empty());
    }

    #[test]
    fn probe_whisper_runtime_summary_returns_a_row_without_panicking() {
        let row = probe_whisper_runtime_summary();
        assert!(!row.label.is_empty());
        assert!(!row.value.is_empty());
    }

    #[test]
    fn known_toolchain_install_hints_are_actionable() {
        for entry in KNOWN_TOOLCHAIN {
            assert!(
                !entry.hint_windows.is_empty(),
                "windows hint missing for {}",
                entry.issue_id
            );
            assert!(
                !entry.hint_unix.is_empty(),
                "unix hint missing for {}",
                entry.issue_id
            );
        }
    }

    #[test]
    fn probe_tool_returns_present_for_cargo_during_cargo_test() {
        let rust_entry = KNOWN_TOOLCHAIN
            .iter()
            .find(|t| t.issue_id == "G1")
            .expect("G1 entry");
        let probe = probe_tool(rust_entry);
        assert!(
            probe.is_present(),
            "rust toolchain probe failed during cargo test — env is broken"
        );
    }

    #[test]
    fn probe_tool_marks_known_missing_tool_absent() {
        let bogus = ToolchainEntry {
            display: "Bogus",
            probes: &["this-binary-is-not-installed-anywhere-12345"],
            cargo_subcommand: None,
            severity: ToolSeverity::Low,
            issue_id: "G99",
            purpose: "test fixture",
            hint_windows: "n/a",
            hint_unix: "n/a",
        };
        let probe = probe_tool(&bogus);
        assert!(!probe.is_present());
        assert!(probe.version.is_none());
    }

    #[test]
    fn install_hint_for_picks_platform_string() {
        let entry = &KNOWN_TOOLCHAIN[0]; // Rust
        let hint = install_hint_for(entry);
        if cfg!(windows) {
            assert_eq!(hint, entry.hint_windows);
        } else {
            assert_eq!(hint, entry.hint_unix);
        }
    }

    #[test]
    fn msvc_install_hint_mentions_winget_and_buildtools() {
        assert!(MSVC_INSTALL_HINT.contains("winget install"));
        assert!(MSVC_INSTALL_HINT.contains("Microsoft.VisualStudio.2022.BuildTools"));
        assert!(MSVC_INSTALL_HINT.contains("VS Installer"));
    }

    #[test]
    fn hooks_remediation_message_zero_drops_enable_hooks_flag() {
        // Access private helper via compose_hooks_message with None/None.
        let msg = compose_hooks_message(0, 8, None, None);
        assert!(
            msg.contains("mneme install"),
            "remediation must mention `mneme install`: {msg}"
        );
        assert!(
            !msg.contains("--enable-hooks"),
            "remediation must NOT contain the deprecated `--enable-hooks` flag: {msg}"
        );
    }

    #[test]
    fn hooks_remediation_message_partial_drops_enable_hooks_flag() {
        let msg = compose_hooks_message(3, 8, None, None);
        assert!(
            msg.contains("mneme install"),
            "remediation must mention `mneme install`: {msg}"
        );
        assert!(
            msg.contains("--force"),
            "partial-registration remediation must keep `--force`: {msg}"
        );
        assert!(
            !msg.contains("--enable-hooks"),
            "remediation must NOT contain the deprecated `--enable-hooks` flag: {msg}"
        );
    }

    #[test]
    fn hooks_remediation_message_full_does_not_remediate() {
        let msg = compose_hooks_message(8, 8, None, None);
        assert_eq!(msg, "8/8 entries registered");
        assert!(!msg.contains("re-run"));
        assert!(!msg.contains("--enable-hooks"));
    }

    #[test]
    fn windows_doctor_mcp_probe_flags() {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let flags = super::windows_doctor_mcp_probe_flags();
        assert_eq!(
            flags & CREATE_NO_WINDOW,
            CREATE_NO_WINDOW,
            "doctor mcp-probe spawn must set CREATE_NO_WINDOW (0x08000000); got {flags:#010x}"
        );
    }

    #[test]
    fn is_claude_code_running_never_panics() {
        let _ = super::is_claude_code_running();
    }

    #[test]
    fn message_when_claude_running_and_hooks_missing_calls_out_pid() {
        let msg = super::compose_hooks_message(0, 8, Some(98765), None);
        assert!(msg.contains("0/8"), "must show count: {msg}");
        assert!(
            msg.to_lowercase().contains("claude"),
            "must name Claude Code: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("running") || msg.to_lowercase().contains("open"),
            "must indicate Claude is alive: {msg}"
        );
        assert!(msg.contains("98765"), "must include the PID: {msg}");
        assert!(
            msg.to_lowercase().contains("close"),
            "must tell the user to close Claude: {msg}"
        );
    }

    #[test]
    fn message_when_claude_not_running_and_hooks_missing_keeps_install_remediation() {
        let msg = super::compose_hooks_message(0, 8, None, None);
        assert!(msg.contains("0/8"), "must show count: {msg}");
        assert!(
            msg.contains("mneme install"),
            "true-negative must point to `mneme install`: {msg}"
        );
        assert!(
            !msg.to_lowercase().contains("running"),
            "must not blame Claude when it isn't open: {msg}"
        );
    }

    #[test]
    fn message_when_claude_running_and_hooks_present_emits_restart_reminder() {
        let msg = super::compose_hooks_message(8, 8, Some(12345), None);
        assert!(msg.contains("8/8"), "must show count: {msg}");
        assert!(
            msg.to_lowercase().contains("restart")
                || msg.to_lowercase().contains("won't fire")
                || msg.to_lowercase().contains("won't pick"),
            "running-Claude-with-hooks-present must remind to restart: {msg}"
        );
    }

    #[test]
    fn message_when_claude_not_running_and_hooks_present_is_clean() {
        let msg = super::compose_hooks_message(8, 8, None, None);
        assert!(msg.contains("8/8"), "must show count: {msg}");
        assert!(
            msg.contains("entries registered"),
            "happy path must show the existing 'entries registered' line: {msg}"
        );
        assert!(
            !msg.to_lowercase().contains("restart"),
            "happy path must not emit a restart reminder: {msg}"
        );
    }

    #[test]
    fn message_when_read_error_surfaces_diagnostic() {
        let parse_err = "settings.json failed to parse as JSON: trailing comma at line 12";
        let msg = super::compose_hooks_message(0, 8, None, Some(parse_err.to_string()));
        assert!(
            msg.contains("settings.json"),
            "diagnostic must mention the file: {msg}"
        );
        assert!(
            msg.contains("parse") || msg.contains("trailing comma"),
            "diagnostic must surface the concrete reason: {msg}"
        );
    }

    #[test]
    fn message_with_claude_running_and_read_error_combines_both_signals() {
        let parse_err = "unexpected end of JSON input";
        let msg = super::compose_hooks_message(0, 8, Some(54321), Some(parse_err.to_string()));
        assert!(msg.contains("54321"), "must include PID: {msg}");
        assert!(
            msg.contains("parse") || msg.contains("unexpected end"),
            "must surface parse error: {msg}"
        );
    }
}
