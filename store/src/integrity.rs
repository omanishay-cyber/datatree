//! Cross-shard integrity audit for multi-SQLite shards.
//!
//! # Problem
//!
//! Mneme stores data across ~26 per-project SQLite shards. Several shards hold
//! columns that are *logically* foreign keys into a different shard (e.g.
//! `findings.file` references `graph.files.path`). SQLite can only enforce
//! referential integrity within one database file, so these cross-file
//! relationships are unenforced. When a file is deleted from the graph shard
//! its `findings`, `refactor_proposals`, `commit_files`, etc. rows become
//! orphans silently.
//!
//! # Reference matrix
//!
//! | Source shard | Source table.column | Target shard | Target table.column |
//! |---|---|---|---|
//! | Findings | `findings.file` | Graph | `files.path` |
//! | Semantic | `community_membership.node_qualified` | Graph | `nodes.qualified_name` |
//! | Tests | `test_coverage.function_qualified` | Graph | `nodes.qualified_name` |
//! | Refactors | `refactor_proposals.file` | Graph | `files.path` |
//! | Refactors | `refactor_proposals.symbol` (nullable) | Graph | `nodes.qualified_name` |
//! | LiveState | `file_events.file_path` | Graph | `files.path` |
//! | Corpus | `corpus_items.file_path` | Graph | `files.path` |
//! | Git | `commit_files.file_path` | Graph | `files.path` |
//! | Errors | `errors.file_path` (nullable) | Graph | `files.path` |
//! | Wiki | `wiki_pages.community_id` (nullable) | Semantic | `communities.id` |
//!
//! # Design
//!
//! - **Read-only audit**: every shard is opened with `SQLITE_OPEN_READ_ONLY`.
//!   No writes happen; the audit is purely observational.
//! - **ATTACH-based joins**: each check ATTACHes the target shard, runs a
//!   single LEFT JOIN query to find unmatched values, then DETACHes. This
//!   avoids loading both shards into Rust memory.
//! - **Log + return**: orphans are logged at `warn!` level and returned to
//!   the caller. Deletion is a user-driven decision via `mneme rebuild`.
//! - **`apply_pragmas`**: every connection goes through the canonical
//!   pragma helper (busy_timeout, WAL mode, foreign_keys, mmap_size).
//!
//! Wire orphans into `mneme audit` as findings with
//! `rule_id = "cross_shard_orphan.<shard>.<column>"`.

use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tracing::warn;

use common::{
    error::{DbError, DtResult},
    ids::ProjectId,
    layer::DbLayer,
    paths::PathManager,
};

// ──────────────────────────────────────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────────────────────────────────────

/// One orphan row discovered by the cross-shard audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrphanRow {
    /// The shard that owns the referencing column (e.g. `DbLayer::Findings`).
    pub source_layer: DbLayer,
    /// `"table.column"` in the source shard (e.g. `"findings.file"`).
    pub source_table_column: String,
    /// The dangling value stored in that column.
    pub value: String,
    /// The shard that was expected to contain the value.
    pub target_layer: DbLayer,
    /// `"table.column"` in the target shard (e.g. `"files.path"`).
    pub target_table_column: String,
}

/// Full result of one `cross_shard_integrity_audit` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossShardReport {
    /// All orphan rows found across every checked relationship.
    pub orphans: Vec<OrphanRow>,
}

