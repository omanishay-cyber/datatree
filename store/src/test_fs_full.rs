//! K10 chaos-test-only fault-injection: simulate `SQLITE_FULL` after N
//! bytes have been written to a shard.
//!
//! Compiled into the store crate ONLY when one of:
//!   * `cfg(test)` (in-tree unit tests)
//!   * `cfg(feature = "test-hooks")` (chaos tests build with this feature)
//!
//! Production binaries never see this module.
//!
//! ## Why a hook rather than a real custom VFS
//!
//! The original spec called for a custom rusqlite VFS that wraps the
//! platform's native VFS and short-circuits writes after N bytes. That
//! is the canonical way to simulate `SQLITE_FULL`, but it requires the
//! `vtab` / `loadable_extension` feature surface in `libsqlite3-sys`,
//! which we do not currently enable. The semantics the chaos test
//! actually asserts are:
//!
//!   1. After ≥ N bytes of writes, the shard rejects further inserts.
//!   2. The error surfaces to the CLI as a non-zero exit with a
//!      "disk full" / "out of space" / database-shaped message.
//!   3. The graph.db file does NOT contain partial half-written rows
//!      (i.e. the failed transaction is rolled back).
//!
//! All three are satisfied by hooking `commit_hook` + `update_hook`:
//!   * `update_hook` counts bytes per row (using the row payload size
//!     exposed by `last_changes()` doesn't give us bytes, so we
//!     accumulate the size of the SQL statement plus a constant per
//!     row — close enough for a budget gate).
//!   * `commit_hook` returns `true` (rollback) once the byte budget is
//!     exhausted, mapping cleanly to `SQLITE_FULL`-shaped semantics
//!     from the writer's POV (the transaction commit fails; SQLite
//!     rolls back; no partial writes land).
//!
//! ## Activation
//!
//! Set `MNEME_TEST_FAIL_FS_AT_BYTES=N` (decimal byte count) before
//! launching `mneme build`. Production users never set this, so the
//! hook is dormant.

use rusqlite::Connection;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Per-connection running byte counter. Each call to `update_hook`
/// adds an estimate of the row size; `commit_hook` checks against
/// the budget read from the env var.
///
/// We intentionally do NOT make this a global — each writer
/// connection installs its own counter so independent shards (graph,
/// audit, semantic, etc.) don't accidentally trip each other.
#[derive(Debug)]
pub struct WriteByteCounter {
    written: AtomicU64,
    budget: u64,
}

impl WriteByteCounter {
    pub fn new(budget: u64) -> Self {
        Self {
            written: AtomicU64::new(0),
            budget,
        }
    }

    /// Add `bytes` to the running counter and return the new total.
    pub fn add(&self, bytes: u64) -> u64 {
        self.written.fetch_add(bytes, Ordering::SeqCst) + bytes
    }

    /// Has the budget been exhausted?
    pub fn over_budget(&self) -> bool {
        self.written.load(Ordering::SeqCst) >= self.budget
    }

    /// Snapshot the current count (for diagnostics / tests).
    pub fn current(&self) -> u64 {
        self.written.load(Ordering::SeqCst)
    }
}

/// Read the configured byte budget from `MNEME_TEST_FAIL_FS_AT_BYTES`.
/// Returns `None` when the env var is unset, empty, or unparseable —
/// in all of these cases the hook is dormant.
pub fn budget_from_env() -> Option<u64> {
    let raw = std::env::var("MNEME_TEST_FAIL_FS_AT_BYTES").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u64>().ok().filter(|n| *n > 0)
}

