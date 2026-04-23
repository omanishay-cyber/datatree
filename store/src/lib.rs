//! Database Operations Layer (DOL) for mneme.
//!
//! Implements the 7 sub-layers from design §13.5:
//!   1. Builder        — creates project shards
//!   2. Finder         — resolves any input → shard
//!   3. PathManager    — single source of truth for filesystem paths
//!   4. Query          — typed reads + single-writer-per-shard writes
//!   5. Response       — uniform envelope for every op (re-exported from common)
//!   6. Inject         — typed insert/update/delete with idempotency, audit, events
//!   7. Lifecycle      — snapshot, restore, migrate, vacuum, repair, archive, purge
//!
//! All other mneme workers talk to a single `Store` instance via the
//! supervisor's IPC. They never construct paths or open SQLite files
//! themselves.

pub mod builder;
pub mod finder;
pub mod query;
pub mod inject;
pub mod lifecycle;
pub mod schema;
pub mod ipc;

pub use builder::{DbBuilder, DefaultBuilder};
pub use finder::{DbFinder, DefaultFinder};
pub use query::{DbQuery, Query, Write, WriteSummary, BatchSummary, DefaultQuery};
pub use inject::{DbInject, InjectOp, InjectOptions, UpsertResult, BatchResult, DefaultInject};
pub use lifecycle::{
    DbLifecycle, SnapshotMeta, MigrationReport, VacuumReport, IntegrityReport, ArchiveMeta,
    PurgeToken, DefaultLifecycle,
};

use std::sync::Arc;

use common::{PathManager};

/// Top-level handle. Workers acquire this once at boot and use it for
/// all DB ops.
#[derive(Clone)]
pub struct Store {
    pub paths: Arc<PathManager>,
    pub builder: Arc<dyn DbBuilder + Send + Sync>,
    pub finder: Arc<dyn DbFinder + Send + Sync>,
    pub query: Arc<dyn DbQuery + Send + Sync>,
    pub inject: Arc<dyn DbInject + Send + Sync>,
    pub lifecycle: Arc<dyn DbLifecycle + Send + Sync>,
}

impl Store {
    pub fn new(paths: PathManager) -> Self {
        let paths = Arc::new(paths);
        let builder = Arc::new(DefaultBuilder::new(paths.clone()));
        let finder = Arc::new(DefaultFinder::new(paths.clone()));
        let query = Arc::new(DefaultQuery::new(paths.clone()));
        let inject = Arc::new(DefaultInject::new(paths.clone(), query.clone()));
        let lifecycle = Arc::new(DefaultLifecycle::new(paths.clone()));
        Self {
            paths,
            builder,
            finder,
            query,
            inject,
            lifecycle,
        }
    }
}
