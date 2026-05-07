//! `mneme status` — fast colorful health snapshot.
//!
//! Like `mneme doctor` but compact, instant, and built for at-a-glance
//! reading. Probes every wire mneme depends on (daemon socket, HTTP
//! server, IPC pipe, MCP integration, models, projects, hooks) and
//! prints a single screen with green/yellow/red status icons.
//!
//! When the supervisor is reachable we delegate the per-project shard
//! counts (it has the freshest snapshots). When it is NOT reachable we
//! fall back to a direct read of `meta.db::projects` so `mneme status`
//! still tells the user something useful — same fallback shape as
//! `history.rs` and the v0.4.0 status path.
//!
//! Anish 2026-05-06: "mneme doctor and mneme status, also both should
//! look colorful in terminal by command". This is the colorful one.

use clap::Args;
use console::{style, Term};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::commands::build::{handle_response, make_client};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::paths::PathManager;

/// CLI args for `mneme status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Optional project path. Defaults to CWD.
    pub project: Option<PathBuf>,

    /// Skip the colorful UI and emit a single-line machine-friendly
    /// summary (`ok|degraded|down: N projects`). For shell scripts and
    /// CI; humans want the default colorful path.
    #[arg(long)]
    pub plain: bool,

    /// Forward to the supervisor for the rich per-project numbers
    /// (default behaviour). Pass `--quick` to skip the daemon round-trip
    /// when you only want the local probes (~10ms instead of ~100ms).
    #[arg(long)]
    pub quick: bool,
}

/// Single probe-result row. A wire status header gets one of these per
/// probe; the renderer maps `Verdict` -> icon + colour.
struct Probe {
    label: &'static str,
    verdict: Verdict,
    detail: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Ok,
    Warn,
    Fail,
    Skip,
}

impl Verdict {
    fn icon(self) -> &'static str {
        match self {
            Verdict::Ok => "✓",
            Verdict::Warn => "!",
            Verdict::Fail => "✗",
            Verdict::Skip => "·",
        }
    }
}

/// Entry point used by `main.rs`.
pub async fn run(args: StatusArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    if args.plain {
        return run_plain(args, socket_override).await;
    }

    let started = Instant::now();
    let paths = PathManager::default_root();
    let mut probes: Vec<Probe> = Vec::with_capacity(8);

    // 1. ~/.mneme home directory (every other check assumes this exists)
    probes.push(probe_home_dir(&paths));

    // 2. Daemon socket (named pipe on Windows, unix domain on POSIX).
    // discover_socket_path honours MNEME_SUPERVISOR_SOCKET first, then
    // falls back to the canonical default — matches the rest of the CLI.
    let daemon_socket = common::worker_ipc::discover_socket_path()
        .unwrap_or_else(|| paths.root().join("daemon.sock"));
    probes.push(probe_daemon_socket(&daemon_socket).await);

    // 3. HTTP server on 127.0.0.1:7777 — only if socket is up
    let daemon_alive = matches!(probes.last().map(|p| p.verdict), Some(Verdict::Ok));
    probes.push(probe_http_health(daemon_alive).await);

    // 4. IPC roundtrip (Status request + reply within 1s)
    let socket_for_ipc = socket_override.clone();
    probes.push(probe_ipc_roundtrip(daemon_alive, socket_for_ipc).await);

    // 5. MCP — claude.json + mneme registered
    probes.push(probe_mcp_registration(&paths));

    // 6. Models — BGE + tokenizer present
    probes.push(probe_models(&paths));

    // 7. Hooks — settings.json mneme-hook entries
    probes.push(probe_hooks(&paths));

    // 8. Projects — count + freshness from meta.db
    let (proj_probe, project_count, mneme_home_for_render) = probe_projects(&paths);
    probes.push(proj_probe);

    render_dashboard(&probes, project_count, &mneme_home_for_render, started);

    // 9. Optional rich per-project numbers via supervisor (skipped on --quick)
    if !args.quick {
        if let Err(e) = render_supervisor_detail(&args, socket_override).await {
            eprintln!("  [warn] supervisor detail unavailable: {e}");
        }
    }

    Ok(())
}

