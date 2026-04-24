//! `mneme doctor` — health check / self-test.
//!
//! v0.3.1: human-readable summary first (closes F-006 from the
//! install-report — prior output was an unbounded raw-JSON dump),
//! optional `--json` for machine output. Diagnostics run in-process
//! (version, runtime/state dir writable) plus a live supervisor ping.

use clap::Args;
use std::path::PathBuf;
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