impl CrossShardReport {
    /// `true` when no orphans were found.
    pub fn is_clean(&self) -> bool {
        self.orphans.is_empty()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Audit entry point
// ──────────────────────────────────────────────────────────────────────────────

/// Run the full cross-shard integrity audit for a project.
///
/// Opens each relevant source shard read-only, ATTACHes the appropriate target
/// shard, runs a LEFT JOIN orphan query, then DETACHes. Results are logged at
/// `warn!` and aggregated into a [`CrossShardReport`].
///
/// Returns `Ok(CrossShardReport)` even when orphans exist — the *error* path
/// is reserved for I/O failures. A non-empty `orphans` vec is a data-integrity
/// signal, not a Rust error.
///
/// # Skips missing shards
///
/// If either the source or target shard file does not exist on disk the
/// relationship is silently skipped. This is the normal state for fresh
/// projects that haven't run every subsystem yet.
pub fn cross_shard_integrity_audit(
    paths: &Arc<PathManager>,
    project: &ProjectId,
) -> DtResult<CrossShardReport> {
    let mut orphans: Vec<OrphanRow> = Vec::new();

    // Helper: path for a layer's shard file.
    let shard = |layer: DbLayer| paths.shard_db(project, layer);

    // ── 1. findings.file → graph.files.path ──────────────────────────────────
    check_cross_shard_text(
        &shard(DbLayer::Findings),
        &shard(DbLayer::Graph),
        DbLayer::Findings,
        "findings.file",
        // LEFT JOIN: finding rows whose `file` value has no matching path in graph.files.
        r#"
            SELECT DISTINCT f.file
            FROM   main.findings f
            LEFT   JOIN target.files gf ON gf.path = f.file
            WHERE  gf.path IS NULL
        "#,
        DbLayer::Graph,
        "files.path",
        &mut orphans,
    )?;

    // ── 2. community_membership.node_qualified → graph.nodes.qualified_name ──
    check_cross_shard_text(
        &shard(DbLayer::Semantic),
        &shard(DbLayer::Graph),
        DbLayer::Semantic,
        "community_membership.node_qualified",
        r#"
            SELECT DISTINCT cm.node_qualified
            FROM   main.community_membership cm
            LEFT   JOIN target.nodes n ON n.qualified_name = cm.node_qualified
            WHERE  n.qualified_name IS NULL
        "#,
        DbLayer::Graph,
        "nodes.qualified_name",
        &mut orphans,
    )?;

    // ── 3. test_coverage.function_qualified → graph.nodes.qualified_name ─────
    check_cross_shard_text(
        &shard(DbLayer::Tests),
        &shard(DbLayer::Graph),
        DbLayer::Tests,
        "test_coverage.function_qualified",
        r#"
            SELECT DISTINCT tc.function_qualified
            FROM   main.test_coverage tc
            LEFT   JOIN target.nodes n ON n.qualified_name = tc.function_qualified
            WHERE  n.qualified_name IS NULL
        "#,
        DbLayer::Graph,
        "nodes.qualified_name",
        &mut orphans,
    )?;

    // ── 4. refactor_proposals.file → graph.files.path ────────────────────────
    check_cross_shard_text(
        &shard(DbLayer::Refactors),
        &shard(DbLayer::Graph),
        DbLayer::Refactors,
        "refactor_proposals.file",
        r#"
            SELECT DISTINCT rp.file
            FROM   main.refactor_proposals rp
            LEFT   JOIN target.files gf ON gf.path = rp.file
            WHERE  gf.path IS NULL
        "#,
        DbLayer::Graph,
        "files.path",
        &mut orphans,
    )?;

    // ── 5. refactor_proposals.symbol → graph.nodes.qualified_name (nullable) ─
    check_cross_shard_text(
        &shard(DbLayer::Refactors),
        &shard(DbLayer::Graph),
        DbLayer::Refactors,
        "refactor_proposals.symbol",
        r#"
            SELECT DISTINCT rp.symbol
            FROM   main.refactor_proposals rp
            LEFT   JOIN target.nodes n ON n.qualified_name = rp.symbol
            WHERE  rp.symbol IS NOT NULL
              AND  n.qualified_name IS NULL
        "#,
        DbLayer::Graph,
        "nodes.qualified_name",
        &mut orphans,
    )?;

    // ── 6. file_events.file_path → graph.files.path ──────────────────────────
    check_cross_shard_text(
        &shard(DbLayer::LiveState),
        &shard(DbLayer::Graph),
        DbLayer::LiveState,
        "file_events.file_path",
        r#"
            SELECT DISTINCT fe.file_path
            FROM   main.file_events fe
            LEFT   JOIN target.files gf ON gf.path = fe.file_path
            WHERE  gf.path IS NULL
        "#,
        DbLayer::Graph,
        "files.path",
        &mut orphans,
    )?;

    // ── 7. corpus_items.file_path → graph.files.path ─────────────────────────
    check_cross_shard_text(
        &shard(DbLayer::Corpus),
        &shard(DbLayer::Graph),
        DbLayer::Corpus,
        "corpus_items.file_path",
        r#"
            SELECT DISTINCT ci.file_path
            FROM   main.corpus_items ci
            LEFT   JOIN target.files gf ON gf.path = ci.file_path
            WHERE  gf.path IS NULL
        "#,
        DbLayer::Graph,
        "files.path",
        &mut orphans,
    )?;

    // ── 8. commit_files.file_path → graph.files.path ─────────────────────────
    check_cross_shard_text(
        &shard(DbLayer::Git),
        &shard(DbLayer::Graph),
        DbLayer::Git,
        "commit_files.file_path",
        r#"
            SELECT DISTINCT cf.file_path
            FROM   main.commit_files cf
            LEFT   JOIN target.files gf ON gf.path = cf.file_path
            WHERE  gf.path IS NULL
        "#,
        DbLayer::Graph,
        "files.path",
        &mut orphans,
    )?;

    // ── 9. errors.file_path → graph.files.path (nullable) ────────────────────
    check_cross_shard_text(
        &shard(DbLayer::Errors),
        &shard(DbLayer::Graph),
        DbLayer::Errors,
        "errors.file_path",
        r#"
            SELECT DISTINCT e.file_path
            FROM   main.errors e
            LEFT   JOIN target.files gf ON gf.path = e.file_path
            WHERE  e.file_path IS NOT NULL
              AND  gf.path IS NULL
        "#,
        DbLayer::Graph,
        "files.path",
        &mut orphans,
    )?;

    // ── 10. wiki_pages.community_id → semantic.communities.id (nullable) ─────
    // community_id is INTEGER, so we coerce to TEXT for the uniform OrphanRow.
    check_cross_shard_text(
        &shard(DbLayer::Wiki),
        &shard(DbLayer::Semantic),
        DbLayer::Wiki,
        "wiki_pages.community_id",
        r#"
            SELECT DISTINCT CAST(wp.community_id AS TEXT)
            FROM   main.wiki_pages wp
            LEFT   JOIN target.communities c ON c.id = wp.community_id
            WHERE  wp.community_id IS NOT NULL
              AND  c.id IS NULL
        "#,
        DbLayer::Semantic,
        "communities.id",
        &mut orphans,
    )?;

    if !orphans.is_empty() {
        warn!(
            count = orphans.len(),
            "cross-shard integrity audit found orphan rows; run `mneme rebuild` to clear them"
        );
    }

    Ok(CrossShardReport { orphans })
}

// ──────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Apply only the pragmas that are safe on a `SQLITE_OPEN_READ_ONLY` connection.
///
/// `journal_mode = WAL` and `auto_vacuum = INCREMENTAL` require write access
/// to update database-file headers and sidecar files (`-wal`, `-shm`). On a
/// read-only connection those pragmas return `SQLITE_READONLY` (mapped to
/// `Db(PermissionDenied)`). The full `apply_pragmas` from builder.rs is only
/// appropriate for writable opens. This helper applies the safe subset:
///
///   * `busy_timeout = 5000` — retry on contention instead of failing instantly
///   * `mmap_size`           — memory-map for faster sequential reads
fn apply_pragmas_readonly(conn: &Connection) -> DtResult<()> {
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(DbError::from)?;
    conn.pragma_update(None, "mmap_size", 268_435_456_i64)
        .map_err(DbError::from)?;
    Ok(())
}

/// Open `source_path` read-only, ATTACH `target_path` as `target`, execute
/// `orphan_sql` (SELECT of dangling TEXT values), then DETACH. Appends any
/// discovered orphan values to `out`.
///
/// Both shard files must exist; if either is absent the check is skipped
/// silently (normal for fresh projects).
fn check_cross_shard_text(
    source_path: &Path,
    target_path: &Path,
    source_layer: DbLayer,
    source_table_column: &str,
    orphan_sql: &str,
    target_layer: DbLayer,
    target_table_column: &str,
    out: &mut Vec<OrphanRow>,
) -> DtResult<()> {
    if !source_path.exists() || !target_path.exists() {
        return Ok(());
    }

    let conn = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(DbError::from)?;
    // Apply a read-safe subset of the canonical pragmas. We call
    // apply_pragmas_readonly rather than the full apply_pragmas because:
    //   * `journal_mode = WAL` requires write access to create/update the
    //     -wal and -shm sidecars; on a SQLITE_OPEN_READ_ONLY connection it
    //     returns SQLITE_READONLY (PermissionDenied) on Windows.
    //   * `auto_vacuum = INCREMENTAL` similarly needs write access to update
    //     the database header.
    // We keep busy_timeout and mmap_size — both are safe on read-only
    // connections and make the audit more robust under concurrent writers.
    apply_pragmas_readonly(&conn)?;

    // ATTACH the target shard under the alias `target`.
    // SQLite parameter binding does not support ATTACH path arguments, so we
    // format the path directly. The path comes from PathManager — a
    // trusted, internal source — never from user input.
    let target_path_str = target_path.to_string_lossy();
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS target",
        // Escape any embedded single-quotes (Windows paths with apostrophes)
        target_path_str.replace('\'', "''")
    ))
    .map_err(DbError::from)?;