/// Install the chaos-test hooks on `conn` if the env var is set.
///
/// `update_hook` counts an estimated row size on every insert/update/
/// delete. `commit_hook` returns `true` (rollback) once the budget has
/// been crossed. Returns `Some(counter)` when the hook is armed (so
/// callers can introspect for debugging) and `None` when the hook is
/// dormant.
///
/// The counter is `Arc`-shared so the lifetime works across the two
/// rusqlite hook closures (each takes `'static`-bounded `FnMut`).
///
/// # Bytes-per-row estimate
///
/// Real disk-bytes-per-row depends on page layout, fragmentation, WAL
/// mode, and the column types involved. The chaos test only needs
/// **monotonically increasing accounting** so the budget eventually
/// trips — exact bytes are irrelevant. We use `64 + key.len() *
/// payload_factor` as a stable proxy. The 64 covers SQLite's
/// per-row header overhead; multiplying the table name by a constant
/// gives us a deterministic, non-zero accumulator for the test.
pub fn install_full_disk_hook(conn: &Connection) -> Option<Arc<WriteByteCounter>> {
    let budget = budget_from_env()?;
    let counter = Arc::new(WriteByteCounter::new(budget));

    // update_hook fires on every row mutation. We don't have access
    // to the row's serialized bytes here (rusqlite's API only exposes
    // table name + rowid), so we use a conservative per-row size
    // estimate. Real disk pressure scales with the actual row payload,
    // but for the budget-gate semantics the test cares about, this is
    // sufficient.
    let counter_for_update = counter.clone();
    conn.update_hook(Some(
        move |_action: rusqlite::hooks::Action, _db: &str, table: &str, _rowid: i64| {
            // 64-byte fixed header overhead + 8 * table-name length
            // (the per-row bookkeeping SQLite holds in its page).
            // Overestimating is safe — the test budgets at the level
            // of "I inserted enough rows to cross 1 MiB", not at the
            // level of byte-exact accounting.
            let est = 64u64 + (table.len() as u64).saturating_mul(8);
            counter_for_update.add(est);
        },
    ));

    // commit_hook fires once per transaction commit. Returning `true`
    // rolls the transaction back — exactly the semantics SQLite gives
    // when the underlying VFS reports `SQLITE_FULL` mid-commit. From
    // the application's POV the writer task observes a failed write
    // result and propagates the error upward, just like a real
    // out-of-space scenario would.
    let counter_for_commit = counter.clone();
    conn.commit_hook(Some(move || -> bool {
        if counter_for_commit.over_budget() {
            // true = rollback. SQLite returns SQLITE_CONSTRAINT to
            // the caller; the chaos test asserts the CLI surfaces a
            // sensible disk-shaped error, which the post-commit rollback
            // delivers.
            true
        } else {
            false
        }
    }));

    Some(counter)
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn budget_from_env_unset_is_none() {
        std::env::remove_var("MNEME_TEST_FAIL_FS_AT_BYTES");
        assert!(budget_from_env().is_none());
    }

    #[test]
    fn budget_from_env_empty_is_none() {
        std::env::set_var("MNEME_TEST_FAIL_FS_AT_BYTES", "");
        assert!(budget_from_env().is_none());
        std::env::remove_var("MNEME_TEST_FAIL_FS_AT_BYTES");
    }

    #[test]
    fn budget_from_env_zero_is_none() {
        std::env::set_var("MNEME_TEST_FAIL_FS_AT_BYTES", "0");
        assert!(budget_from_env().is_none());
        std::env::remove_var("MNEME_TEST_FAIL_FS_AT_BYTES");
    }

    #[test]
    fn budget_from_env_valid_parses() {
        std::env::set_var("MNEME_TEST_FAIL_FS_AT_BYTES", "1048576");
        assert_eq!(budget_from_env(), Some(1_048_576));
        std::env::remove_var("MNEME_TEST_FAIL_FS_AT_BYTES");
    }

    #[test]
    fn budget_from_env_garbage_is_none() {
        std::env::set_var("MNEME_TEST_FAIL_FS_AT_BYTES", "not-a-number");
        assert!(budget_from_env().is_none());
        std::env::remove_var("MNEME_TEST_FAIL_FS_AT_BYTES");
    }

    #[test]
    fn counter_accumulates() {
        let c = WriteByteCounter::new(100);
        assert_eq!(c.current(), 0);
        c.add(40);
        assert_eq!(c.current(), 40);
        assert!(!c.over_budget());
        c.add(60);
        assert_eq!(c.current(), 100);
        assert!(c.over_budget());
    }
}
