//! Sub-layer 2: FINDER — resolves any input → ShardHandle.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use rusqlite::Connection;
use tracing::debug;

use datatree_common::{
    error::{DbError, DtError, DtResult},
    ids::ProjectId,
    paths::PathManager,
    project::{Project, ShardHandle},
    time::Timestamp,
};

#[async_trait]
pub trait DbFinder {
    async fn find_by_path(&self, project_root: &Path) -> DtResult<Option<ShardHandle>>;
    async fn find_by_cwd(&self, cwd: &Path) -> DtResult<Option<ShardHandle>>;
    async fn find_by_hash(&self, hash: &str) -> DtResult<Option<ShardHandle>>;
    async fn find_by_file(&self, file: &Path) -> DtResult<Option<ShardHandle>>;
    async fn list_all(&self) -> DtResult<Vec<ShardHandle>>;
}

pub struct DefaultFinder {
    paths: Arc<PathManager>,
}

impl DefaultFinder {
    pub fn new(paths: Arc<PathManager>) -> Self { Self { paths } }
}

#[async_trait]
impl DbFinder for DefaultFinder {
    async fn find_by_path(&self, project_root: &Path) -> DtResult<Option<ShardHandle>> {
        let id = ProjectId::from_path(project_root)?;
        self.find_by_hash(id.as_str()).await
    }

    async fn find_by_cwd(&self, cwd: &Path) -> DtResult<Option<ShardHandle>> {
        // Walk up to find a project root marker.
        let mut cur = Some(cwd.to_path_buf());
        while let Some(dir) = cur {
            if is_project_root(&dir) {
                return self.find_by_path(&dir).await;
            }
            cur = dir.parent().map(Path::to_path_buf);
        }
        Ok(None)
    }

    async fn find_by_hash(&self, hash: &str) -> DtResult<Option<ShardHandle>> {
        let paths = self.paths.clone();
        let hash = hash.to_string();
        tokio::task::spawn_blocking(move || -> DtResult<Option<ShardHandle>> {
            let conn = Connection::open(paths.meta_db()).map_err(DbError::from)?;
            let row = conn.query_row(
                "SELECT id, root, name, created_at, last_indexed_at, schema_version
                 FROM projects WHERE id = ?1",
                rusqlite::params![hash],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, u32>(5)?,
                    ))
                },
            );
            match row {
                Ok((id, root, name, _created_at, _last_indexed, schema_version)) => Ok(Some(ShardHandle {
                    project: Project {
                        id: ProjectId::from_hash(id),
                        root: PathBuf::from(root),
                        name,
                        created_at: Timestamp::now(),
                        last_indexed_at: None,
                        schema_version,
                    },
                })),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DtError::Db(DbError::from(e))),
            }
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }

    async fn find_by_file(&self, file: &Path) -> DtResult<Option<ShardHandle>> {
        if let Some(parent) = file.parent() {
            self.find_by_cwd(parent).await
        } else {
            Ok(None)
        }
    }

    async fn list_all(&self) -> DtResult<Vec<ShardHandle>> {
        let paths = self.paths.clone();
        tokio::task::spawn_blocking(move || -> DtResult<Vec<ShardHandle>> {
            let conn = Connection::open(paths.meta_db()).map_err(DbError::from)?;
            let mut stmt = conn
                .prepare("SELECT id, root, name, schema_version FROM projects ORDER BY name")
                .map_err(DbError::from)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(ShardHandle {
                        project: Project {
                            id: ProjectId::from_hash(r.get::<_, String>(0)?),
                            root: PathBuf::from(r.get::<_, String>(1)?),
                            name: r.get::<_, String>(2)?,
                            created_at: Timestamp::now(),
                            last_indexed_at: None,
                            schema_version: r.get(3)?,
                        },
                    })
                })
                .map_err(DbError::from)?;
            let mut out = vec![];
            for row in rows {
                out.push(row.map_err(DbError::from)?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| DtError::Internal(format!("join: {}", e)))?
    }
}

fn is_project_root(dir: &Path) -> bool {
    let markers = [".git", ".claude", "package.json", "Cargo.toml", "pyproject.toml"];
    for m in markers {
        if dir.join(m).exists() {
            debug!(marker = m, dir = %dir.display(), "matched project root");
            return true;
        }
    }
    false
}