    let mut stmt = conn.prepare(orphan_sql).map_err(DbError::from)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(DbError::from)?;

    for row in rows {
        let value = row.map_err(DbError::from)?;
        out.push(OrphanRow {
            source_layer,
            source_table_column: source_table_column.to_string(),
            value,
            target_layer,
            target_table_column: target_table_column.to_string(),
        });
    }

    conn.execute_batch("DETACH DATABASE target")
        .map_err(DbError::from)?;

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    // -------------------------------------------------------------------------
    // Fabricated PathManager pointing at a temp directory.
    //
    // We need `PathManager::shard_db(project, layer)` to return real paths so
    // the audit entry-point resolves shard files correctly. The simplest
    // approach is to build a PathManager whose root is the temp directory and
    // pre-populate shard files with the right names.
    // -------------------------------------------------------------------------

    /// Build a PathManager rooted at `dir` and create stub shard files with
    /// the provided DDL. Returns (paths, project_id).
    fn setup(dir: &TempDir, shards: &[(DbLayer, &str)]) -> (Arc<PathManager>, ProjectId) {
        let root = dir.path().to_path_buf();
        let project_id = ProjectId::from_hash("test");

        // PathManager expects <root>/projects/<project_id>/<layer>.db
        let project_dir = root.join("projects").join(project_id.as_str());
        std::fs::create_dir_all(&project_dir).expect("create project dir");

        for (layer, ddl) in shards {
            let shard_path = project_dir.join(layer.file_name());
            let conn = Connection::open(&shard_path).expect("open shard");
            conn.execute_batch(ddl).expect("apply ddl");
        }

        let paths = Arc::new(PathManager::with_root(root));
        (paths, project_id)
    }