async fn run_plain(args: StatusArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let attempt = client
        .request(IpcRequest::Status {
            project: args.project.clone(),
        })
        .await;

    match attempt {
        Ok(IpcResponse::Error { message }) => {
            tracing::warn!(
                error = %message,
                "supervisor returned error on status; falling back to direct-db"
            );
        }
        Ok(resp) => return handle_response(resp),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "supervisor unreachable on status; falling back to direct-db"
            );
        }
    }
    direct_db_fallback_plain()
}

/// Render the dashboard header + probe list + footer with the same
/// colour palette across light + dark terminals (no hardcoded
/// foregrounds — `console::style` reads the user's terminal theme).
fn render_dashboard(probes: &[Probe], project_count: usize, home: &Path, started: Instant) {
    let term = Term::stdout();
    let _ = term.clear_screen();

    // Banner
    let title = style(" mneme status ").bold().on_blue().white();
    let version = style(format!(" v{} ", env!("CARGO_PKG_VERSION")))
        .dim()
        .italic();
    println!();
    println!("{title}{version}");
    println!();

    let label_width = probes.iter().map(|p| p.label.len()).max().unwrap_or(12);

    for probe in probes {
        let icon = match probe.verdict {
            Verdict::Ok => style(probe.verdict.icon()).green().bold(),
            Verdict::Warn => style(probe.verdict.icon()).yellow().bold(),
            Verdict::Fail => style(probe.verdict.icon()).red().bold(),
            Verdict::Skip => style(probe.verdict.icon()).dim(),
        };
        let label = style(format!("{:<width$}", probe.label, width = label_width)).cyan();
        let detail = match probe.verdict {
            Verdict::Ok => style(&probe.detail).green(),
            Verdict::Warn => style(&probe.detail).yellow(),
            Verdict::Fail => style(&probe.detail).red(),
            Verdict::Skip => style(&probe.detail).dim(),
        };
        println!("  {icon}  {label}  {detail}");
    }

    // Footer
    let elapsed_ms = started.elapsed().as_millis();
    println!();
    let summary = match overall(probes) {
        Verdict::Ok => style(format!(" all {} wires healthy ", probes.len()))
            .bold()
            .on_green()
            .black(),
        Verdict::Warn => style(" some wires degraded ".to_string())
            .bold()
            .on_yellow()
            .black(),
        Verdict::Fail => style(" wires DOWN ".to_string()).bold().on_red().white(),
        Verdict::Skip => style(" status mixed ".to_string()).bold().on_blue().white(),
    };
    let timing = style(format!("({} ms)", elapsed_ms)).dim();
    let projects = style(format!("· {} project(s)", project_count)).dim();
    let home_hint = style(format!("· {}", home.display())).dim();
    println!("  {summary}  {timing}  {projects}  {home_hint}");
    println!();
}

fn overall(probes: &[Probe]) -> Verdict {
    if probes.iter().any(|p| p.verdict == Verdict::Fail) {
        Verdict::Fail
    } else if probes.iter().any(|p| p.verdict == Verdict::Warn) {
        Verdict::Warn
    } else if probes.iter().all(|p| p.verdict == Verdict::Ok) {
        Verdict::Ok
    } else {
        Verdict::Skip
    }
}

// ------------------------------------------------------------------
// Probes
// ------------------------------------------------------------------

fn probe_home_dir(paths: &PathManager) -> Probe {
    let home = paths.root();
    if home.is_dir() {
        Probe {
            label: "home dir",
            verdict: Verdict::Ok,
            detail: home.display().to_string(),
        }
    } else {
        Probe {
            label: "home dir",
            verdict: Verdict::Fail,
            detail: format!(
                "missing: {} — run `mneme build .` or set MNEME_HOME",
                home.display()
            ),
        }
    }
}

async fn probe_daemon_socket(socket: &Path) -> Probe {
    if !socket_exists(socket) {
        return Probe {
            label: "daemon socket",
            verdict: Verdict::Warn,
            detail: format!("not running ({})", socket.display()),
        };
    }
    Probe {
        label: "daemon socket",
        verdict: Verdict::Ok,
        detail: format!("up ({})", socket.display()),
    }
}

