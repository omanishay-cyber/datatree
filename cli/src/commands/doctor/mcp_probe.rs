//! MCP-related probes: bridge health, per-tool self-test (JSON-RPC
//! handshake), and integration status (AI-host registry check).

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command as StdCommand, Stdio};
use std::time::{Duration, Instant};

use super::daemon_probe::which_on_path;
use super::hooks_probe::is_claude_code_running;
use super::render::line;

// ─── MCP bridge box ───────────────────────────────────────────────────────────

/// Render the "MCP bridge" box (entry path + bun runtime). Split out so
/// we can emit it on both the supervisor-up and supervisor-down paths.
pub fn render_mcp_bridge_box() {
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ MCP bridge                                              │");
    println!("├─────────────────────────────────────────────────────────┤");
    let mcp_entry = super::mcp_entry_path();
    let mcp_exists = mcp_entry.as_ref().map(|p| p.exists()).unwrap_or(false);
    line(
        if mcp_exists {
            "✓ MCP entry"
        } else {
            "✗ MCP entry"
        },
        mcp_entry
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "?".into())
            .as_str(),
    );
    let bun_on_path = which_on_path("bun");
    let bun_str = bun_on_path.as_ref().map(|p| p.display().to_string());
    line(
        if bun_on_path.is_some() {
            "✓ bun runtime"
        } else {
            "✗ bun runtime"
        },
        bun_str.as_deref().unwrap_or("not on PATH"),
    );
    println!("└─────────────────────────────────────────────────────────┘");
}

// ─── MCP self-test (tool probe) ───────────────────────────────────────────────

/// A1-001 (2026-05-04): RENAMED from "per-MCP-tool health" to "MCP
/// self-test". The old label implied this proved Claude Code (or any
/// other AI host) was actually using mneme. It does not — it only
/// proves THIS binary can spawn its own MCP server and list its tools.
/// Real integration verification lives in render_mcp_integrations_box.
pub fn render_mcp_tool_probe_box() {
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ MCP self-test (mneme can serve its own tools)           │");
    println!("├─────────────────────────────────────────────────────────┤");
    match probe_mcp_tools(Duration::from_secs(10)) {
        Ok(tools) => {
            for t in &tools {
                line(&format!("✓ {t}"), "live");
            }
            let count = tools.len();
            let summary = if count >= 40 {
                format!("{count} tools exposed (expected >= 40) ✓")
            } else {
                format!("{count} tools exposed (expected >= 40) ✗")
            };
            line("total", &summary);
        }
        Err(reason) => {
            line("✗ probe", &format!("could not probe MCP server — {reason}"));
        }
    }
    println!("└─────────────────────────────────────────────────────────┘");
}

/// Spawn a fresh `mneme mcp stdio` child, drive the MCP JSON-RPC
/// handshake, and return the list of tool names the server publishes
/// via `tools/list`.
///
/// Fails fast (and cleanly — never hangs the main doctor command) if:
///   - the current exe path cannot be resolved
///   - spawning the child fails
///   - stdin/stdout pipes can't be captured
///   - the child doesn't respond within `deadline`
///   - the `tools/list` response is malformed
///
/// Always kills the child before returning so no zombie Bun processes
/// linger.
pub fn probe_mcp_tools(deadline: Duration) -> Result<Vec<String>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe unavailable: {e}"))?;

    let mut cmd = StdCommand::new(&exe);
    cmd.arg("mcp")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // B3: pipe stderr (was Stdio::null) so failures can echo the
        // child's bun/node diagnostic output back to the doctor report.
        .stderr(Stdio::piped())
        .env("MNEME_LOG", "error")
        .env("NO_COLOR", "1");
    // Bug M10 (D-window class): suppress console-window allocation
    // when this probe runs from a windowless parent.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(super::windows_doctor_mcp_probe_flags());
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;

    let start = Instant::now();

    let mut stderr_pipe = child.stderr.take();
    let mut stdin = match child.stdin.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let exit = child.wait().ok().and_then(|s| s.code());
            let tail = drain_stderr_blocking(&mut stderr_pipe);
            return Err(format_probe_failure("no stdin pipe", exit, &tail));
        }
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let exit = child.wait().ok().and_then(|s| s.code());
            let tail = drain_stderr_blocking(&mut stderr_pipe);
            return Err(format_probe_failure("no stdout pipe", exit, &tail));
        }
    };

    // B3: drain stderr in a worker thread so the child can't block on
    // a full pipe and we always have a buffer to surface on failure.
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let stderr_handle = stderr_pipe.take();
    std::thread::spawn(move || {
        let buf = match stderr_handle {
            Some(mut s) => {
                let mut all = Vec::new();
                let _ = s.read_to_end(&mut all);
                all
            }
            None => Vec::new(),
        };
        let _ = stderr_tx.send(buf);
    });

    // Run the JSON-RPC handshake on a worker thread so we can enforce
    // the deadline from this thread without blocking on `read_line`.
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<String>, String>>();
    std::thread::spawn(move || {
        let res = handshake_and_list(&mut stdin, stdout);
        let _ = tx.send(res);
    });

    let remaining = deadline.saturating_sub(start.elapsed());
    let handshake_result = match rx.recv_timeout(remaining) {
        Ok(res) => res,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(format!("timed out after {}s", deadline.as_secs()))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err("handshake thread died".into()),
    };

    let _ = child.kill();
    let exit_code = child.wait().ok().and_then(|s| s.code());

    let stderr_tail = stderr_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();

    match handshake_result {
        Ok(tools) => Ok(tools),
        Err(reason) => Err(format_probe_failure(&reason, exit_code, &stderr_tail)),
    }
}

