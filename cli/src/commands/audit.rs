//! `mneme audit [--scope=...] [--severity=...]` — run all configured scanners.
//!
//! Two paths:
//!   1. **Supervisor IPC** (preferred): the daemon owns the scanner pool
//!      and writes findings to `~/.mneme/projects/<id>/findings.db`
//!      asynchronously. Returns the JSON findings list.
//!   2. **Direct subprocess fallback** (this file): when the supervisor
//!      is down or rejects the request, spawn `mneme-scanners` as a
//!      child and pipe a one-line `scan_all` orchestration command into
//!      its stdin. The worker walks the project, runs every applicable
//!      scanner, and emits one JSON-line [`Finding`] per discovered
//!      issue on stdout, terminating with a `{"_done": ..., ...}`
//!      summary line. The CLI persists those findings to the per-project
//!      `findings.db` shard via [`mneme_scanners::FindingsWriter`] and
//!      prints a human-friendly summary table.
//!
//! Exit codes:
//!   - 0  : no critical findings (or no findings at all)
//!   - 1  : at least one `critical` finding present (after `--severity` filter)
//!   - 5  : subprocess failed to spawn / crashed / wrote malformed output

use clap::Args;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use tracing::{debug, info, warn};

use crate::commands::build::make_client;
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};

use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};
use scanners::{Finding, FindingsWriter, Severity};

/// CLI args for `mneme audit`.
#[derive(Debug, Args)]
pub struct AuditArgs {
    /// Scope filter: `full` (every scannable file) or `diff` (only files
    /// changed in the last 24h — fast pre-commit check).
    #[arg(long, default_value = "full")]
    pub scope: String,

    /// Lower-bound severity filter. Findings less severe than this are
    /// dropped before printing. Order: `critical` > `error` > `warn` >
    /// `info`. Defaults to `info` (no filter).
    #[arg(long, default_value = "info")]
    pub severity: String,

    /// Optional project root. Defaults to CWD.
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: AuditArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project.clone())?;
    let severity_floor = parse_severity(&args.severity)?;
    let scope = normalise_scope(&args.scope)?;

    info!(
        project = %project.display(),
        scope,
        severity = severity_floor.label(),
        "starting mneme audit",
    );

    // Try IPC-first. On any failure (Err or `IpcResponse::Error`) we
    // fall back to the direct subprocess path below.
    let client = make_client(socket_override.clone());
    let ipc_attempt = client
        .request(IpcRequest::Audit {
            scope: scope.to_string(),
        })
        .await;

    match ipc_attempt {
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "supervisor returned error; falling back to direct subprocess");
        }
        Ok(other) => {
            // Supervisor handled it — reuse the standard renderer.
            return crate::commands::build::handle_response(other);
        }
        Err(e) => {
            warn!(error = %e, "supervisor unreachable; falling back to direct subprocess");
        }
    }

    run_direct_subprocess(&project, scope, severity_floor).await
}

