//! Claude Code hooks probe: check that mneme's hook entries are
//! registered in `~/.claude/settings.json`, detect a running Claude
//! process that may be holding the file, and render the "Claude Code
//! hooks" doctor box.

use super::render::line;

// ─── remediation message builders ────────────────────────────────────────────

/// Render the right-hand "value" string for the `hooks_registered` row.
///
/// **M2 (audit DEEP-AUDIT-2026-04-29.md §M2):** After the K1 fix in
/// v0.3.2, `mneme install` defaults hooks ON and `--enable-hooks` is a
/// deprecated no-op. Remediation copy therefore says `re-run \`mneme
/// install\`` — not the deprecated flag.
///
/// Pure (no I/O) so it can be unit-tested without a real settings tree.
fn hooks_remediation_message(count: usize, expected: usize) -> String {
    if count == expected {
        format!("{count}/{expected} entries registered")
    } else if count == 0 {
        format!("{count}/{expected} — re-run `mneme install` to register")
    } else {
        format!("{count}/{expected} — partial registration; re-run `mneme install --force`")
    }
}

/// B-AGENT-C-2 (v0.3.2): compose the full doctor message for the hooks
/// row, taking Claude Code's running-state into account.
///
/// Truth table:
///
/// | count == expected | claude_running | message
/// |-------------------|-------------------------------------------------
/// | yes               | None           | "8/8 entries registered"
/// | yes               | Some(pid)      | "+ note: restart Claude"
/// | no (count == 0)   | None           | "0/N — re-run `mneme install`"
/// | no (count == 0)   | Some(pid)      | "0/N + claude is running"
/// | no (partial)      | None           | "M/N — partial; re-run --force"
/// | no (partial)      | Some(pid)      | "M/N + claude is running"
///
/// `parse_error` is an orthogonal overlay that surfaces Layer-1 read
/// failures instead of silently degrading to a zero count.
///
/// Pure (no I/O); Claude state and parse-error are passed in by the
/// caller so this can be unit-tested deterministically.
pub fn compose_hooks_message(
    count: usize,
    expected: usize,
    claude_pid: Option<u32>,
    parse_error: Option<String>,
) -> String {
    // Layer 1 overlay: a concrete read / parse error trumps every other
    // signal — surface it so the user knows the file is broken, not
    // empty.
    //
    // A1-008 (2026-05-04): drop `count/expected` from the parse-error
    // branch. When parse fails, the count is meaningless.
    if let Some(err) = parse_error {
        let _ = (count,); // intentionally unused per audit A1-008
        if let Some(pid) = claude_pid {
            return format!(
                "could not determine: settings.json did not parse ({err}). \
                 Claude Code is RUNNING (PID {pid}); it may be holding the file. \
                 [{expected} hooks expected; close Claude entirely and re-run `mneme doctor` to verify.]"
            );
        }
        return format!(
            "could not determine: settings.json did not parse ({err}). \
             [{expected} hooks expected; open the file and check its JSON, then re-run `mneme install`.]"
        );
    }

    let all_present = count == expected;
    let none_present = count == 0;

    match (all_present, claude_pid) {
        (true, None) => hooks_remediation_message(count, expected),

        (true, Some(pid)) => format!(
            "{count}/{expected} entries registered. Note: Claude Code is running \
             (PID {pid}); new hooks won't fire in already-open sessions — \
             restart Claude to pick them up."
        ),

        (false, Some(pid)) if none_present => format!(
            "{count}/{expected} detected, but Claude Code is RUNNING (PID {pid}). \
             Claude may be holding settings.json with an in-memory copy that does \
             not include mneme hooks. Close Claude entirely and re-run \
             `mneme doctor` to verify. If still missing, run `mneme install` \
             to re-register."
        ),

        (false, None) if none_present => hooks_remediation_message(count, expected),

        (false, Some(pid)) => format!(
            "{count}/{expected} — partial registration; Claude Code is RUNNING \
             (PID {pid}) and may have rewritten settings.json. Close Claude \
             entirely and re-run `mneme install --force`."
        ),

        (false, None) => hooks_remediation_message(count, expected),
    }
}

// ─── Claude process detection ─────────────────────────────────────────────────