#[cfg(windows)]
fn socket_exists(_socket: &Path) -> bool {
    // 2026-05-07 fix: the prior implementation looked for `daemon.pid`
    // in `socket.parent()`. Windows named pipes have paths like
    // `\\.\pipe\mneme-supervisor`, and `.parent()` returns the
    // pipe-namespace `\\.\pipe` which is NOT a real filesystem
    // directory — so the `daemon.pid` probe always returned false and
    // status reported the daemon as down even when doctor reported it
    // running with 10/10 workers and 70s uptime (VM round 2 bug).
    //
    // Fix: route through the same PID-liveness check doctor uses
    // (~/.mneme/run/daemon.pid + sysinfo refresh + name filter for
    // Windows PID reuse). When the file is missing or the PID is
    // dead, status reports "not running"; when alive, "up". Now
    // status and doctor agree on the daemon's liveness verdict.
    use crate::commands::doctor::{check_daemon_pid_liveness, DaemonPidState};
    let root = common::paths::PathManager::default_root()
        .root()
        .to_path_buf();
    matches!(
        check_daemon_pid_liveness(&root),
        DaemonPidState::AliveProbeFresh
    )
}

#[cfg(unix)]
fn socket_exists(socket: &Path) -> bool {
    socket.exists()
}

async fn probe_http_health(daemon_alive: bool) -> Probe {
    if !daemon_alive {
        return Probe {
            label: "http :7777",
            verdict: Verdict::Skip,
            detail: "daemon down → skipped".to_string(),
        };
    }
    let probe = async {
        let resp = reqwest::Client::builder()
            .timeout(Duration::from_millis(800))
            .build()
            .ok()?
            .get("http://127.0.0.1:7777/api/health")
            .send()
            .await
            .ok()?;
        if resp.status().is_success() {
            Some(resp.status().as_u16())
        } else {
            None
        }
    };
    match timeout(Duration::from_millis(1000), probe).await {
        Ok(Some(_)) => Probe {
            label: "http :7777",
            verdict: Verdict::Ok,
            detail: "200 OK".into(),
        },
        Ok(None) => Probe {
            label: "http :7777",
            verdict: Verdict::Fail,
            detail: "non-2xx response".into(),
        },
        Err(_) => Probe {
            label: "http :7777",
            verdict: Verdict::Fail,
            detail: "timeout (>1s)".into(),
        },
    }
}

async fn probe_ipc_roundtrip(daemon_alive: bool, socket_override: Option<PathBuf>) -> Probe {
    if !daemon_alive {
        return Probe {
            label: "ipc roundtrip",
            verdict: Verdict::Skip,
            detail: "daemon down → skipped".to_string(),
        };
    }
    let client = make_client(socket_override);
    let started = Instant::now();
    match timeout(
        Duration::from_millis(1200),
        client.request(IpcRequest::Status { project: None }),
    )
    .await
    {
        Ok(Ok(_resp)) => Probe {
            label: "ipc roundtrip",
            verdict: Verdict::Ok,
            detail: format!("ok ({} ms)", started.elapsed().as_millis()),
        },
        Ok(Err(e)) => Probe {
            label: "ipc roundtrip",
            verdict: Verdict::Fail,
            detail: format!("error: {e}"),
        },
        Err(_) => Probe {
            label: "ipc roundtrip",
            verdict: Verdict::Fail,
            detail: "timeout (>1.2s)".into(),
        },
    }
}

fn probe_mcp_registration(paths: &PathManager) -> Probe {
    // 2026-05-07 fix: Claude Code's primary settings file is at
    // `~/.claude.json` (a FILE in $HOME, NOT inside the `.claude/`
    // directory). The prior probe looked at `~/.claude/claude.json`
    // — a path that doesn't exist on stock Claude Code installs —
    // so the probe always reported "not found" and told users to run
    // `mneme register-mcp` even when `claude mcp list` showed mneme
    // connected. Same lying-status pattern as the daemon liveness
    // bug (commit 3d7c2f7) and the tokenizer.json probe (commit
    // de045ff). We check both locations: `~/.claude.json` (current,
    // primary) and `~/.claude/settings.json` (where hooks live and
    // where MCP servers may also be declared depending on Claude
    // Code version). Either holding `"mneme"` = OK.
    let _ = paths; // unused; keeps the same signature shape as other probes
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => {
            return Probe {
                label: "mcp",
                verdict: Verdict::Warn,
                detail: "$HOME unset — can't locate Claude Code config".into(),
            };
        }
    };
    let candidates = [
        home.join(".claude.json"),
        home.join(".claude").join("settings.json"),
    ];
    let mut any_exists = false;
    for path in &candidates {
        if !path.exists() {
            continue;
        }
        any_exists = true;
        let content = std::fs::read_to_string(path).unwrap_or_default();
        if content.contains("\"mneme\"") {
            return Probe {
                label: "mcp",
                verdict: Verdict::Ok,
                detail: format!(
                    "mneme registered in {}",
                    path.file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
            };
        }
    }
    if !any_exists {
        Probe {
            label: "mcp",
            verdict: Verdict::Warn,
            detail: "Claude Code config not found (run `mneme register-mcp`)".into(),
        }
    } else {
        Probe {
            label: "mcp",
            verdict: Verdict::Warn,
            detail: "mneme NOT registered (run `mneme register-mcp`)".into(),
        }
    }
}

