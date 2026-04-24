//! `mneme doctor` — health check / self-test.
//!
//! v0.3.1: human-readable summary first (closes F-006 from the
//! install-report — prior output was an unbounded raw-JSON dump),
//! optional `--json` for machine output. Diagnostics run in-process
//! (version, runtime/state dir writable) plus a live supervisor ping.
//! v0.3.1+: per-MCP-tool probe — spawns a fresh `mneme mcp stdio`
//! child, runs a JSON-RPC `initialize` + `tools/list` handshake, and
//! reports a ✓ for every tool the MCP server actually exposes.

use clap::Args;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio};
use std::time::{Duration, Instant};
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::ipc::{IpcRequest, IpcResponse};

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
    /// `mneme mcp stdio` child to enumerate the live tool set). The
    /// probe is usually <2s on POS2 but can be skipped for a faster
    /// run in CI / automated scripts.
    #[arg(long)]
    pub skip_mcp_probe: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: DoctorArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // Banner — matches the MCP handshake banner so CLI + MCP feel
    // like one product. 64 chars wide inside the box.
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                                                              ║");
    println!("║   ███╗   ███╗███╗   ██╗███████╗███╗   ███╗███████╗           ║");
    println!("║   ████╗ ████║████╗  ██║██╔════╝████╗ ████║██╔════╝           ║");
    println!("║   ██╔████╔██║██╔██╗ ██║█████╗  ██╔████╔██║█████╗             ║");
    println!("║   ██║╚██╔╝██║██║╚██╗██║██╔══╝  ██║╚██╔╝██║██╔══╝             ║");
    println!("║   ██║ ╚═╝ ██║██║ ╚████║███████╗██║ ╚═╝ ██║███████╗           ║");
    println!("║   ╚═╝     ╚═╝╚═╝  ╚═══╝╚══════╝╚═╝     ╚═╝╚══════╝           ║");
    println!("║                                                              ║");
    println!(
        "║   persistent memory · code graph · drift detector · 47 tools ║"
    );
    println!(
        "║   v{:<8} · 100% local · Apache-2.0                         ║",
        env!("CARGO_PKG_VERSION")
    );
    println!("║                                                              ║");
    println!("║   © 2026 Anishbhai Trivedi & Kruti Trivedi                   ║");
    println!("║                                                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "  {:<16}{}",
        "timestamp:",
        utc_now_readable()
    );
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
        return Ok(());
    }

    let client = make_client(socket_override);
    let is_up = client.is_running().await;
    line(
        "supervisor",
        if is_up { "running ✓" } else { "NOT RUNNING ✗" },
    );
    if !is_up {
        println!("└─────────────────────────────────────────────────────────┘");
        println!();
        // Even without the supervisor, the MCP bridge + per-tool probe
        // are useful — the Bun MCP server spawns independently of the
        // Rust supervisor for `tools/list`.
        render_mcp_bridge_box();
        if !args.skip_mcp_probe {
            render_mcp_tool_probe_box();
        }
        println!();
        println!("start the daemon with:  mneme daemon start");
        return Ok(());
    }

    // Request per-child snapshot for the summary.
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
            match status {
                "running" | "healthy" => running += 1,
                "pending" | "starting" => pending += 1,
                "failed" | "crashed" => failed += 1,
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

        // Per-worker breakdown — one row per worker with status + pid +
        // uptime. Humans can tell which worker is failing at a glance.
        println!("┌─────────────────────────────────────────────────────────┐");
        println!("│ per-worker health                                       │");
        println!("├─────────────────────────────────────────────────────────┤");
        for child in children {
            let name = child
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let status = child
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
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
            let mark = match status {
                "running" | "healthy" => "✓",
                "pending" | "starting" => "…",
                "failed" | "crashed" => "✗",
                _ => "?",
            };
            let uptime_str = if uptime_ms > 0 {
                format!("{}s", uptime_ms / 1000)
            } else {
                "-".to_string()
            };
            line(
                &format!("{mark} {name}"),
                &format!(
                    "status={status:<9}  pid={pid:<6}  uptime={uptime_str:<6}  restarts={restarts}"
                ),
            );
        }
        println!("└─────────────────────────────────────────────────────────┘");

        // Per-binary health — does every expected mneme-* binary live
        // on disk next to `mneme.exe`?
        println!();
        println!("┌─────────────────────────────────────────────────────────┐");
        println!("│ binaries on disk                                        │");
        println!("├─────────────────────────────────────────────────────────┤");
        let bin_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        if let Some(dir) = bin_dir {
            for b in [
                "mneme.exe",
                "mneme-daemon.exe",
                "mneme-brain.exe",
                "mneme-parsers.exe",
                "mneme-scanners.exe",
                "mneme-livebus.exe",
                "mneme-md-ingest.exe",
                "mneme-store.exe",
                "mneme-multimodal.exe",
            ] {
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

        // MCP bridge health — does `~/.mneme/mcp/src/index.ts` exist?
        // Is `bun` on PATH?
        render_mcp_bridge_box();

        // Per-MCP-tool probe — spawn a fresh `mneme mcp stdio` child,
        // run the JSON-RPC handshake, and list every tool the server
        // actually exposes. Gated behind --skip-mcp-probe so CI / very
        // slow disks can opt out.
        if !args.skip_mcp_probe {
            render_mcp_tool_probe_box();
        }

        if args.json {
            println!();
            println!("raw status:");
            println!("{}", serde_json::to_string_pretty(&children)?);
        }
    } else {
        println!("└─────────────────────────────────────────────────────────┘");
        warn!(?resp, "supervisor returned non-status response");
    }
    Ok(())
}

/// Render the "MCP bridge" box (entry path + bun runtime). Split out
/// so we can emit it on both the supervisor-up and supervisor-down
/// paths without duplicating the box-drawing.
fn render_mcp_bridge_box() {
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ MCP bridge                                              │");
    println!("├─────────────────────────────────────────────────────────┤");
    let home = dirs::home_dir();
    let mcp_entry = home
        .as_ref()
        .map(|h| h.join(".mneme").join("mcp").join("src").join("index.ts"));
    let mcp_exists = mcp_entry
        .as_ref()
        .map(|p| p.exists())
        .unwrap_or(false);
    line(
        if mcp_exists { "✓ MCP entry" } else { "✗ MCP entry" },
        mcp_entry
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "?".into())
            .as_str(),
    );
    let bun_on_path = which_on_path("bun");
    line(
        if bun_on_path.is_some() {
            "✓ bun runtime"
        } else {
            "✗ bun runtime"
        },
        bun_on_path.as_deref().unwrap_or("not on PATH"),
    );
    println!("└─────────────────────────────────────────────────────────┘");
}