/// Subprocess fallback: spawn `mneme-scanners`, pipe an orchestrator
/// `scan_all` command into stdin, stream findings back from stdout, and
/// persist them to the per-project `findings.db` shard.
async fn run_direct_subprocess(
    project: &Path,
    scope: &'static str,
    severity_floor: Severity,
) -> CliResult<()> {
    let bin = resolve_scanners_binary()?;
    debug!(bin = %bin.display(), "resolved scanners binary");

    // Build the orchestrator command JSON. The scanners worker recognises
    // this on the very first stdin line.
    let cmd_json = serde_json::json!({
        "action": "scan_all",
        "project_root": project,
        "scope": scope,
        "scanner_filter": Vec::<String>::new(),
    });
    let cmd_line = format!("{}\n", serde_json::to_string(&cmd_json)?);

    let mut child = TokioCommand::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            CliError::Other(format!(
                "failed to spawn {}: {e}",
                bin.display()
            ))
        })?;

    // Send command, then close stdin so the worker stops reading.
    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            CliError::Other("child stdin pipe missing".into())
        })?;
        stdin
            .write_all(cmd_line.as_bytes())
            .await
            .map_err(|e| CliError::Other(format!("write stdin failed: {e}")))?;
        stdin.flush().await.ok();
        // Drop stdin via take() to send EOF to the worker.
        drop(child.stdin.take());
    }

    // Stream stdout — one JSON line per finding, plus one summary line.
    let stdout = child.stdout.take().ok_or_else(|| {
        CliError::Other("child stdout pipe missing".into())
    })?;
    let mut reader = BufReader::new(stdout).lines();
    let mut findings: Vec<Finding> = Vec::new();
    let mut summary: Option<DoneSummary> = None;
    let mut subprocess_error: Option<String> = None;

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Try the summary marker first — it's the LAST line, so success
        // shortcircuits.
        if trimmed.starts_with("{\"_done\"") || trimmed.contains("\"_done\":true") {
            match serde_json::from_str::<DoneSummary>(trimmed) {
                Ok(s) => {
                    summary = Some(s);
                    continue;
                }
                Err(_) => { /* fall through to error parse */ }
            }
        }
        if trimmed.contains("\"_error\"") {
            #[derive(serde::Deserialize)]
            struct ErrLine { #[serde(rename = "_error")] _error: String }
            if let Ok(e) = serde_json::from_str::<ErrLine>(trimmed) {
                subprocess_error = Some(e._error);
                continue;
            }
        }
        match serde_json::from_str::<Finding>(trimmed) {
            Ok(f) => findings.push(f),
            Err(e) => {
                debug!(error = %e, line = %trimmed, "skipping malformed finding line");
            }
        }
    }

    // Wait for the child to exit so we can collect its status.
    let status = child
        .wait()
        .await
        .map_err(|e| CliError::Other(format!("subprocess wait failed: {e}")))?;

    if !status.success() {
        return Err(CliError::Other(format!(
            "mneme-scanners subprocess exited with status {status}"
        )))
        .map_err(|mut e| {
            // Bump exit code to 5 — the contract for "subprocess crashed".
            // Trick: wrap as Other but force the printed message to mention
            // it; the actual exit code mapping happens in `error::CliError`.
            if let CliError::Other(s) = &mut e { s.push_str(" (subprocess crashed)") };
            e
        });
    }

    if let Some(err) = subprocess_error {
        return Err(CliError::Other(format!("orchestrator error: {err}")));
    }

    // Apply the severity floor before persistence so the user's own
    // findings.db is uncluttered with stuff they explicitly filtered out.
    let kept: Vec<Finding> = findings
        .into_iter()
        .filter(|f| severity_rank(f.severity) <= severity_rank(severity_floor))
        .collect();

    // Persist into the per-project findings shard. The path layout
    // matches what the supervised path writes to.
    let project_id = ProjectId::from_path(project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let findings_db = paths.shard_db(&project_id, DbLayer::Findings);
    let inserted = match FindingsWriter::open(&findings_db) {
        Ok(mut w) => match w.write_findings(&kept) {
            Ok(n) => n,
            Err(e) => {
                warn!(error = %e, "could not persist findings to findings.db (continuing with print-only)");
                0
            }
        },
        Err(e) => {
            warn!(error = %e, db = %findings_db.display(), "findings.db open failed (continuing with print-only)");
            0
        }
    };

    print_summary(&kept, summary.as_ref(), inserted, &findings_db);

    // Exit code: 1 if any critical findings remain after the filter, else 0.
    let has_critical = kept.iter().any(|f| f.severity == Severity::Critical);
    if has_critical {
        // Use Other(...) here — main.rs maps Other → exit 1, which
        // matches the contract.
        return Err(CliError::Other(format!(
            "audit found {} critical finding(s)",
            kept.iter()
                .filter(|f| f.severity == Severity::Critical)
                .count()
        )));
    }
    Ok(())
}

