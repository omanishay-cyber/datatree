//! `mneme drift [--severity=...]` — current drift findings.
//!
//! ## REG-026: direct-DB fallback
//!
//! Mirrors the `audit.rs` pattern: try the supervisor first; if it's
//! unreachable OR returns an `Error` response, fall back to opening
//! `findings.db` directly via rusqlite and emitting findings as JSON
//! lines on stdout (one finding per line, terminated by a `_done`
//! summary marker — same shape as the audit subprocess output).
//!
//! Severity filter is honored on either path. When the daemon is up
//! the supervisor handles it; on the fallback we filter in SQL.

use clap::Args;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};
use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};

/// CLI args for `mneme drift`.
#[derive(Debug, Args)]
pub struct DriftArgs {
    /// Severity filter: `info` | `warn` | `error` | `critical`.
    #[arg(long)]
    pub severity: Option<String>,

    /// Optional project root (used by the direct-DB fallback). Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: DriftArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let client = make_client(socket_override);
    let attempt = client
        .request(IpcRequest::Drift {
            severity: args.severity.clone(),
        })
        .await;

    match attempt {
        Ok(IpcResponse::Error { message }) => {
            tracing::warn!(
                error = %message,
                "supervisor returned error on drift; falling back to direct-db"
            );
        }
        Ok(resp) => return handle_response(resp),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "supervisor unreachable on drift; falling back to direct-db"
            );
        }
    }

    direct_db_fallback(&args)
}

fn direct_db_fallback(args: &DriftArgs) -> CliResult<()> {
    let project = args
        .project
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let project = std::fs::canonicalize(&project).unwrap_or(project);
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let findings_db = paths.shard_db(&project_id, DbLayer::Findings);

    if !findings_db.exists() {
        // Print the same "_done" summary marker so downstream JSON-line
        // consumers see a consistent envelope on every path.
        println!(
            r#"{{"_done":true,"open":0,"note":"findings.db not found at {}"}}"#,
            findings_db.display().to_string().replace('"', r#"\""#)
        );
        return Ok(());
    }

    let conn = Connection::open_with_flags(
        &findings_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| CliError::Other(format!("open {}: {e}", findings_db.display())))?;

    // Severity filter: SQL-side so we don't ship rows we'd just drop.
    let filter = match args.severity.as_deref() {
        Some(s) => normalise_severity(s)?,
        None => None,
    };

    // Open findings only — `resolved_at IS NULL`. Same predicate the
    // supervisor's `drift_findings` MCP tool uses.
    let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match filter {
        Some(sev) => (
            "SELECT rule_id, scanner, severity, file, line_start, line_end,
                    column_start, column_end, message, suggestion
             FROM findings
             WHERE resolved_at IS NULL AND severity = ?1
             ORDER BY id DESC
             LIMIT 1000",
            vec![Box::new(sev)],
        ),
        None => (
            "SELECT rule_id, scanner, severity, file, line_start, line_end,
                    column_start, column_end, message, suggestion
             FROM findings
             WHERE resolved_at IS NULL
             ORDER BY id DESC
             LIMIT 1000",
            vec![],
        ),
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| CliError::Other(format!("prep drift: {e}")))?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())), |row| {
            let rule_id: String = row.get(0)?;
            let scanner: String = row.get(1)?;
            let severity: String = row.get(2)?;
            let file: String = row.get(3)?;
            let line_start: i64 = row.get(4)?;
            let line_end: i64 = row.get(5)?;
            let column_start: i64 = row.get(6)?;
            let column_end: i64 = row.get(7)?;
            let message: String = row.get(8)?;
            let suggestion: Option<String> = row.get(9)?;
            Ok(serde_json::json!({
                "rule_id": rule_id,
                "scanner": scanner,
                "severity": severity,
                "file": file,
                "line_start": line_start,
                "line_end": line_end,
                "column_start": column_start,
                "column_end": column_end,
                "message": message,
                "suggestion": suggestion,
            }))
        })
        .map_err(|e| CliError::Other(format!("exec drift: {e}")))?;

    let mut count = 0usize;
    for v in rows.flatten() {
        println!("{v}");
        count += 1;
    }

    println!(r#"{{"_done":true,"open":{count}}}"#);
    Ok(())
}

/// Validate a `--severity` argument and return the canonical label the
/// findings table stores. Rejects unknown values with a clear error.
fn normalise_severity(s: &str) -> CliResult<Option<String>> {
    let canonical = match s.to_ascii_lowercase().as_str() {
        "critical" | "crit" => "critical",
        "error" | "err" => "error",
        "warn" | "warning" => "warning",
        "info" | "i" => "info",
        other => {
            return Err(CliError::Other(format!(
                "invalid --severity {other:?}; expected critical|error|warn|info"
            )));
        }
    };
    Ok(Some(canonical.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_severity_accepts_canonical() {
        assert_eq!(normalise_severity("critical").unwrap().as_deref(), Some("critical"));
        assert_eq!(normalise_severity("error").unwrap().as_deref(), Some("error"));
        assert_eq!(normalise_severity("warn").unwrap().as_deref(), Some("warning"));
        assert_eq!(normalise_severity("warning").unwrap().as_deref(), Some("warning"));
        assert_eq!(normalise_severity("info").unwrap().as_deref(), Some("info"));
    }

    #[test]
    fn normalise_severity_rejects_garbage() {
        assert!(normalise_severity("urgent").is_err());
        assert!(normalise_severity("").is_err());
    }

    #[test]
    fn normalise_severity_synonyms() {
        assert_eq!(normalise_severity("CRIT").unwrap().as_deref(), Some("critical"));
        assert_eq!(normalise_severity("Err").unwrap().as_deref(), Some("error"));
    }
}
