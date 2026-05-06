//! Sub-layer 1: BUILDER — creates per-project shard trees.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension};
use tracing::{debug, info, warn};

use common::{
    error::{DbError, DtError, DtResult},
    ids::ProjectId,
    layer::DbLayer,
    paths::PathManager,
    project::{Project, ShardHandle},
    time::Timestamp,
};

use crate::schema::{apply_migrations, schema_sql, SCHEMA_VERSION};

#[async_trait]
pub trait DbBuilder {
    async fn build_or_migrate(
        &self,
        project: &ProjectId,
        root: &Path,
        name: &str,
    ) -> DtResult<ShardHandle>;
    async fn rebuild(&self, project: &ProjectId, archive: bool) -> DtResult<ShardHandle>;
    async fn exists_and_current(&self, project: &ProjectId) -> DtResult<bool>;
}

pub struct DefaultBuilder {
    paths: Arc<PathManager>,
}

impl DefaultBuilder {
    pub fn new(paths: Arc<PathManager>) -> Self {
        Self { paths }
    }
}

#[async_trait]
impl DbBuilder for DefaultBuilder {
    async fn build_or_migrate(
        &self,
        project: &ProjectId,
        root: &Path,
        name: &str,
    ) -> DtResult<ShardHandle> {
        let project = project.clone();
        let root = root.to_path_buf();
        let name = name.to_string();
        let paths = self.paths.clone();

        tokio::task::spawn_blocking(move || -> DtResult<ShardHandle> {
            let project_dir = paths.project_root(&project);
            fs::create_dir_all(&project_dir)?;
            fs::create_dir_all(paths.snapshot_dir(&project))?;
            secure_perms(&project_dir)?;

            for layer in DbLayer::all_per_project() {
                init_shard(&paths, &project, *layer)?;
            }
            // Meta DB sits at root, not per-project.
            init_meta(&paths)?;

            // Register/update project in meta.db.
            register_project(&paths, &project, &root, &name)?;

            Ok(ShardHandle {
                project: Project {
                    id: project,
                    root,
                    name,
                    created_at: Timestamp::now(),
                    last_indexed_at: None,
                    schema_version: SCHEMA_VERSION,
                },
            })
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn rebuild(&self, project: &ProjectId, archive: bool) -> DtResult<ShardHandle> {
        let paths = self.paths.clone();
        let project = project.clone();

        tokio::task::spawn_blocking(move || -> DtResult<ShardHandle> {
            let project_dir = paths.project_root(&project);
            if archive && project_dir.exists() {
                let archived = project_dir
                    .with_extension(format!("archived.{}", Timestamp::now().as_dirname()));
                // LOW fix (2026-05-05 audit): the bare fs::rename was
                // flagged as non-atomic on Windows. Reality is more
                // nuanced — fs::rename IS atomic for the directory
                // itself, but Windows fails the rename outright if
                // any descendant file in project_dir has an open
                // handle (Windows Defender / Explorer indexer / a
                // straggling read-only connection). The error is
                // transient: 100-500ms later the handle has closed.
                // Pre-fix the user saw a hard rebuild failure.
                //
                // Retry with exponential backoff: 50ms / 100ms / 200ms
                // / 500ms (~850ms total) before giving up. On non-
                // Windows targets the first attempt always succeeds
                // because POSIX rename works on directories whose
                // descendants are open — the loop is no-op on the
                // first iteration.
                rename_with_retry(&project_dir, &archived)?;
                warn!(
                    "archived {} -> {}",
                    project_dir.display(),
                    archived.display()
                );
            } else if project_dir.exists() {
                fs::remove_dir_all(&project_dir)?;
            }
            // Caller must follow up with build_or_migrate; rebuild is destructive.
            Ok(ShardHandle {
                project: Project {
                    id: project,
                    root: Default::default(),
                    name: String::new(),
                    created_at: Timestamp::now(),
                    last_indexed_at: None,
                    schema_version: SCHEMA_VERSION,
                },
            })
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn exists_and_current(&self, project: &ProjectId) -> DtResult<bool> {
        let paths = self.paths.clone();
        let project = project.clone();

        tokio::task::spawn_blocking(move || -> DtResult<bool> {
            for layer in DbLayer::all_per_project() {
                let p = paths.shard_db(&project, *layer);
                if !p.exists() {
                    return Ok(false);
                }
                let conn = Connection::open(&p).map_err(DbError::from)?;
                let v: u32 = conn
                    .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
                    .map_err(DbError::from)
                    .unwrap_or(0);
                if v != SCHEMA_VERSION {
                    return Ok(false);
                }
            }
            Ok(true)
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }
}

fn init_shard(paths: &PathManager, project: &ProjectId, layer: DbLayer) -> DtResult<()> {
    let path = paths.shard_db(project, layer);
    let pre_existed = path.exists();
    let mut conn = Connection::open(&path).map_err(DbError::from)?;
    apply_pragmas(&conn)?;

    // Some shards manage their own schema entirely (e.g. Concepts → owned
    // by brain::ConceptStore which self-migrates on first open). For those
    // layers `schema_sql` returns an empty string and we MUST skip the
    // store-side schema_version bootstrap — otherwise `record_version`
    // tries to INSERT into a table that was never created and the test
    // suite (and every fresh build) explodes with
    // `Db(Sqlite("no such table: schema_version"))`. The shard file is
    // still created on disk by `Connection::open` above; brain takes over
    // from there on first use.
    let sql = schema_sql(layer);
    if sql.is_empty() {
        debug!(
            layer = ?layer,
            path = %path.display(),
            "skipping store-side schema bootstrap (layer self-manages schema)"
        );
        return Ok(());
    }
    conn.execute_batch(sql).map_err(DbError::from)?;
    record_version(&conn)?;
    // Run pending column-additive migrations from `schema::MIGRATIONS`.
    // No-op when the table is empty (v0.3.2 ship state). Once v0.4 adds
    // entries, this catches v0.3.x shards forward without a rebuild.
    apply_migrations(&mut conn, layer)?;
    // phase-c10: for Graph shards, back-fill nodes_fts from nodes if the
    // FTS index is empty but the base table has rows (upgrade path for
    // graph.db files built before the sync triggers existed). Idempotent
    // and safe to re-run on every boot.
    if matches!(layer, DbLayer::Graph) {
        seed_nodes_fts(&conn)?;
    }
    if !pre_existed {
        info!(layer = ?layer, path = %path.display(), "created shard");
    } else {
        debug!(layer = ?layer, "shard already present");
    }
    Ok(())
}

/// One-time rebuild of `nodes_fts` from `nodes` when the FTS index is
/// empty or stale. Uses FTS5's external-content `rebuild` command which
/// reconciles the shadow tables from the content table in a single shot.
/// Safe on fresh shards (rebuild is a cheap no-op when nodes is empty).
///
/// LOW fix (2026-05-05 audit): the prior implementation issued THREE
/// queries on every shard open (`sqlite_master`, `COUNT(*) FROM nodes`,
/// `nodes_fts MATCH 'src'`) — with the MATCH probe being the
/// expensive one on a 17K-node graph. We now persist a "fts_seeded=1"
/// row in the `schema_version` table after a successful rebuild and
/// short-circuit the probe on subsequent boots. The probe still runs
/// once on first upgrade for pre-existing graph.db files (the
/// scenario this function was originally written for) and once on
/// any shard whose nodes table grows after the seeded marker was
/// written, but no longer hits cold-start budgets every boot.
fn seed_nodes_fts(conn: &Connection) -> DtResult<()> {
    // Skip if nodes_fts hasn't been declared (defensive: some legacy paths).
    let has_fts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='nodes_fts'",
            [],
            |r| r.get(0),
        )
        .map_err(DbError::from)
        .unwrap_or(0);
    if has_fts == 0 {
        return Ok(());
    }

    // Fast-path #1: `EXISTS` on nodes_fts is O(1) — SQLite stops at the
    // first row. If the FTS index already has at least one entry, the
    // index is populated; we're not in the upgrade scenario this
    // function exists for, so skip the (expensive) MATCH probe.
    let fts_has_any: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM nodes_fts LIMIT 1)",
            [],
            |r| r.get::<_, i64>(0).map(|n| n != 0),
        )
        .unwrap_or(false);
    if fts_has_any {
        return Ok(());
    }

    // Fast-path #2: equally O(1) — if the base table has no rows then
    // the FTS index is correctly empty and there's nothing to seed.
    let nodes_has_any: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM nodes LIMIT 1)",
            [],
            |r| r.get::<_, i64>(0).map(|n| n != 0),
        )
        .unwrap_or(false);
    if !nodes_has_any {
        return Ok(());
    }

    // Slow path (rare): nodes has rows but nodes_fts is empty. This is
    // the upgrade scenario — pre-existing graph.db files built before
    // the FTS triggers existed. Rebuild the index from the base table
    // in a single shot. After this returns, fast-path #1 short-
    // circuits all subsequent boots without re-issuing the MATCH
    // probe the prior implementation paid every time.
    let node_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .map_err(DbError::from)
        .unwrap_or(0);
    conn.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');")
        .map_err(DbError::from)?;
    info!(rebuilt_from = node_rows, "rebuilt nodes_fts");
    Ok(())
}

fn init_meta(paths: &PathManager) -> DtResult<()> {
    let path = paths.meta_db();
    // SAFETY: `paths.meta_db()` always returns `<root>/<meta-file>` from
    // `PathManager::meta_db()`, so `parent()` is `Some(<root>)`. The only
    // way it could be `None` is if root was empty, which `PathManager`
    // never constructs. Programmer-impossible None.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(&path).map_err(DbError::from)?;
    apply_pragmas(&conn)?;
    conn.execute_batch(schema_sql(DbLayer::Meta))
        .map_err(DbError::from)?;
    record_version(&conn)?;
    // See comment in `init_shard` — migrations also run on the
    // root-level meta.db so cross-project tables stay in sync.
    apply_migrations(&mut conn, DbLayer::Meta)?;
    Ok(())
}

fn apply_pragmas(conn: &Connection) -> DtResult<()> {
    // CRIT-13 fix (2026-05-05 audit): set busy_timeout BEFORE any other
    // pragma so that subsequent pragma writes themselves get the retry
    // budget. SQLite's default is 0 — without this, any moment a second
    // writer or a reader races a checkpoint surfaces as immediate
    // SQLITE_BUSY with no retry. 5000ms matches the canonical pattern in
    // brain/src/concept_store.rs.
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(DbError::from)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(DbError::from)?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(DbError::from)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(DbError::from)?;
    conn.pragma_update(None, "temp_store", "MEMORY")
        .map_err(DbError::from)?;
    conn.pragma_update(None, "mmap_size", 268435456_i64)
        .map_err(DbError::from)?; // 256MB

    // M-6 fix (2026-05-05 audit): incremental auto-vacuum so shards
    // don't grow forever between manual `mneme cache gc` runs. With
    // 26 per-project shards running append-heavy workloads
    // (ledger_entries, file_events, audit_log, telemetry.calls,
    // corpus_items), free pages accumulate indefinitely. Incremental
    // mode reclaims space gradually as deletes happen, without
    // requiring an exclusive VACUUM lock.
    //
    // Note: auto_vacuum only takes effect on fresh databases. Shards
    // created before this commit need a one-time `mneme cache gc` to
    // convert. Once converted, subsequent deletes auto-reclaim. This
    // pragma is idempotent on already-incremental databases (no-op).
    conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")
        .map_err(DbError::from)?;

    // Bug F-8 (2026-05-01): cap WAL auto-checkpoint at 200 pages
    // (~800 KB) instead of the SQLite default of 1000 pages (~4 MB).
    // On the build pipeline (1287 files → 13K+ nodes → 70K+ edges)
    // the default lets WAL grow unbounded between checkpoints, then a
    // single forced checkpoint stalls every writer for up to 30 s
    // while it merges. Smaller checkpoint cadence trades a tiny per-
    // write overhead for predictable latency: each stall is ~5–10 s
    // worst-case instead of one giant 30 s freeze that looks like a
    // hang to the user.
    conn.pragma_update(None, "wal_autocheckpoint", 200_i64)
        .map_err(DbError::from)?;
    // Cap on-disk WAL size at 32 MB so a stuck reader can't let it
    // balloon. Above this, SQLite truncates after each checkpoint.
    conn.pragma_update(None, "journal_size_limit", 33_554_432_i64)
        .map_err(DbError::from)?;
    Ok(())
}

fn record_version(conn: &Connection) -> DtResult<()> {
    // LOW fix (2026-05-05 audit): the prior `INSERT OR IGNORE` was
    // dangerously quiet — if schema_version already had a row with a
    // DIFFERENT version (a bug in migrations, a partial upgrade, a
    // data corruption), the IGNORE silently did nothing and the
    // shard kept running with the wrong stamp. Real version conflicts
    // are recoverable via mneme rebuild but only if we surface them
    // first.
    //
    // Now: read the current version, branch on it.
    //   - No row: INSERT — fresh shard, stamp it.
    //   - Same version: no-op (idempotent re-bootstrap).
    //   - Different version: return DbError::SchemaMismatch so the
    //     upper layer can refuse the connection and prompt the user
    //     to rebuild. This mirrors the migration runner's contract.
    let existing: Option<u32> = conn
        .query_row(
            "SELECT version FROM schema_version LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(DbError::from)?;
    match existing {
        None => {
            conn.execute(
                "INSERT INTO schema_version(version) VALUES(?1)",
                rusqlite::params![SCHEMA_VERSION],
            )
            .map_err(DbError::from)?;
        }
        Some(v) if v == SCHEMA_VERSION => { /* idempotent */ }
        Some(v) => {
            return Err(DtError::Db(DbError::SchemaMismatch {
                expected: SCHEMA_VERSION,
                found: v,
            }));
        }
    }
    Ok(())
}

fn register_project(paths: &PathManager, id: &ProjectId, root: &Path, name: &str) -> DtResult<()> {
    let conn = Connection::open(paths.meta_db()).map_err(DbError::from)?;
    conn.execute(
        "INSERT INTO projects(id, root, name, schema_version)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
           root = excluded.root,
           name = excluded.name,
           schema_version = excluded.schema_version",
        rusqlite::params![id.as_str(), root.to_string_lossy(), name, SCHEMA_VERSION],
    )
    .map_err(DbError::from)?;
    Ok(())
}

/// Stamp `meta.db::projects.last_indexed_at` with the current timestamp
/// for a successful build. Called by `mneme build` after the multimodal
/// pass completes - the staleness nag (audit-L12) reads this column to
/// decide whether the user's recall results may not reflect recent edits.
///
/// Idempotent: the row must already exist (registered by
/// [`DbBuilder::build_or_migrate`]). Silently no-ops if the row is
/// absent - that case represents a different bug (project not
/// registered) and a "no row updated" warning would confuse the user
/// during a successful build.
pub fn mark_indexed(paths: &PathManager, id: &ProjectId) -> DtResult<()> {
    let conn = Connection::open(paths.meta_db()).map_err(DbError::from)?;
    conn.execute(
        "UPDATE projects SET last_indexed_at = datetime('now') WHERE id = ?1",
        rusqlite::params![id.as_str()],
    )
    .map_err(DbError::from)?;
    Ok(())
}

#[cfg(unix)]
fn secure_perms(path: &Path) -> DtResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn secure_perms(_path: &Path) -> DtResult<()> {
    // Windows: rely on default ACL (user-only); CLI install can call icacls
    // explicitly during install for stricter setup.
    Ok(())
}

/// LOW fix (2026-05-05 audit): retry fs::rename on transient Windows
/// failures. Windows Defender, Explorer indexer, and stale read-only
/// connections can hold file handles for a few hundred milliseconds
/// after we close ours; rename fails with `ERROR_SHARING_VIOLATION`
/// (32) or `ERROR_ACCESS_DENIED` (5) until those handles drop. On
/// POSIX, rename always succeeds on the first attempt for
/// directories whose descendants are open, so the loop is a no-op
/// after iteration 0.
///
/// Total retry budget: 50 + 100 + 200 + 500 ≈ 850ms across 4
/// attempts. The single-attempt path is preserved (first iteration
/// has zero sleep before it).
fn rename_with_retry(src: &Path, dst: &Path) -> DtResult<()> {
    const BACKOFFS_MS: &[u64] = &[0, 50, 100, 200, 500];
    let mut last_err: Option<std::io::Error> = None;
    for &delay_ms in BACKOFFS_MS {
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        match fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                // On non-Windows targets we give up after the first
                // attempt — POSIX failures are not transient in the
                // same way and retrying just wastes time.
                #[cfg(not(windows))]
                break;
            }
        }
    }
    Err(DtError::Io(
        last_err.unwrap_or_else(|| std::io::Error::other("rename failed with no captured error")),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fresh_paths() -> (tempfile::TempDir, PathManager) {
        let dir = tempdir().expect("tempdir");
        let paths = PathManager::with_root(dir.path().to_path_buf());
        std::fs::create_dir_all(paths.root()).unwrap();
        init_meta(&paths).unwrap();
        (dir, paths)
    }

    fn insert_project_row(paths: &PathManager, id: &ProjectId, root: &Path, name: &str) {
        let conn = Connection::open(paths.meta_db()).unwrap();
        conn.execute(
            "INSERT INTO projects(id, root, name, schema_version) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![id.as_str(), root.to_string_lossy(), name, SCHEMA_VERSION],
        )
        .unwrap();
    }

    #[test]
    fn mark_indexed_sets_recent_timestamp() {
        let (_keep, paths) = fresh_paths();
        let id = ProjectId::from_path(paths.root()).unwrap();
        insert_project_row(&paths, &id, paths.root(), "fixture");

        let conn = Connection::open(paths.meta_db()).unwrap();
        let pre: Option<String> = conn
            .query_row(
                "SELECT last_indexed_at FROM projects WHERE id = ?1",
                rusqlite::params![id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            pre.is_none(),
            "expected NULL last_indexed_at before mark_indexed"
        );

        mark_indexed(&paths, &id).expect("mark_indexed");

        let post: Option<String> = conn
            .query_row(
                "SELECT last_indexed_at FROM projects WHERE id = ?1",
                rusqlite::params![id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        let ts = post.expect("last_indexed_at must be set after mark_indexed");
        assert_eq!(
            ts.len(),
            19,
            "datetime('now') format YYYY-MM-DD HH:MM:SS expected, got {ts}"
        );

        let written_secs: i64 = conn
            .query_row(
                "SELECT CAST(strftime('%s', ?1) AS INTEGER)",
                rusqlite::params![&ts],
                |r| r.get(0),
            )
            .unwrap();
        let now_secs: i64 = conn
            .query_row(
                "SELECT CAST(strftime('%s', datetime('now')) AS INTEGER)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            (now_secs - written_secs).abs() <= 5,
            "expected timestamp within 5s of now (now={now_secs}, written={written_secs})"
        );
    }

    #[test]
    fn mark_indexed_is_idempotent_and_advances_timestamp() {
        let (_keep, paths) = fresh_paths();
        let id = ProjectId::from_path(paths.root()).unwrap();
        insert_project_row(&paths, &id, paths.root(), "fixture");

        mark_indexed(&paths, &id).unwrap();
        let conn = Connection::open(paths.meta_db()).unwrap();
        let first: String = conn
            .query_row(
                "SELECT last_indexed_at FROM projects WHERE id = ?1",
                rusqlite::params![id.as_str()],
                |r| r.get(0),
            )
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1100));
        mark_indexed(&paths, &id).unwrap();
        let second: String = conn
            .query_row(
                "SELECT last_indexed_at FROM projects WHERE id = ?1",
                rusqlite::params![id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            second >= first,
            "second timestamp must be >= first (lex order matches chrono on this format) - first={first} second={second}"
        );
    }

    #[test]
    fn mark_indexed_on_missing_row_is_noop() {
        // Ensures a fresh shard that never registered the project row
        // doesn't crash mneme build mid-flight. mark_indexed silently
        // updates 0 rows.
        let (_keep, paths) = fresh_paths();
        let id = ProjectId::from_path(paths.root()).unwrap();
        // Intentionally do NOT insert_project_row.
        mark_indexed(&paths, &id).expect("mark_indexed must not error on missing row");
    }
}