    // -------------------------------------------------------------------------
    // Test 1: empty shards → 0 orphans
    // -------------------------------------------------------------------------
    #[test]
    fn empty_shards_yield_no_orphans() {
        let dir = TempDir::new().unwrap();
        let (paths, project) = setup(
            &dir,
            &[
                (
                    DbLayer::Findings,
                    "CREATE TABLE IF NOT EXISTS findings (
                         id INTEGER PRIMARY KEY,
                         rule_id TEXT NOT NULL,
                         scanner TEXT NOT NULL,
                         severity TEXT NOT NULL,
                         file TEXT NOT NULL,
                         line_start INTEGER NOT NULL DEFAULT 0,
                         line_end INTEGER NOT NULL DEFAULT 0,
                         column_start INTEGER NOT NULL DEFAULT 0,
                         column_end INTEGER NOT NULL DEFAULT 0,
                         message TEXT NOT NULL
                     )",
                ),
                (
                    DbLayer::Graph,
                    "CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL);
                     CREATE TABLE IF NOT EXISTS nodes (
                         id INTEGER PRIMARY KEY AUTOINCREMENT,
                         qualified_name TEXT UNIQUE NOT NULL,
                         name TEXT NOT NULL,
                         kind TEXT NOT NULL
                     )",
                ),
            ],
        );

