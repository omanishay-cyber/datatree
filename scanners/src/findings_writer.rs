//! Direct per-shard writer for `findings.db`.
//!
//! Complements the IPC batcher in [`crate::store_ipc`]. The supervisor uses
//! the batcher path (async, cross-process); the `mneme audit` CLI and unit
//! tests use this direct writer because they want synchronous, in-process
//! persistence of a single scan's worth of findings into a concrete
//! SQLite file.
//!
//! The per-shard single-writer invariant is preserved because callers open
//! one short-lived [`rusqlite::Connection`] per batch and drop it on
//! completion. Concurrent audits SHOULD serialize at the process level —
//! the supervisor is the sole long-running writer; `mneme audit` is a
//! one-shot invocation.
//!
//! Wire format matches the `findings` table in `store/src/schema.rs`
//! exactly: `(rule_id, scanner, severity, file, line_start, line_end,
//! column_start, column_end, message, suggestion, auto_fixable)`.
//!
//! The `scanner` column is derived from the rule_id prefix (everything up
//! to the first `.`): `theme.hardcoded-hex` -> `theme`. Findings whose
//! rule_id carries no dot are tagged with the fallback `"unknown"` so a
//! row is never rejected.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::error::{Result, ScannerError};
use crate::scanner::{Finding, Severity};

/// Deduced scanner name from a `rule_id` prefix.
///
/// `theme.hardcoded-hex` -> `theme`; `security.eval` -> `security`;
/// `no-dot-rule` -> `unknown`. Never panics.
#[must_use]
pub fn scanner_name_for_rule(rule_id: &str) -> &str {
    match rule_id.split_once('.') {
        Some((prefix, _)) if !prefix.is_empty() => prefix,
        _ => "unknown",
    }
}

/// Short-lived, synchronous writer for the `findings.db` shard.
///
/// Construct via [`FindingsWriter::open`]. Each call to
/// [`FindingsWriter::write_findings`] runs inside a single transaction so a
/// mid-batch crash can never leave partial rows behind. Dropping the writer
/// closes the connection.
pub struct FindingsWriter {
    conn: Connection,
}

impl FindingsWriter {
    /// Open (or create) the findings shard at `db_path`. If the table does
    /// not yet exist it is created on the fly — this makes the writer safe
    /// to use from unit tests and `mneme audit --standalone` paths where
    /// the normal `DbBuilder` init may not have run.
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(ScannerError::Io)?;
            }
        }
        let conn = Connection::open(db_path).map_err(|e| ScannerError::Other(e.to_string()))?;
        conn.execute_batch(ENSURE_TABLE_SQL)
            .map_err(|e| ScannerError::Other(e.to_string()))?;
        // WAL gives concurrent readers while the writer holds its short
        // transaction. Safe to set repeatedly; no-op if already WAL.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        Ok(Self { conn })
    }

    /// Persist every finding in `batch`. Returns the number of rows
    /// inserted. Runs in a single immediate transaction — all-or-nothing.
    pub fn write_findings(&mut self, batch: &[Finding]) -> Result<usize> {
        if batch.is_empty() {
            return Ok(0);
        }
        let tx = self
            .conn
            .transaction()
            .map_err(|e| ScannerError::Other(e.to_string()))?;
        let mut n = 0usize;
        {
            let mut stmt = tx
                .prepare_cached(INSERT_SQL)
                .map_err(|e| ScannerError::Other(e.to_string()))?;
            for f in batch {
                let scanner = scanner_name_for_rule(&f.rule_id);
                let severity_label = severity_label(f.severity);
                let auto_fixable_i: i64 = if f.auto_fixable { 1 } else { 0 };
                stmt.execute(params![
                    &f.rule_id,
                    scanner,
                    severity_label,
                    &f.file,
                    f.line_start as i64,
                    f.line_end as i64,
                    f.column_start as i64,
                    f.column_end as i64,
                    &f.message,
                    f.suggestion.as_deref(),
                    auto_fixable_i,
                ])
                .map_err(|e| ScannerError::Other(e.to_string()))?;
                n += 1;
            }
        }
        tx.commit()
            .map_err(|e| ScannerError::Other(e.to_string()))?;
        Ok(n)
    }

    /// Count open findings (`resolved_at IS NULL`). Useful for tests.
    pub fn open_findings_count(&self) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM findings WHERE resolved_at IS NULL",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| ScannerError::Other(e.to_string()))
    }
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "critical",
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