/// Render the "per-MCP-tool health" box — spawn a fresh mneme child,
/// enumerate tools via JSON-RPC, show one ✓ per live tool. Split out
/// so we can emit it on both the supervisor-up and supervisor-down
/// paths.
fn render_mcp_tool_probe_box() {
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ per-MCP-tool health                                     │");
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

/// Locate an executable on PATH. Returns the absolute path if found.
fn which_on_path(name: &str) -> Option<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    let path = std::env::var_os("PATH")?;
    let s = path.to_string_lossy();
    for dir in s.split(sep) {
        for ext in exts {
            let candidate = std::path::PathBuf::from(dir).join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate.display().to_string());
            }
        }
    }
    None
}

fn line(label: &str, value: &str) {
    let padded_label = format!("{label:<17}");
    let content = format!("│ {padded_label}: {value}");
    // Pad to width 59 (inside borders), then add right border.
    let visible_len = content.chars().count();
    let target = 59;
    let pad = if visible_len < target {
        " ".repeat(target - visible_len)
    } else {
        String::new()
    };
    println!("{content}{pad}│");
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
fn probe_mcp_tools(deadline: Duration) -> Result<Vec<String>, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("current_exe unavailable: {e}"))?;

    let mut child = StdCommand::new(&exe)
        .arg("mcp")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Silence the MCP server's own stderr banner / diagnostic
        // logs so nothing contaminates the probe; stderr is piped to
        // null anyway, but belt-and-braces for any SDK that reads env.
        .env("MNEME_LOG", "error")
        .env("NO_COLOR", "1")
        .spawn()
        .map_err(|e| format!("spawn failed: {e}"))?;

    let start = Instant::now();

    // Take ownership of stdin/stdout handles. If either is missing the
    // child is unusable — kill it and bail.
    let mut stdin = match child.stdin.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("no stdin pipe".into());
        }
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("no stdout pipe".into());
        }
    };

    // Run the actual JSON-RPC handshake on a worker thread so we can
    // enforce the deadline from this thread without blocking forever
    // on a stuck `read_line`.
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<String>, String>>();

    // Handshake thread — owns stdout reader, writes to stdin via the
    // handle it captures, posts result to the channel.
    std::thread::spawn(move || {
        let res = handshake_and_list(&mut stdin, stdout);
        let _ = tx.send(res);
    });

    // Wait for the worker to finish, bounded by `deadline`.
    let remaining = deadline.saturating_sub(start.elapsed());
    let result = match rx.recv_timeout(remaining) {
        Ok(res) => res,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(format!("timed out after {}s", deadline.as_secs()))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err("handshake thread died".into())
        }
    };

    // Always kill the child and reap it before returning.
    let _ = child.kill();
    let _ = child.wait();

    result
}