/// B3: blocking variant for the early-exit error paths (stdin / stdout
/// pipe missing) where no drainer thread is set up yet. Reads up to
/// ~4 KB of stderr from the child's pipe handle.
fn drain_stderr_blocking(pipe: &mut Option<std::process::ChildStderr>) -> Vec<u8> {
    let mut out = Vec::new();
    if let Some(s) = pipe.as_mut() {
        let _ = s.read_to_end(&mut out);
    }
    out
}

/// B3: format the enriched probe-failure message.
///
///   <reason> (exit=N) — stderr tail: <last lines>
///
/// Exit code suffix is omitted when the child exited cleanly. Stderr
/// tail is bounded to the last 4 KB and the last 20 lines.
pub fn format_probe_failure(reason: &str, exit: Option<i32>, stderr: &[u8]) -> String {
    let mut out = String::from(reason);
    if let Some(code) = exit {
        if code != 0 {
            out.push_str(&format!(" (exit={code})"));
        }
    }
    if !stderr.is_empty() {
        let text = String::from_utf8_lossy(stderr);
        let trimmed: &str = if text.len() > 4096 {
            let cut = text.len() - 4096;
            let mut start = cut;
            while start < text.len() && !text.is_char_boundary(start) {
                start += 1;
            }
            &text[start..]
        } else {
            &text
        };
        let lines: Vec<&str> = trimmed.lines().filter(|l| !l.trim().is_empty()).collect();
        let take = lines.len().min(20);
        let tail_lines = &lines[lines.len() - take..];
        let joined = tail_lines.join(" | ");
        if !joined.is_empty() {
            out.push_str(&format!(" — stderr tail: {joined}"));
        }
    }
    out
}

/// Drive the MCP JSON-RPC handshake: initialize → initialized →
/// tools/list. Returns the tool names in the order the server returned.
fn handshake_and_list(
    stdin: &mut std::process::ChildStdin,
    stdout: std::process::ChildStdout,
) -> Result<Vec<String>, String> {
    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": super::MCP_CLIENT_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            },
        },
    });
    let initialized = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {},
    });
    let tools_list = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {},
    });

    write_frame(stdin, &initialize)?;
    let mut reader = BufReader::new(stdout);
    let _init_resp = read_response_with_id(&mut reader, 1)?;
    write_frame(stdin, &initialized)?;
    write_frame(stdin, &tools_list)?;
    let resp = read_response_with_id(&mut reader, 2)?;

    let tools = resp
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .ok_or_else(|| "tools/list response missing result.tools[]".to_string())?;

    let mut names = Vec::with_capacity(tools.len());
    for t in tools {
        if let Some(n) = t.get("name").and_then(|v| v.as_str()) {
            names.push(n.to_string());
        }
    }
    Ok(names)
}

/// Write one JSON-RPC frame (`{json}\n`) to the child's stdin.
fn write_frame<W: Write>(w: &mut W, value: &serde_json::Value) -> Result<(), String> {
    let s = serde_json::to_string(value).map_err(|e| format!("encode failed: {e}"))?;
    w.write_all(s.as_bytes())
        .map_err(|e| format!("stdin write failed: {e}"))?;
    w.write_all(b"\n")
        .map_err(|e| format!("stdin newline failed: {e}"))?;
    w.flush().map_err(|e| format!("stdin flush failed: {e}"))?;
    Ok(())
}

/// Read lines from the child's stdout until we find a JSON object with
/// `id == want_id`. Intermediate lines are skipped defensively.
fn read_response_with_id<R: BufRead>(
    reader: &mut R,
    want_id: u64,
) -> Result<serde_json::Value, String> {
    loop {
        let mut buf = String::new();
        let n = reader
            .read_line(&mut buf)
            .map_err(|e| format!("stdout read failed: {e}"))?;
        if n == 0 {
            return Err("child closed stdout before response arrived".into());
        }
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
            if id == want_id {
                return Ok(value);
            }
        }
    }
}

