use std::path::{Path, PathBuf};

use crate::error::DtResult;
#[cfg(not(any(unix, windows)))]
use crate::error::DtError;
use crate::ids::{ProjectId, SnapshotId};
use crate::layer::DbLayer;
use crate::time::Timestamp;

/// Single source of truth for every mneme path. No other module
/// constructs paths manually.
#[derive(Debug, Clone)]
pub struct PathManager {
    root: PathBuf,
}

impl PathManager {
    /// Resolve the default mneme root path with a fallback chain.
    ///
    /// Resolution order (WIDE-011):
    ///   1. `MNEME_HOME` environment variable (operator override).
    ///   2. `dirs::home_dir().join(".mneme")` (the historical default).
    ///   3. OS-level fallback:
    ///      * Unix: `/var/lib/mneme`
    ///      * Windows: `%PROGRAMDATA%\mneme` (then `C:\ProgramData\mneme`).
    ///
    /// Returns the final `PathBuf` so callers can handle the (extremely
    /// unlikely) case where every fallback fails — for example, a fully
    /// stripped sandbox with no `HOME`, no `MNEME_HOME`, and no
    /// `%PROGRAMDATA%`. In that case [`DtError::Internal`] is returned.
    #[allow(clippy::needless_return)]
    pub fn resolve_default_root() -> DtResult<PathBuf> {
        // 1. Explicit env override.
        if let Some(p) = std::env::var("MNEME_HOME").ok().map(PathBuf::from) {
            if !p.as_os_str().is_empty() {
                return Ok(p);
            }
        }

        // 2. User home dir.
        if let Some(home) = dirs::home_dir() {
            return Ok(home.join(".mneme"));
        }

        // 3. OS default.
        #[cfg(unix)]
        {
            return Ok(PathBuf::from("/var/lib/mneme"));
        }
        #[cfg(windows)]
        {
            if let Some(pd) = std::env::var_os("PROGRAMDATA") {
                let mut p = PathBuf::from(pd);
                p.push("mneme");
                return Ok(p);
            }
            return Ok(PathBuf::from(r"C:\ProgramData\mneme"));
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(DtError::Internal(
                "no usable default mneme root: no MNEME_HOME, no home dir, and no OS fallback"
                    .into(),
            ))
        }
    }

    /// Default install root: tries `MNEME_HOME`, then `~/.mneme`, then
    /// an OS default (`/var/lib/mneme` on Unix, `%PROGRAMDATA%\mneme` on
    /// Windows). Infallible — the OS fallback always succeeds on a
    /// supported target so this function never panics.
    ///
    /// Prefer [`PathManager::try_default_root`] when you want to surface
    /// a structured error in a user-facing CLI flow.
    pub fn default_root() -> Self {
        let root = Self::resolve_default_root()
            .unwrap_or_else(|_| PathBuf::from(".mneme"));
        PathManager { root }
    }

    /// Fallible variant of [`PathManager::default_root`]. Returns a
    /// structured [`DtError`] when no fallback succeeds (extremely rare).
    pub fn try_default_root() -> DtResult<Self> {
        let root = Self::resolve_default_root()?;
        Ok(PathManager { root })
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
        // SAFETY: `shard_db` always returns a path of the form
        // `<root>/projects/<id>/<layer-file>`; `<layer-file>` is a non-empty
        // constant from `DbLayer::file_name()`, so `file_name()` is always
        // `Some(_)`. Programmer-impossible None.
        let stem = p.file_name().expect("shard_db result always has a file_name").to_owned();
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
            PathBuf::from(r"\\.\pipe\mneme-supervisor")
        }
        #[cfg(not(windows))]
        {
            self.root.join("supervisor.sock")
        }
    }

    pub fn livebus_socket(&self) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(r"\\.\pipe\mneme-livebus")
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