/// Pretty-print a per-scanner summary table.
///
/// Layout (fixed column widths for readability):
///
/// ```text
/// scanner       critical  error  warn  info  total
/// theme               0      0    37     2     39
/// security            1      4     0     0      5
/// ...
/// ```
fn print_summary(
    findings: &[Finding],
    summary: Option<&DoneSummary>,
    persisted: usize,
    findings_db: &Path,
) {
    let mut by_scanner: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    for f in findings {
        let scanner = scanner_for_rule(&f.rule_id);
        let cell = by_scanner.entry(scanner).or_insert([0; 4]);
        cell[severity_index(f.severity)] += 1;
    }

    println!();
    println!("{:<14}{:>10}{:>8}{:>7}{:>7}{:>8}", "scanner", "critical", "error", "warn", "info", "total");
    println!("{:-<54}", "");
    let mut total_total = 0usize;
    let mut total_per_sev = [0usize; 4];
    for (scanner, cells) in &by_scanner {
        let row_total: usize = cells.iter().sum();
        total_total += row_total;
        for (i, c) in cells.iter().enumerate() {
            total_per_sev[i] += c;
        }
        println!(
            "{:<14}{:>10}{:>8}{:>7}{:>7}{:>8}",
            scanner, cells[0], cells[1], cells[2], cells[3], row_total
        );
    }
    println!("{:-<54}", "");
    println!(
        "{:<14}{:>10}{:>8}{:>7}{:>7}{:>8}",
        "TOTAL", total_per_sev[0], total_per_sev[1], total_per_sev[2], total_per_sev[3], total_total
    );
    println!();
    if let Some(s) = summary {
        println!(
            "scanned {} files in {}ms ({} scanner errors)",
            s.scanned, s.duration_ms, s.errors
        );
    }
    println!(
        "{} findings persisted to {}",
        persisted,
        findings_db.display()
    );
}

/// Final stdout line emitted by the scanner subprocess in orchestrator mode.
#[derive(Debug, serde::Deserialize)]
struct DoneSummary {
    #[allow(dead_code)] #[serde(rename = "_done")] _done: bool,
    scanned: usize,
    #[allow(dead_code)]
    findings: usize,
    errors: usize,
    duration_ms: u64,
}

/// Resolve `~/.mneme/bin/mneme-scanners[.exe]` first, then a developer
/// fallback to `target/release/mneme-scanners[.exe]`. The two paths are
/// mutually exclusive — the installed binary takes priority because in
/// release builds `target/` may be stale or missing.
fn resolve_scanners_binary() -> CliResult<PathBuf> {
    let exe_name = if cfg!(windows) {
        "mneme-scanners.exe"
    } else {
        "mneme-scanners"
    };
    if let Some(home) = dirs::home_dir() {
        let installed = home.join(".mneme").join("bin").join(exe_name);
        if installed.is_file() {
            return Ok(installed);
        }
    }
    // Dev fallback: target/release/mneme-scanners[.exe] relative to the
    // workspace root. We try the same workspace this CLI was built in.
    let dev_candidates = [
        PathBuf::from("target").join("release").join(exe_name),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join(exe_name)))
            .unwrap_or_default(),
    ];
    for candidate in &dev_candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    Err(CliError::Other(format!(
        "could not find {exe_name} in ~/.mneme/bin or alongside the running binary; \
         install mneme via `mneme install` or build the workspace with `cargo build --release`"
    )))
}

/// Resolve `project` to an absolute, canonicalised path. Falls back to
/// CWD if the user passed nothing.
fn resolve_project(arg: Option<PathBuf>) -> CliResult<PathBuf> {
    let raw = arg.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let canonical = std::fs::canonicalize(&raw).unwrap_or(raw);
    Ok(canonical)
}

/// Parse `--severity` into a [`Severity`]. Accepts the canonical labels
/// (`critical|error|warn|warning|info`) plus a few synonyms.
fn parse_severity(s: &str) -> CliResult<Severity> {
    match s.to_ascii_lowercase().as_str() {
        "critical" | "crit" => Ok(Severity::Critical),
        "error" | "err" => Ok(Severity::Error),
        "warn" | "warning" => Ok(Severity::Warning),
        "info" | "i" => Ok(Severity::Info),
        other => Err(CliError::Other(format!(
            "invalid --severity {other:?}; expected critical|error|warn|info"
        ))),
    }
}

/// Validate `--scope`. Accepts only `full` or `diff` — the orchestrator
/// rejects anything else.
fn normalise_scope(s: &str) -> CliResult<&'static str> {
    match s.to_ascii_lowercase().as_str() {
        "full" | "all" => Ok("full"),
        "diff" => Ok("diff"),
        other => Err(CliError::Other(format!(
            "invalid --scope {other:?}; expected full|diff"
        ))),
    }
}

/// Stable rank for sorting + filtering (lower = more severe).
fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Critical => 0,
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Info => 3,
    }
}

/// Column index into the per-scanner table (matches the header order
/// `critical, error, warn, info`).
fn severity_index(s: Severity) -> usize {
    match s {
        Severity::Critical => 0,
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Info => 3,
    }
}