/// B-AGENT-C-2 (v0.3.2): is Claude Code currently running on this host?
///
/// Returns the PID of the first matching process, or None. Cross-platform
/// via the workspace `sysinfo` dep already used by abort.rs and
/// build_lock.rs.
///
/// Recognition heuristics (any one match):
///   - process name (case-insensitive) == "claude.exe" / "claude"
///   - exe path or argv[0] contains "claude-code" / "claude_code"
///
/// A1-009 (2026-05-04): tightened from joined-cmdline substring to
/// exe-path / argv[0] match only. The previous heuristic matched ANY
/// substring in the joined command line, so a text editor with a file
/// named "claude-code-readme.md" open would trigger "Claude is RUNNING".
pub fn is_claude_code_running() -> Option<u32> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new());
    for (pid, proc_) in sys.processes() {
        let name = proc_.name().to_string_lossy().to_lowercase();
        if name == "claude.exe" || name == "claude" {
            return Some(pid.as_u32());
        }
        let exe_path: String = proc_
            .exe()
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let argv0: String = proc_
            .cmd()
            .first()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let identifies_claude_code = |hay: &str| -> bool {
            hay.contains("claude-code")
                || hay.contains("claude_code")
                || hay.ends_with("\\claude.exe")
                || hay.ends_with("/claude")
        };
        if identifies_claude_code(&exe_path) || identifies_claude_code(&argv0) {
            return Some(pid.as_u32());
        }
    }
    None
}

// ─── box renderer ─────────────────────────────────────────────────────────────

/// K1 / Phase A §K1: render the "Claude Code hooks" box.
/// Reports whether mneme's 8 hook entries are registered in
/// `~/.claude/settings.json`. Green when all 8 are present, red
/// otherwise. Emitted on both supervisor-up and supervisor-down paths.
///
/// B-AGENT-C-2 (v0.3.2): uses `compose_hooks_message` (above) to
/// explain WHY hooks appear missing when Claude is running.
pub fn render_hooks_registered_box() {
    use crate::platforms::claude_code::{
        count_registered_mneme_hooks_detailed, HookFileReadState, HOOK_SPECS,
    };

    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Claude Code hooks (~/.claude/settings.json)             │");
    println!("├─────────────────────────────────────────────────────────┤");

    let settings_path = dirs::home_dir().map(|h| h.join(".claude").join("settings.json"));
    let path_str = settings_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "?".into());

    let claude_pid = is_claude_code_running();

    match settings_path.as_ref() {
        Some(p) => {
            let r = count_registered_mneme_hooks_detailed(p);
            match &r.read_state {
                HookFileReadState::Missing => {
                    let value = compose_hooks_message(0, r.expected, claude_pid, None);
                    line("✗ hooks_registered", &value);
                    line("settings.json", &p.display().to_string());
                    line(
                        "  status",
                        "settings.json does not exist (mneme install has not run)",
                    );
                }
                HookFileReadState::UnreadableIo(io_msg) => {
                    let value = compose_hooks_message(
                        0,
                        r.expected,
                        claude_pid,
                        Some(format!("io error: {io_msg}")),
                    );
                    line("✗ hooks_registered", &value);
                    line("settings.json", &path_str);
                }
                HookFileReadState::Read => {
                    let mark = if r.count == r.expected { "✓" } else { "✗" };
                    let value = compose_hooks_message(
                        r.count,
                        r.expected,
                        claude_pid,
                        r.parse_error.clone(),
                    );
                    line(&format!("{mark} hooks_registered"), &value);
                    line("settings.json", &path_str);
                    // Per-event breakdown so users can see which event is
                    // missing without opening the JSON. Only render when
                    // parse succeeded — otherwise the body isn't trustworthy.
                    if r.count != r.expected && r.parse_error.is_none() {
                        let body = std::fs::read_to_string(p).unwrap_or_default();
                        let parsed: serde_json::Value =
                            serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
                        let hooks_obj = parsed
                            .get("hooks")
                            .and_then(|v| v.as_object())
                            .cloned()
                            .unwrap_or_default();
                        for spec in HOOK_SPECS {
                            let present = hooks_obj
                                .get(spec.event)
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter().any(|e| {
                                        e.get("_mneme")
                                            .and_then(|m| m.get("managed"))
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false);
                            let m = if present { "✓" } else { "✗" };
                            line(
                                &format!("  {m} {}", spec.event),
                                if present { "yes" } else { "no" },
                            );
                        }
                    }
                }
            }
        }
        None => {
            line("✗ hooks_registered", "could not resolve home dir");
        }
    }
    println!("└─────────────────────────────────────────────────────────┘");
}