fn probe_models(paths: &PathManager) -> Probe {
    let models_dir = paths.root().join("models");
    if !models_dir.is_dir() {
        return Probe {
            label: "models",
            verdict: Verdict::Warn,
            detail: format!("missing: {}", models_dir.display()),
        };
    }
    // 2026-05-07 fix: probe BOTH possible layouts. The flat layout
    // (`models/bge-small-en-v1.5.onnx` + `models/tokenizer.json`) is the
    // one `brain::Embedder::from_default_path` actually loads from
    // (brain/src/embeddings.rs::from_default_path) and the one the
    // public install scripts produce. The nested layout
    // (`models/bge-small-en-v1.5/{model.onnx,tokenizer.json}`) is the
    // legacy v0.3 layout still on some user machines.
    //
    // The prior code checked the .onnx file in EITHER layout but only
    // checked tokenizer.json in the NESTED layout — so on a fresh
    // install with the flat layout it falsely reported
    // "BGE present but tokenizer.json missing" while the embedder was
    // happily loading the same file.
    let bge_present = models_dir.join("bge-small-en-v1.5").is_dir()
        || models_dir.join("bge-small-en-v1.5.onnx").is_file();
    let tokenizer_present = models_dir.join("tokenizer.json").is_file()
        || models_dir
            .join("bge-small-en-v1.5")
            .join("tokenizer.json")
            .is_file();
    if bge_present && tokenizer_present {
        Probe {
            label: "models",
            verdict: Verdict::Ok,
            detail: "bge-small-en-v1.5 + tokenizer present".into(),
        }
    } else if bge_present {
        Probe {
            label: "models",
            verdict: Verdict::Warn,
            detail: "BGE present but tokenizer.json missing".into(),
        }
    } else {
        Probe {
            label: "models",
            verdict: Verdict::Warn,
            detail: "BGE model NOT downloaded — run `mneme install-models`".into(),
        }
    }
}

fn probe_hooks(_paths: &PathManager) -> Probe {
    let settings = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("settings.json"),
        None => {
            return Probe {
                label: "hooks",
                verdict: Verdict::Warn,
                detail: "$HOME unset — can't locate ~/.claude/settings.json".into(),
            }
        }
    };
    if !settings.exists() {
        return Probe {
            label: "hooks",
            verdict: Verdict::Warn,
            detail: "~/.claude/settings.json not found".into(),
        };
    }
    let content = std::fs::read_to_string(&settings).unwrap_or_default();
    if content.contains("mneme-hook") {
        Probe {
            label: "hooks",
            verdict: Verdict::Ok,
            detail: "mneme-hook wired in settings.json".into(),
        }
    } else {
        Probe {
            label: "hooks",
            verdict: Verdict::Warn,
            detail: "mneme-hook NOT wired (run `mneme install`)".into(),
        }
    }
}

fn probe_projects(paths: &PathManager) -> (Probe, usize, PathBuf) {
    let meta_db = paths.meta_db();
    let mneme_home = paths.root().to_path_buf();
    if !meta_db.exists() {
        return (
            Probe {
                label: "projects",
                verdict: Verdict::Warn,
                detail: "no meta.db (no projects built yet)".into(),
            },
            0,
            mneme_home,
        );
    }
    let conn = match Connection::open_with_flags(
        &meta_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            return (
                Probe {
                    label: "projects",
                    verdict: Verdict::Fail,
                    detail: format!("open meta.db: {e}"),
                },
                0,
                mneme_home,
            );
        }
    };
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap_or(0);
    let count_usize = count.max(0) as usize;
    let verdict = if count > 0 {
        Verdict::Ok
    } else {
        Verdict::Warn
    };
    let detail = if count > 0 {
        format!("{count} project(s) tracked in meta.db")
    } else {
        "meta.db exists but no projects yet".into()
    };
    (
        Probe {
            label: "projects",
            verdict,
            detail,
        },
        count_usize,
        mneme_home,
    )
}