const INSERT_SQL: &str = r#"
INSERT INTO findings (
    rule_id, scanner, severity, file,
    line_start, line_end, column_start, column_end,
    message, suggestion, auto_fixable
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
"#;

// Mirrors `store::schema::FINDINGS_SQL`. Kept in sync manually: this crate
// does NOT depend on `store` to avoid a dependency cycle (store depends on
// common; scanners depends on common; store links rusqlite at
// workspace-pinned version so we reuse that).
const ENSURE_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id TEXT NOT NULL,
    scanner TEXT NOT NULL,
    severity TEXT NOT NULL,
    file TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    column_start INTEGER NOT NULL,
    column_end INTEGER NOT NULL,
    message TEXT NOT NULL,
    suggestion TEXT,
    auto_fixable INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_findings_file ON findings(file);
CREATE INDEX IF NOT EXISTS idx_findings_severity ON findings(severity);
CREATE INDEX IF NOT EXISTS idx_findings_open ON findings(resolved_at) WHERE resolved_at IS NULL;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn mk_finding(rule: &str, sev: Severity, file: &str) -> Finding {
        Finding::new_line(rule, sev, file, 1, 0, 10, format!("{rule} msg"))
    }

    #[test]
    fn scanner_name_prefix_extraction() {
        assert_eq!(scanner_name_for_rule("theme.hardcoded-hex"), "theme");
        assert_eq!(scanner_name_for_rule("security.eval"), "security");
        assert_eq!(scanner_name_for_rule("a11y.img-no-alt"), "a11y");
        assert_eq!(scanner_name_for_rule("perf.sync-io"), "perf");
        assert_eq!(scanner_name_for_rule("secrets.aws-access-key"), "secrets");
        assert_eq!(scanner_name_for_rule("no-dot-rule"), "unknown");
        assert_eq!(scanner_name_for_rule(""), "unknown");
    }

    #[test]
    fn writes_one_finding_per_scanner() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("findings.db");
        let mut w = FindingsWriter::open(&path).unwrap();
        let batch = vec![
            mk_finding("theme.hardcoded-hex", Severity::Warning, "a.tsx"),
            mk_finding("security.dynamic-eval-call", Severity::Critical, "b.ts"),
            mk_finding("a11y.img-no-alt", Severity::Error, "c.tsx"),
            mk_finding("perf.sync-io", Severity::Info, "d.ts"),
            mk_finding("secrets.aws-access-key", Severity::Critical, "e.ts"),
        ];
        let n = w.write_findings(&batch).unwrap();
        assert_eq!(n, 5);
        assert_eq!(w.open_findings_count().unwrap(), 5);

        // Row-level assertions: scanner column derived from prefix.
        let got: Vec<(String, String, String)> = {
            let conn = Connection::open(&path).unwrap();
            let mut stmt = conn
                .prepare("SELECT rule_id, scanner, severity FROM findings ORDER BY id")
                .unwrap();
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
        };
        assert_eq!(got[0].1, "theme");
        assert_eq!(got[0].2, "warning");
        assert_eq!(got[1].1, "security");
        assert_eq!(got[1].2, "critical");
        assert_eq!(got[2].1, "a11y");
        assert_eq!(got[3].1, "perf");
        assert_eq!(got[4].1, "secrets");
    }

    #[test]
    fn empty_batch_is_noop() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("findings.db");
        let mut w = FindingsWriter::open(&path).unwrap();
        let n = w.write_findings(&[]).unwrap();
        assert_eq!(n, 0);
        assert_eq!(w.open_findings_count().unwrap(), 0);
    }

    #[test]
    fn preserves_suggestion_and_auto_fixable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("findings.db");
        let mut w = FindingsWriter::open(&path).unwrap();
        let f = Finding::new_line(
            "theme.hardcoded-hex",
            Severity::Warning,
            "x.tsx",
            1,
            0,
            7,
            "Hardcoded hex",
        )
        .with_fix("var(--color-TODO)");
        w.write_findings(&[f]).unwrap();
        let conn = Connection::open(&path).unwrap();
        let (sug, afx): (Option<String>, i64) = conn
            .query_row(
                "SELECT suggestion, auto_fixable FROM findings LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(sug.as_deref(), Some("var(--color-TODO)"));
        assert_eq!(afx, 1);
    }
}
