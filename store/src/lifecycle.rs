//! Sub-layer 7: LIFECYCLE — backup, restore, snapshot, migrate, vacuum, repair, archive, purge.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rusqlite::{backup::Backup, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use common::{
    error::{DbError, DtError, DtResult},
    ids::{ProjectId, SnapshotId},
    layer::DbLayer,
    paths::PathManager,
    time::Timestamp,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: SnapshotId,
    pub captured_at: Timestamp,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub from_version: u32,
    pub to_version: u32,
    pub layers_migrated: Vec<DbLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VacuumReport {
    pub bytes_reclaimed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub layer: DbLayer,
    pub ok: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMeta {
    pub project: ProjectId,
    pub bytes: u64,
    pub destination: PathBuf,
}

/// Token returned by `purge_request` and required by `purge`. Prevents
/// accidental data loss from a single mistaken call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeToken(pub String);

#[async_trait]
pub trait DbLifecycle {
    async fn snapshot(&self, project: &ProjectId) -> DtResult<SnapshotId>;
    async fn restore(&self, project: &ProjectId, snapshot: SnapshotId) -> DtResult<()>;
    async fn list_snapshots(&self, project: &ProjectId) -> DtResult<Vec<SnapshotMeta>>;

    async fn migrate(&self, project: &ProjectId, target_version: u32) -> DtResult<MigrationReport>;
    async fn vacuum(&self, project: &ProjectId) -> DtResult<VacuumReport>;
    async fn integrity_check(&self, project: &ProjectId) -> DtResult<Vec<IntegrityReport>>;
    async fn repair(&self, project: &ProjectId, reports: Vec<IntegrityReport>) -> DtResult<()>;

    async fn archive(&self, project: &ProjectId, dest: PathBuf) -> DtResult<ArchiveMeta>;
    async fn purge_request(&self, project: &ProjectId) -> DtResult<PurgeToken>;
    async fn purge(&self, project: &ProjectId, token: PurgeToken) -> DtResult<()>;
}

pub struct DefaultLifecycle {
    paths: Arc<PathManager>,
}

impl DefaultLifecycle {
    pub fn new(paths: Arc<PathManager>) -> Self {
        Self { paths }
    }
}

#[async_trait]
impl DbLifecycle for DefaultLifecycle {
    async fn snapshot(&self, project: &ProjectId) -> DtResult<SnapshotId> {
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<SnapshotId> {
            let ts = Timestamp::now();
            let snap_dir = paths.snapshot_dir(&project).join(ts.as_dirname());
            fs::create_dir_all(&snap_dir)?;
            for layer in DbLayer::all_per_project() {
                let src = paths.shard_db(&project, *layer);
                if !src.exists() {
                    continue;
                }
                let dst = snap_dir.join(layer.file_name());
                online_backup(&src, &dst)?;
            }
            info!(project = %project, dir = %snap_dir.display(), "snapshot complete");
            Ok(SnapshotId::from_str(ts.as_dirname()))
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn restore(&self, project: &ProjectId, snapshot: SnapshotId) -> DtResult<()> {
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<()> {
            let snap_dir = paths.snapshot_by_id(&project, &snapshot);
            if !snap_dir.exists() {
                return Err(DtError::Validation(format!(
                    "snapshot not found: {}",
                    snapshot
                )));
            }
            for layer in DbLayer::all_per_project() {
                let src = snap_dir.join(layer.file_name());
                if !src.exists() {
                    continue;
                }
                let dst = paths.shard_db(&project, *layer);
                if dst.exists() {
                    let bak = dst
                        .with_extension(format!("pre-restore.{}", Timestamp::now().as_dirname()));
                    fs::rename(&dst, &bak)?;
                }
                fs::copy(&src, &dst)?;
            }
            warn!(project = %project, snapshot = %snapshot, "restored from snapshot");
            Ok(())
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn list_snapshots(&self, project: &ProjectId) -> DtResult<Vec<SnapshotMeta>> {
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<Vec<SnapshotMeta>> {
            let dir = paths.snapshot_dir(&project);
            if !dir.exists() {
                return Ok(vec![]);
            }
            let mut out = vec![];
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let id_str = entry.file_name().to_string_lossy().to_string();
                let mut bytes = 0u64;
                for sub in fs::read_dir(entry.path())? {
                    let sub = sub?;
                    if let Ok(m) = sub.metadata() {
                        bytes += m.len();
                    }
                }
                out.push(SnapshotMeta {
                    id: SnapshotId::from_str(id_str),
                    captured_at: Timestamp::now(),
                    bytes,
                });
            }
            out.sort_by(|a, b| b.id.as_str().cmp(a.id.as_str()));
            Ok(out)
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn migrate(
        &self,
        _project: &ProjectId,
        target_version: u32,
    ) -> DtResult<MigrationReport> {
        // v1 → v1 noop. Future versions: dispatch per-layer migration scripts.
        Ok(MigrationReport {
            from_version: target_version,
            to_version: target_version,
            layers_migrated: vec![],
        })
    }

    async fn vacuum(&self, project: &ProjectId) -> DtResult<VacuumReport> {
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<VacuumReport> {
            let mut reclaimed = 0i64;
            for layer in DbLayer::all_per_project() {
                let p = paths.shard_db(&project, *layer);
                if !p.exists() {
                    continue;
                }
                let before = fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0);
                let conn = Connection::open(&p).map_err(DbError::from)?;
                // DB-4 fix (2026-05-05 audit): set busy_timeout BEFORE
                // VACUUM. VACUUM needs an exclusive lock; without the
                // timeout, any concurrent reader/writer activity
                // surfaces as instant SQLITE_BUSY and the lifecycle
                // operation fails. 5000ms matches the canonical pragma
                // block in builder.rs::apply_pragmas.
                conn.busy_timeout(std::time::Duration::from_millis(5000))
                    .map_err(DbError::from)?;
                conn.execute("VACUUM", []).map_err(DbError::from)?;
                let after = fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0);
                reclaimed += before - after;
            }
            Ok(VacuumReport {
                bytes_reclaimed: reclaimed,
            })
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn integrity_check(&self, project: &ProjectId) -> DtResult<Vec<IntegrityReport>> {
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<Vec<IntegrityReport>> {
            let mut out = vec![];
            for layer in DbLayer::all_per_project() {
                let p = paths.shard_db(&project, *layer);
                if !p.exists() {
                    continue;
                }
                let conn = Connection::open_with_flags(&p, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(DbError::from)?;
                let mut stmt = conn
                    .prepare("PRAGMA integrity_check")
                    .map_err(DbError::from)?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(DbError::from)?;
                let mut errors = vec![];
                for row in rows {
                    let s = row.map_err(DbError::from)?;
                    if s != "ok" {
                        errors.push(s);
                    }
                }
                out.push(IntegrityReport {
                    layer: *layer,
                    ok: errors.is_empty(),
                    errors,
                });
            }
            Ok(out)
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn repair(&self, project: &ProjectId, reports: Vec<IntegrityReport>) -> DtResult<()> {
        // Strategy:
        // 1. For each broken layer, attempt VACUUM + reindex.
        // 2. If still broken, try restore from latest snapshot.
        let paths = self.paths.clone();
        let project = project.clone();
        let snapshots = self.list_snapshots(&project).await?;

        tokio::task::spawn_blocking(move || -> DtResult<()> {
            for report in reports {
                if report.ok {
                    continue;
                }
                let p = paths.shard_db(&project, report.layer);
                let conn = Connection::open(&p).map_err(DbError::from)?;
                // DB-4 fix: same busy_timeout as VACUUM path above.
                // REINDEX also takes an exclusive lock and would race
                // a concurrent reader without this.
                let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
                let _ = conn.execute("VACUUM", []);
                let _ = conn.execute("REINDEX", []);
                drop(conn);

                // Verify
                let conn = Connection::open_with_flags(&p, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(DbError::from)?;
                let still_broken: bool = conn
                    .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                    .map(|s| s != "ok")
                    .unwrap_or(true);

                if still_broken {
                    if let Some(latest) = snapshots.first() {
                        let src = paths
                            .snapshot_by_id(&project, &latest.id)
                            .join(report.layer.file_name());
                        if src.exists() {
                            let bak = p.with_extension(format!(
                                "corrupt.{}",
                                Timestamp::now().as_dirname()
                            ));
                            fs::rename(&p, &bak).ok();
                            fs::copy(&src, &p)?;
                            warn!(layer = ?report.layer, "restored from snapshot {}", latest.id);
                        }
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn archive(&self, project: &ProjectId, dest: PathBuf) -> DtResult<ArchiveMeta> {
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<ArchiveMeta> {
            let src = paths.project_root(&project);
            if !src.exists() {
                return Err(DtError::Validation("project not found".into()));
            }
            fs::create_dir_all(dest.parent().unwrap_or_else(|| std::path::Path::new(".")))?;
            // Simple copy archive (caller can tar/zip externally).
            fs_copy_recursive(&src, &dest)?;
            let bytes = walkdir::WalkDir::new(&dest)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum();
            Ok(ArchiveMeta {
                project,
                bytes,
                destination: dest,
            })
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn purge_request(&self, project: &ProjectId) -> DtResult<PurgeToken> {
        // Token bound to project + 5min validity (caller checks).
        let token = format!("{}:{}", project, Timestamp::now().as_unix_millis());
        Ok(PurgeToken(token))
    }

    async fn purge(&self, project: &ProjectId, token: PurgeToken) -> DtResult<()> {
        if !token.0.starts_with(project.as_str()) {
            return Err(DtError::Validation("token does not match project".into()));
        }
        let paths = self.paths.clone();
        let project = project.clone();
        tokio::task::spawn_blocking(move || -> DtResult<()> {
            let p = paths.project_root(&project);
            if p.exists() {
                fs::remove_dir_all(&p)?;
                warn!(project = %project, "purged");
            }
            Ok(())
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }
}

fn online_backup(src: &std::path::Path, dst: &std::path::Path) -> DtResult<()> {
    let src_conn = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(DbError::from)?;
    let mut dst_conn = Connection::open(dst).map_err(DbError::from)?;
    let backup = Backup::new(&src_conn, &mut dst_conn).map_err(DbError::from)?;
    backup
        .run_to_completion(100, std::time::Duration::from_millis(0), None)
        .map_err(DbError::from)?;
    Ok(())
}

fn fs_copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs_copy_recursive(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}