/// Drive the MCP JSON-RPC handshake: initialize → initialized →
/// tools/list. Returns the tool names in the order the server
/// returned them.
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
                "name": "mneme-doctor",
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

    // Write initialize, then wait for its response.
    write_frame(stdin, &initialize)?;

    let mut reader = BufReader::new(stdout);

    // Consume the initialize response (match id == 1). The server may
    // interleave log lines on stdout in some transports, but the MCP
    // SDK uses pure JSON-RPC framing on stdio — one JSON object per
    // line — so we just read until we see id == 1.
    let _init_resp = read_response_with_id(&mut reader, 1)?;

    // Tell the server initialization is complete.
    write_frame(stdin, &initialized)?;

    // Ask for the tool list.
    write_frame(stdin, &tools_list)?;

    // Read until we find the response with id == 2.
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
    let s = serde_json::to_string(value)
        .map_err(|e| format!("encode failed: {e}"))?;
    w.write_all(s.as_bytes())
        .map_err(|e| format!("stdin write failed: {e}"))?;
    w.write_all(b"\n")
        .map_err(|e| format!("stdin newline failed: {e}"))?;
    w.flush()
        .map_err(|e| format!("stdin flush failed: {e}"))?;
    Ok(())
}

/// Read lines from the child's stdout until we find a JSON object
/// with `id == want_id`. Intermediate lines (other responses,
/// notifications, blank lines) are skipped.
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
            Err(_) => continue, // Skip non-JSON lines defensively.
        };
        if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
            if id == want_id {
                return Ok(value);
            }
        }
    }
}

fn is_writable(path: &std::path::Path) -> bool {
    std::fs::create_dir_all(path)
        .and_then(|_| {
            let probe = path.join(".mneme-probe");
            std::fs::write(&probe, b"")?;
            std::fs::remove_file(&probe)
        })
        .is_ok()
}

/// `YYYY-MM-DD HH:MM:SS UTC` without pulling chrono.
fn utc_now_readable() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let s = secs % 86_400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    let (y, m, d) = ymd(days);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}
fn ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z / 146_097 } else { (z - 146_096) / 146_097 };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