// ─── MCP integrations box ────────────────────────────────────────────────────

/// A1-001 (2026-05-04): MCP host integration status.
///
/// Captures whether each AI host has mneme registered in its MCP config
/// AND whether the host process is currently running. Distinct from
/// `probe_mcp_tools` (which only verifies this binary can serve tools to
/// itself) — this is the real "is anyone actually using mneme?" probe.
#[derive(Debug, Clone)]
struct McpHostStatus {
    host: &'static str,
    registry_path: std::path::PathBuf,
    registered: bool,
    /// Registered command path resolves to the current `mneme` binary?
    /// `None` when not registered.
    command_matches: Option<bool>,
    /// Host process found in the running process table?
    live_pid: Option<u32>,
    note: Option<String>,
}

/// A1-001: probe Claude Code's `~/.claude.json` for the `mcpServers.mneme`
/// entry and verify the command path matches the running mneme binary.
fn probe_mcp_claude_code_status() -> McpHostStatus {
    let registry_path = match dirs::home_dir() {
        Some(h) => h.join(".claude.json"),
        None => std::path::PathBuf::from(".claude.json"),
    };

    let mut status = McpHostStatus {
        host: "claude-code",
        registry_path: registry_path.clone(),
        registered: false,
        command_matches: None,
        live_pid: is_claude_code_running(),
        note: None,
    };

    let raw = match std::fs::read_to_string(&registry_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            status.note =
                Some("~/.claude.json missing -- Claude Code never installed an MCP entry".into());
            return status;
        }
        Err(e) => {
            status.note = Some(format!("read failed: {e}"));
            return status;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            status.note = Some(format!("parse failed: {e}"));
            return status;
        }
    };

    let mneme = parsed
        .get("mcpServers")
        .and_then(|v| v.get("mneme"))
        .and_then(|v| v.as_object());
    let mneme = match mneme {
        Some(m) => m,
        None => {
            status.note =
                Some("no mcpServers.mneme entry -- run `mneme install` to register".into());
            return status;
        }
    };

    status.registered = true;

    let current_exe = std::env::current_exe().ok();
    let registered_cmd = mneme
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if registered_cmd.is_empty() {
        status.command_matches = Some(false);
        status.note = Some("mcpServers.mneme.command field is empty".into());
    } else {
        let same = match &current_exe {
            Some(cur) => {
                let leaf_eq = |a: &std::path::Path, b: &std::path::Path| {
                    let af = a.file_name().map(|s| s.to_string_lossy().to_lowercase());
                    let bf = b.file_name().map(|s| s.to_string_lossy().to_lowercase());
                    af.is_some() && af == bf
                };
                let registered = std::path::PathBuf::from(registered_cmd);
                leaf_eq(&registered, cur) || registered_cmd.eq_ignore_ascii_case("mneme")
            }
            None => true, // can't compare; assume match rather than panic
        };
        status.command_matches = Some(same);
        if !same {
            status.note = Some(format!(
                "registered command {:?} doesn't match current binary {:?}",
                registered_cmd,
                current_exe
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ));
        }
    }

    status
}

/// A1-001 (2026-05-04): render the "MCP integrations" box — the answer
/// to "is any AI host actually using mneme right now?".
///
/// Distinct from `render_mcp_tool_probe_box` (the self-test). This box
/// reads the host's MCP registry file (~/.claude.json for Claude Code)
/// and surfaces three independent signals: registry entry present?
/// command path matches? host process running?
pub fn render_mcp_integrations_box() {
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ MCP integrations (clients actually wired to mneme)      │");
    println!("├─────────────────────────────────────────────────────────┤");

    let status = probe_mcp_claude_code_status();
    let glyph_reg = if status.registered { "✓" } else { "✗" };
    line(
        &format!("{glyph_reg} {} registered", status.host),
        &status.registry_path.display().to_string(),
    );
    if let Some(matches) = status.command_matches {
        let glyph_cmd = if matches { "✓" } else { "✗" };
        line(
            &format!("{glyph_cmd} {} command path", status.host),
            if matches {
                "matches current binary"
            } else {
                "MISMATCH — registered cmd != current_exe"
            },
        );
    }
    let live_msg = match status.live_pid {
        Some(pid) => format!("running (pid {pid})"),
        None => "host process not detected".to_string(),
    };
    line(&format!("• {} live", status.host), &live_msg);
    if let Some(note) = &status.note {
        line("note", note);
    }
    println!("└─────────────────────────────────────────────────────────┘");
}
