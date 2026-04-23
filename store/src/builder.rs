//! Sub-layer 1: BUILDER — creates per-project shard trees.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use rusqlite::Connection;
use tracing::{debug, info, warn};

use common::{
    error::{DbError, DtError, DtResult},
    ids::ProjectId,
    layer::DbLayer,
    paths::PathManager,
    project::{Project, ShardHandle},
    time::Timestamp,
};

use crate::schema::{schema_sql, SCHEMA_VERSION};

#[async_trait]
pub trait DbBuilder {
    async fn build_or_migrate(&self, project: &ProjectId, root: &Path, name: &str)
        -> DtResult<ShardHandle>;
    async fn rebuild(&self, project: &ProjectId, archive: bool) -> DtResult<ShardHandle>;
    async fn exists_and_current(&self, project: &ProjectId) -> DtResult<bool>;
}

pub struct DefaultBuilder {
    paths: Arc<PathManager>,
}

impl DefaultBuilder {
    pub fn new(paths: Arc<PathManager>) -> Self { Self { paths } }
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
                let archived = project_dir.with_extension(format!(
                    "archived.{}",
                    Timestamp::now().as_dirname()
                ));
                fs::rename(&project_dir, &archived)?;
                warn!("archived {} -> {}", project_dir.display(), archived.display());
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
                    .query_row(
                        "SELECT MAX(version) FROM schema_version",
                        [],
                        |r| r.get(0),
                    )
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
    let conn = Connection::open(&path).map_err(DbError::from)?;
    apply_pragmas(&conn)?;
    conn.execute_batch(schema_sql(layer)).map_err(DbError::from)?;
    record_version(&conn)?;
    if !pre_existed {
        info!(layer = ?layer, path = %path.display(), "created shard");
    } else {
        debug!(layer = ?layer, "shard already present");
    }
    Ok(())
}

fn init_meta(paths: &PathManager) -> DtResult<()> {
    let path = paths.meta_db();
    fs::create_dir_all(path.parent().unwrap())?;
    let conn = Connection::open(&path).map_err(DbError::from)?;
    apply_pragmas(&conn)?;
    conn.execute_batch(schema_sql(DbLayer::Meta)).map_err(DbError::from)?;
    record_version(&conn)?;
    Ok(())
}

fn apply_pragmas(conn: &Connection) -> DtResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL").map_err(DbError::from)?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(DbError::from)?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(DbError::from)?;
    conn.pragma_update(None, "temp_store", "MEMORY").map_err(DbError::from)?;
    conn.pragma_update(None, "mmap_size", 268435456_i64).map_err(DbError::from)?; // 256MB
    Ok(())
}

fn record_version(conn: &Connection) -> DtResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_version(version) VALUES(?1)",
        rusqlite::params![SCHEMA_VERSION],
    )
    .map_err(DbError::from)?;
    Ok(())
}

fn register_project(
    paths: &PathManager,
    id: &ProjectId,
    root: &Path,
    name: &str,
) -> DtResult<()> {
    let conn = Connection::open(paths.meta_db()).map_err(DbError::from)?;
    conn.execute(
        "INSERT INTO projects(id, root, name, schema_version)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
           root = excluded.root,
           name = excluded.name,
           schema_version = excluded.schema_version",
        rusqlite::params![
            id.as_str(),
            root.to_string_lossy(),
            name,
            SCHEMA_VERSION
        ],
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
