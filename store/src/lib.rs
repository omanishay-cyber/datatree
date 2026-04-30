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
pub mod inject;
pub mod ipc;
pub mod lifecycle;
pub mod query;
pub mod schema;

/// K10 chaos-test-only fault-injection: simulate `SQLITE_FULL` after N
/// bytes of writes via `MNEME_TEST_FAIL_FS_AT_BYTES`. Compiled out of
/// release binaries by default — the entire module is gated behind
/// `#[cfg]` so the hook installation site doesn't even exist in
/// user-facing builds.
#[cfg(any(test, feature = "test-hooks"))]
pub mod test_fs_full;

pub use builder::{mark_indexed, DbBuilder, DefaultBuilder};
pub use finder::{DbFinder, DefaultFinder};
pub use inject::{BatchResult, DbInject, DefaultInject, InjectOp, InjectOptions, UpsertResult};
pub use lifecycle::{
    ArchiveMeta, DbLifecycle, DefaultLifecycle, IntegrityReport, MigrationReport, PurgeToken,
    SnapshotMeta, VacuumReport,
};
pub use query::{BatchSummary, DbQuery, DefaultQuery, Query, Write, WriteSummary};

use std::sync::Arc;

use common::PathManager;
use parking_lot::Mutex;
use tokio::sync::oneshot;

/// Top-level handle. Workers acquire this once at boot and use it for
/// all DB ops.
///
/// `Store::new` creates the handle. Bind a graceful-shutdown signal once
/// at startup via [`Store::bind_shutdown`], then `select!` on
/// [`Store::shutdown_signal`] in the binary's main loop. The IPC
/// `Request::Shutdown` handler triggers the signal cooperatively
/// instead of calling `std::process::exit` mid-handler (WIDE-010).
#[derive(Clone)]
pub struct Store {
    pub paths: Arc<PathManager>,
    pub builder: Arc<dyn DbBuilder + Send + Sync>,
    pub finder: Arc<dyn DbFinder + Send + Sync>,
    pub query: Arc<dyn DbQuery + Send + Sync>,
    pub inject: Arc<dyn DbInject + Send + Sync>,
    pub lifecycle: Arc<dyn DbLifecycle + Send + Sync>,
    /// One-shot sender used by the IPC `Shutdown` handler. Wrapped in
    /// `Arc<Mutex<Option<...>>>` so it can be `take()`-n once across any
    /// number of cloned `Store` handles. `None` if no main loop has bound
    /// itself yet.
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl Store {
    /// Construct a new store with all sub-layers wired up.
    ///
    /// Arguments:
    /// * `paths` — fully resolved [`PathManager`] (callers typically use
    ///   `PathManager::default_root()` or build one from a CLI override).
    ///
    /// Returns a ready-to-use [`Store`] handle. Cloning is cheap — each
    /// sub-layer is `Arc`-wrapped, so workers share the same backing
    /// resources.
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
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Bind a one-shot shutdown channel. The returned receiver should be
    /// `select!`-ed on by the binary's main loop. The matching sender
    /// is stored in the [`Store`] and consumed by the IPC `Shutdown`
    /// handler (WIDE-010).
    ///
    /// Calling this more than once replaces the previous sender. The
    /// previously-bound receiver (if any) is dropped, so its caller
    /// observes a closed channel and should treat that as "shutdown".
    pub fn bind_shutdown(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        *self.shutdown_tx.lock() = Some(tx);
        rx
    }

    /// Trigger the bound shutdown signal, if any. Returns `true` when a
    /// signal was actually sent (i.e. a receiver was bound and still
    /// alive); `false` otherwise. Idempotent — second and later calls
    /// are no-ops.
    pub fn trigger_shutdown(&self) -> bool {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            tx.send(()).is_ok()
        } else {
            false
        }
    }
}
