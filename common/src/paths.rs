use std::path::{Path, PathBuf};

use crate::ids::{ProjectId, SnapshotId};
use crate::layer::DbLayer;
use crate::time::Timestamp;

/// Single source of truth for every datatree path. No other module
/// constructs paths manually.
#[derive(Debug, Clone)]
pub struct PathManager {
    root: PathBuf,
}

impl PathManager {
    /// Default install root: `~/.datatree/`.
    pub fn default_root() -> Self {
        let home = dirs::home_dir().expect("no home dir");
        PathManager {
            root: home.join(".datatree"),
        }
    }

    pub fn with_root(root: PathBuf) -> Self {
        PathManager { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn meta_db(&self) -> PathBuf {
        self.root.join(DbLayer::Meta.file_name())
    }

    pub fn project_root(&self, p: &ProjectId) -> PathBuf {
        self.root.join("projects").join(p.as_str())
    }

    pub fn shard_db(&self, p: &ProjectId, layer: DbLayer) -> PathBuf {
        debug_assert!(layer != DbLayer::Meta, "Meta is global, not per-project");
        self.project_root(p).join(layer.file_name())
    }

    pub fn wal_path(&self, p: &ProjectId, layer: DbLayer) -> PathBuf {
        let mut p = self.shard_db(p, layer);
        let stem = p.file_name().unwrap().to_owned();
        p.set_file_name(format!("{}-wal", stem.to_string_lossy()));
        p
    }

    pub fn snapshot_dir(&self, p: &ProjectId) -> PathBuf {
        self.project_root(p).join("snapshots")
    }

    pub fn snapshot_at(&self, p: &ProjectId, ts: &Timestamp) -> PathBuf {
        self.snapshot_dir(p).join(ts.as_dirname())
    }

    pub fn snapshot_by_id(&self, p: &ProjectId, id: &SnapshotId) -> PathBuf {
        self.snapshot_dir(p).join(id.as_str())
    }

    pub fn cache_root(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn docs_cache(&self) -> PathBuf {
        self.cache_root().join("docs")
    }

    pub fn embed_cache(&self) -> PathBuf {
        self.cache_root().join("embed")
    }

    pub fn multimodal_cache(&self) -> PathBuf {
        self.cache_root().join("multimodal")
    }

    pub fn llm_dir(&self) -> PathBuf {
        self.root.join("llm")
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn crash_dir(&self) -> PathBuf {
        self.root.join("crashes")
    }

    pub fn supervisor_log(&self) -> PathBuf {
        self.root.join("supervisor.log")
    }

    pub fn supervisor_socket(&self) -> PathBuf {
        #[cfg(windows)]
        {
            // Named pipe path; interprocess crate maps appropriately.
            PathBuf::from(r"\\.\pipe\datatree-supervisor")
        }
        #[cfg(not(windows))]
        {
            self.root.join("supervisor.sock")
        }
    }

    pub fn livebus_socket(&self) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(r"\\.\pipe\datatree-livebus")
        }
        #[cfg(not(windows))]
        {
            self.root.join("livebus.sock")
        }
    }
}

impl Default for PathManager {
    fn default() -> Self {
        Self::default_root()
    }
}