// ------------------------------------------------------------------
// Supervisor-rich detail (only when daemon reachable AND --quick OFF)
// ------------------------------------------------------------------

async fn render_supervisor_detail(
    args: &StatusArgs,
    socket_override: Option<PathBuf>,
) -> CliResult<()> {
    let client = make_client(socket_override);
    let resp = match timeout(
        Duration::from_millis(800),
        client.request(IpcRequest::Status {
            project: args.project.clone(),
        }),
    )
    .await
    {
        Ok(Ok(r)) => r,
        _ => return Ok(()), // already shown via probes; skip detail
    };
    match resp {
        IpcResponse::Status { children } => {
            // Pretty-print the per-child snapshots the supervisor returned.
            // The dashboard already showed the rolled-up health; this is
            // the per-shard detail block for users who care about
            // node/edge counts and worker liveness.
            println!(
                "  {} per-child detail from supervisor ({} workers):",
                style("·").dim(),
                children.len()
            );
            // Bug #37 (2026-05-07): pretty-print one line per worker
            // instead of dumping raw serde JSON. Format matches the doctor
            // per-worker box but trimmed to one line each so `mneme status`
            // stays brief — full detail still lives in `mneme doctor`.
            // CLI receives children as Vec<serde_json::Value> so we extract
            // fields with .get().and_then() — supervisor's ChildSnapshot
            // type isn't linked into the CLI crate by design.
            // Show the first 8 to keep the screen readable.
            for snap in children.iter().take(8) {
                let name = snap.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let status_str = snap.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                let pid = snap.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                let uptime_ms = snap
                    .get("current_uptime_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let restarts = snap
                    .get("restart_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mark = if status_str == "running" {
                    style("✓").green()
                } else {
                    style("!").yellow()
                };
                let uptime_s = uptime_ms / 1000;
                let uptime_pretty = if uptime_s >= 3600 {
                    format!("{}h {}m", uptime_s / 3600, (uptime_s % 3600) / 60)
                } else if uptime_s >= 60 {
                    format!("{}m {}s", uptime_s / 60, uptime_s % 60)
                } else {
                    format!("{uptime_s}s")
                };
                println!(
                    "    {} {:<22} pid={:<6} up {:<10} restarts={}",
                    mark,
                    style(name).bold(),
                    pid,
                    uptime_pretty,
                    restarts
                );
            }
            if children.len() > 8 {
                println!(
                    "    {} (+{} more — run `mneme doctor` for full)",
                    style("·").dim(),
                    children.len() - 8
                );
            }
            println!();
        }
        IpcResponse::Error { message } => {
            println!(
                "  {} supervisor returned error: {message}",
                style("!").yellow()
            );
        }
        _ => {}
    }
    Ok(())
}

// ------------------------------------------------------------------
// Plain-text fallback (used by `--plain` and as a fallback when stdout
// isn't a TTY — the colour escape sequences would just litter logs).
// ------------------------------------------------------------------

fn direct_db_fallback_plain() -> CliResult<()> {
    let paths = PathManager::default_root();
    let meta_db = paths.meta_db();
    if !meta_db.exists() {
        println!("status: supervisor unreachable + meta.db not found");
        println!("  (no projects have been built yet — run `mneme build .`)");
        return Ok(());
    }

    let conn = Connection::open_with_flags(
        &meta_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", meta_db.display())))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, root, last_indexed_at FROM projects ORDER BY last_indexed_at DESC NULLS LAST",
        )
        .map_err(|e| CliError::Other(format!("prep status: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| CliError::Other(format!("exec status: {e}")))?;

    let mut shown = 0usize;
    println!(
        "status: supervisor unreachable — direct-db summary from {}:",
        meta_db.display()
    );
    println!();
    for (id, name, root, last_indexed_at) in rows.flatten() {
        shown += 1;
        let id_short: String = id.chars().take(12).collect();
        println!(
            "  [{id_short}] {}",
            name.unwrap_or_else(|| "<unnamed>".into())
        );
        if let Some(p) = root {
            println!("      root: {p}");
        }
        println!(
            "      last_indexed_at: {}",
            last_indexed_at.unwrap_or_else(|| "<never>".into())
        );
        println!();
    }
    if shown == 0 {
        println!("  (no projects in meta.db)");
    } else {
        println!("{shown} project(s) tracked.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Smoke clap harness — verify args parser without spinning up the
    /// full binary.
    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: StatusArgs,
    }

    #[tokio::test]
    async fn status_with_no_supervisor_falls_through_cleanly_plain() {
        // --plain path: should always finish Ok regardless of daemon
        // state because direct_db_fallback handles the missing-meta-db
        // case explicitly.
        let args = StatusArgs {
            project: None,
            plain: true,
            quick: false,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme-supervisor.sock"))).await;
        assert!(r.is_ok(), "expected Ok from fallback path, got: {r:?}");
    }

    #[test]
    fn status_args_parse_with_no_args() {
        // `mneme status` with no args — project field defaults to None
        // (resolved at runtime to CWD by PathManager).
        let h = Harness::try_parse_from(["x"]).unwrap();
        assert!(h.args.project.is_none());
        assert!(!h.args.plain);
        assert!(!h.args.quick);
    }

    #[test]
    fn status_args_parse_with_explicit_project() {
        // `mneme status /tmp/proj` — positional argument is captured.
        let h = Harness::try_parse_from(["x", "/tmp/proj"]).unwrap();
        assert!(h.args.project.is_some());
        assert_eq!(
            h.args.project.as_ref().unwrap(),
            &PathBuf::from("/tmp/proj")
        );
    }

    #[test]
    fn status_args_plain_flag_parses() {
        let h = Harness::try_parse_from(["x", "--plain"]).unwrap();
        assert!(h.args.plain);
    }

    #[test]
    fn status_args_quick_flag_parses() {
        let h = Harness::try_parse_from(["x", "--quick"]).unwrap();
        assert!(h.args.quick);
    }

    #[test]
    fn verdict_icons_have_distinct_chars() {
        // The dashboard relies on each verdict mapping to a different
        // glyph so colour-blind users can still distinguish them by
        // shape. Regression-guard the mapping.
        assert_ne!(Verdict::Ok.icon(), Verdict::Warn.icon());
        assert_ne!(Verdict::Ok.icon(), Verdict::Fail.icon());
        assert_ne!(Verdict::Warn.icon(), Verdict::Fail.icon());
        assert_ne!(Verdict::Ok.icon(), Verdict::Skip.icon());
    }

    #[test]
    fn overall_is_fail_when_any_probe_fails() {
        let probes = vec![
            Probe {
                label: "a",
                verdict: Verdict::Ok,
                detail: "x".into(),
            },
            Probe {
                label: "b",
                verdict: Verdict::Fail,
                detail: "y".into(),
            },
        ];
        assert_eq!(overall(&probes), Verdict::Fail);
    }

    #[test]
    fn overall_is_ok_when_all_probes_ok() {
        let probes = vec![
            Probe {
                label: "a",
                verdict: Verdict::Ok,
                detail: "x".into(),
            },
            Probe {
                label: "b",
                verdict: Verdict::Ok,
                detail: "y".into(),
            },
        ];
        assert_eq!(overall(&probes), Verdict::Ok);
    }

    #[test]
    fn overall_is_warn_when_any_warn_no_fail() {
        let probes = vec![
            Probe {
                label: "a",
                verdict: Verdict::Ok,
                detail: "x".into(),
            },
            Probe {
                label: "b",
                verdict: Verdict::Warn,
                detail: "y".into(),
            },
        ];
        assert_eq!(overall(&probes), Verdict::Warn);
    }
}

// PartialEq for Verdict so tests can assert_eq! on it.
impl std::fmt::Debug for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Verdict::Ok => "Ok",
            Verdict::Warn => "Warn",
            Verdict::Fail => "Fail",
            Verdict::Skip => "Skip",
        })
    }
}