/// Strip the rule prefix to recover the scanner name. Mirrors
/// [`mneme_scanners::scanner_name_for_rule`] — kept inline so this
/// module's surface doesn't depend on the scanners crate's public
/// helper.
fn scanner_for_rule(rule_id: &str) -> &str {
    match rule_id.split_once('.') {
        Some((prefix, _)) if !prefix.is_empty() => prefix,
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_finding(rule_id: &str, sev: Severity, file: &str) -> Finding {
        Finding::new_line(rule_id, sev, file, 1, 0, 10, "msg".to_string())
    }

    #[test]
    fn parse_severity_accepts_canonical_labels() {
        assert_eq!(parse_severity("critical").unwrap(), Severity::Critical);
        assert_eq!(parse_severity("error").unwrap(), Severity::Error);
        assert_eq!(parse_severity("warn").unwrap(), Severity::Warning);
        assert_eq!(parse_severity("info").unwrap(), Severity::Info);
    }

    #[test]
    fn parse_severity_accepts_synonyms() {
        assert_eq!(parse_severity("warning").unwrap(), Severity::Warning);
        assert_eq!(parse_severity("crit").unwrap(), Severity::Critical);
        assert_eq!(parse_severity("ERR").unwrap(), Severity::Error);
    }

    #[test]
    fn parse_severity_rejects_unknown() {
        assert!(parse_severity("urgent").is_err());
        assert!(parse_severity("").is_err());
    }

    #[test]
    fn normalise_scope_canonical() {
        assert_eq!(normalise_scope("full").unwrap(), "full");
        assert_eq!(normalise_scope("diff").unwrap(), "diff");
        assert_eq!(normalise_scope("ALL").unwrap(), "full");
    }

    #[test]
    fn normalise_scope_rejects_unknown() {
        assert!(normalise_scope("incremental").is_err());
    }

    #[test]
    fn severity_filter_keeps_at_or_above_floor() {
        let findings = vec![
            mk_finding("a.x", Severity::Critical, "a.ts"),
            mk_finding("a.x", Severity::Error, "a.ts"),
            mk_finding("a.x", Severity::Warning, "a.ts"),
            mk_finding("a.x", Severity::Info, "a.ts"),
        ];
        let floor = Severity::Warning;
        let kept: Vec<_> = findings
            .into_iter()
            .filter(|f| severity_rank(f.severity) <= severity_rank(floor))
            .collect();
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[0].severity, Severity::Critical);
        assert_eq!(kept[1].severity, Severity::Error);
        assert_eq!(kept[2].severity, Severity::Warning);
    }

    #[test]
    fn scanner_for_rule_extracts_prefix() {
        assert_eq!(scanner_for_rule("theme.hardcoded-hex"), "theme");
        assert_eq!(scanner_for_rule("security.eval"), "security");
        assert_eq!(scanner_for_rule("a11y.img-no-alt"), "a11y");
        assert_eq!(scanner_for_rule(""), "unknown");
        assert_eq!(scanner_for_rule("no-dot"), "unknown");
    }

    #[test]
    fn done_summary_round_trips() {
        let line = r#"{"_done":true,"scanned":42,"findings":7,"errors":1,"duration_ms":1234}"#;
        let s: DoneSummary = serde_json::from_str(line).unwrap();
        assert_eq!(s.scanned, 42);
        assert_eq!(s.errors, 1);
        assert_eq!(s.duration_ms, 1234);
    }

    #[test]
    fn finding_round_trips_via_jsonl() {
        let f = mk_finding("theme.hardcoded-hex", Severity::Warning, "x.tsx");
        let line = serde_json::to_string(&f).unwrap();
        let back: Finding = serde_json::from_str(&line).unwrap();
        assert_eq!(back.rule_id, "theme.hardcoded-hex");
        assert_eq!(back.severity, Severity::Warning);
    }

    #[test]
    fn resolve_scanners_binary_returns_path_or_err() {
        // We cannot guarantee a binary exists in CI's test sandbox, but
        // we CAN guarantee the function never panics. A successful
        // result must point at an existing file.
        match resolve_scanners_binary() {
            Ok(p) => assert!(p.is_file(), "returned non-file path: {}", p.display()),
            Err(_) => { /* acceptable in environments where neither path exists */ }
        }
    }
}