        let report = cross_shard_integrity_audit(&paths, &project).unwrap();
        assert!(
            report.is_clean(),
            "empty shards must yield 0 orphans; got: {:?}",
            report.orphans
        );
    }

    // -------------------------------------------------------------------------
    // Test 2: planted orphan in findings.file → exactly 1 orphan reported
    // -------------------------------------------------------------------------
    #[test]
    fn orphan_in_findings_file_is_detected() {
        let dir = TempDir::new().unwrap();
        let (paths, project) = setup(
            &dir,
            &[
                (
                    DbLayer::Findings,
                    "CREATE TABLE IF NOT EXISTS findings (
                         id INTEGER PRIMARY KEY,
                         rule_id TEXT NOT NULL DEFAULT 'r',
                         scanner TEXT NOT NULL DEFAULT 's',
                         severity TEXT NOT NULL DEFAULT 'warn',
                         file TEXT NOT NULL,
                         line_start INTEGER NOT NULL DEFAULT 1,
                         line_end INTEGER NOT NULL DEFAULT 1,
                         column_start INTEGER NOT NULL DEFAULT 0,
                         column_end INTEGER NOT NULL DEFAULT 0,
                         message TEXT NOT NULL DEFAULT 'msg'
                     );
                     -- Insert a finding whose `file` does NOT exist in graph.files
                     INSERT INTO findings (file) VALUES ('ghost/nonexistent.ts')",
                ),
                (
                    DbLayer::Graph,
                    // graph.files is empty — so ghost/nonexistent.ts has no match
                    "CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL);
                     CREATE TABLE IF NOT EXISTS nodes (
                         id INTEGER PRIMARY KEY AUTOINCREMENT,
                         qualified_name TEXT UNIQUE NOT NULL,
                         name TEXT NOT NULL,
                         kind TEXT NOT NULL
                     )",
                ),
            ],
        );

        let report = cross_shard_integrity_audit(&paths, &project).unwrap();

        // Only findings.file → graph.files.path should fire.
        let findings_orphans: Vec<_> = report
            .orphans
            .iter()
            .filter(|o| o.source_layer == DbLayer::Findings)
            .collect();
        assert_eq!(
            findings_orphans.len(),
            1,
            "expected exactly 1 orphan in findings.file; got {:?}",
            findings_orphans
        );
        assert_eq!(findings_orphans[0].value, "ghost/nonexistent.ts");
        assert_eq!(findings_orphans[0].source_table_column, "findings.file");
        assert_eq!(findings_orphans[0].target_layer, DbLayer::Graph);
        assert_eq!(findings_orphans[0].target_table_column, "files.path");
    }

    // -------------------------------------------------------------------------
    // Test 3: real referenced row + planted orphan → only the orphan appears
    // -------------------------------------------------------------------------
    #[test]
    fn valid_row_is_not_reported_as_orphan() {
        let dir = TempDir::new().unwrap();
        let (paths, project) = setup(
            &dir,
            &[
                (
                    DbLayer::Findings,
                    "CREATE TABLE IF NOT EXISTS findings (
                         id INTEGER PRIMARY KEY,
                         rule_id TEXT NOT NULL DEFAULT 'r',
                         scanner TEXT NOT NULL DEFAULT 's',
                         severity TEXT NOT NULL DEFAULT 'warn',
                         file TEXT NOT NULL,
                         line_start INTEGER NOT NULL DEFAULT 1,
                         line_end INTEGER NOT NULL DEFAULT 1,
                         column_start INTEGER NOT NULL DEFAULT 0,
                         column_end INTEGER NOT NULL DEFAULT 0,
                         message TEXT NOT NULL DEFAULT 'msg'
                     );
                     -- Row 1: file exists in graph.files (should NOT appear as orphan)
                     INSERT INTO findings (file) VALUES ('src/main.rs');
                     -- Row 2: file does NOT exist in graph.files (should appear)
                     INSERT INTO findings (file) VALUES ('deleted/removed.rs')",
                ),
                (
                    DbLayer::Graph,
                    "CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL);
                     -- Only src/main.rs is registered; deleted/removed.rs was purged
                     INSERT INTO files (path, sha256) VALUES ('src/main.rs', 'deadbeef');
                     CREATE TABLE IF NOT EXISTS nodes (
                         id INTEGER PRIMARY KEY AUTOINCREMENT,
                         qualified_name TEXT UNIQUE NOT NULL,
                         name TEXT NOT NULL,
                         kind TEXT NOT NULL
                     )",
                ),
            ],
        );

        let report = cross_shard_integrity_audit(&paths, &project).unwrap();

        let findings_orphans: Vec<_> = report
            .orphans
            .iter()
            .filter(|o| o.source_layer == DbLayer::Findings)
            .collect();

        assert_eq!(
            findings_orphans.len(),
            1,
            "only the orphan row must be reported; got {:?}",
            findings_orphans
        );
        assert_eq!(
            findings_orphans[0].value, "deleted/removed.rs",
            "the orphan must be the deleted file, not the valid one"
        );
        // src/main.rs must NOT appear.
        assert!(
            !findings_orphans.iter().any(|o| o.value == "src/main.rs"),
            "the valid row src/main.rs must NOT be reported as an orphan"
        );
    }
}
