//! Persistent concept store (v0.4 Wave 3.3).
//!
//! Concepts extracted or recalled by the brain worker are stored in a
//! per-project SQLite shard (`concepts.db`) so they survive daemon restarts.
//! Prior to v0.4.0, this data lived only in a HashMap that was discarded on
//! every restart — the "seed concept memory" gap from the v0.4 vision plan.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE concepts (
//!     id          TEXT PRIMARY KEY,   -- stable hash: sha256(name || project_id)[:16]
//!     project_id  TEXT NOT NULL,
//!     name        TEXT NOT NULL,
//!     kind        TEXT NOT NULL,      -- "pattern" | "anti-pattern" | "convention" | ...
//!     description TEXT,
//!     examples    TEXT,              -- JSON array of code snippets
//!     score       REAL DEFAULT 0.5, -- 0.0..1.0 confidence
//!     first_seen  INTEGER NOT NULL,  -- unix epoch (seconds)
//!     last_used   INTEGER NOT NULL,
//!     use_count   INTEGER DEFAULT 1
//! );
//! ```
//!
//! # Concurrency
//!
//! WAL mode is enabled on open so concurrent readers (MCP tools calling
//! `recall_concept`) do not block the daemon writer. The connection is wrapped
//! in a `Mutex` to enforce single-writer semantics within a process — SQLite
//! itself serialises cross-process writes via its WAL locking protocol.
//!
//! # Degraded mode
//!
//! All public methods return `BrainResult<_>`. On any SQLite error the caller
//! gets a `BrainError::ConceptStore(msg)` — never a panic. Callers that want
//! best-effort behaviour can log the error and continue with an empty result.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::error::{BrainError, BrainResult};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One persisted concept entry.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredConcept {
    /// Stable, deterministic primary key: `sha256(name + project_id)[:16]` hex.
    pub id: String,
    pub project_id: String,
    pub name: String,
    /// Free-form category tag, e.g. `"pattern"`, `"anti-pattern"`, `"convention"`.
    pub kind: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Optional JSON array of representative code snippets.
    pub examples: Option<String>,
    /// Confidence score in `[0.0, 1.0]`. Decays for stale entries.
    pub score: f64,
    /// Unix epoch (seconds) when the concept was first seen.
    pub first_seen: i64,
    /// Unix epoch (seconds) of the most recent recall / boost.
    pub last_used: i64,
    /// Number of times this concept has been recalled or explicitly boosted.
    pub use_count: i64,
}

// ---------------------------------------------------------------------------
// ConceptStore
// ---------------------------------------------------------------------------

/// Disk-backed concept store.
///
/// Construct with [`ConceptStore::new`]. The caller is responsible for
/// pointing at the right `concepts.db` path — typically via
/// `common::PathManager::concepts_db(project_id)`.
///
/// Internally uses a `Mutex<Connection>` to serialise writes from multiple
/// threads. The underlying SQLite WAL mode allows concurrent reads from other
/// processes (e.g. the MCP layer) without blocking.
pub struct ConceptStore {
    pub(crate) conn: Mutex<Connection>,
}

impl std::fmt::Debug for ConceptStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConceptStore").finish_non_exhaustive()
    }
}

impl ConceptStore {
    /// Open (or create) the concept store at `path`.
    ///
    /// Runs schema bootstrap on every open — `CREATE TABLE IF NOT EXISTS` is
    /// idempotent so there is no migration needed for v0.4.0 first-run.
    pub fn new(path: &Path) -> BrainResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(BrainError::Io)?;
        }
        let conn = Connection::open(path)?;

        // WAL mode: concurrent reads do not block the writer.
        // busy_timeout: wait up to 5s on a write lock instead of failing.
        // foreign_keys: enforced at the application level even though this
        //               schema has no FK constraints yet — good hygiene.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )?;

        conn.execute_batch(CONCEPTS_INIT_SQL)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert or update a concept.
    ///
    /// Conflict resolution (keyed on `id = sha256(name + project_id)[:16]`):
    /// - score: take the higher of the stored and incoming values.
    /// - last_used: always update to `now`.
    /// - use_count: increment by 1.
    /// - All other fields: update from incoming data.
    pub fn upsert(&self, concept: &StoredConcept) -> BrainResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| BrainError::ConceptStore(format!("lock poisoned: {e}")))?;

        conn.execute(
            "INSERT INTO concepts
                 (id, project_id, name, kind, description, examples,
                  score, first_seen, last_used, use_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                 kind        = excluded.kind,
                 description = excluded.description,
                 examples    = excluded.examples,
                 score       = MAX(score, excluded.score),
                 last_used   = excluded.last_used,
                 use_count   = use_count + 1",
            params![
                concept.id,
                concept.project_id,
                concept.name,
                concept.kind,
                concept.description,
                concept.examples,
                concept.score,
                concept.first_seen,
                concept.last_used,
                concept.use_count,
            ],
        )?;

        Ok(())
    }

    /// Look up a single concept by (project_id, name).
    ///
    /// Returns `None` when the concept has never been stored. Looks up by
    /// the stable `id` hash so the query hits the primary key index.
    pub fn get(&self, project_id: &str, name: &str) -> BrainResult<Option<StoredConcept>> {
        let id = stable_id(project_id, name);
        let conn = self
            .conn
            .lock()
            .map_err(|e| BrainError::ConceptStore(format!("lock poisoned: {e}")))?;

        let result = conn
            .query_row(
                "SELECT id, project_id, name, kind, description, examples,
                         score, first_seen, last_used, use_count
                  FROM concepts
                  WHERE id = ?1",
                params![id],
                row_to_concept,
            )
            .optional()?;

        Ok(result)
    }

    /// Return all concepts for a project, ordered by descending score then
    /// descending use_count, capped at `limit`.
    ///
    /// The composite index `idx_concepts_project` covers the WHERE filter;
    /// the ORDER BY re-sorts the small result set in memory — acceptable
    /// because `limit` is bounded.
    pub fn list_for_project(
        &self,
        project_id: &str,
        limit: usize,
    ) -> BrainResult<Vec<StoredConcept>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| BrainError::ConceptStore(format!("lock poisoned: {e}")))?;

        let mut stmt = conn.prepare_cached(
            "SELECT id, project_id, name, kind, description, examples,
                    score, first_seen, last_used, use_count
             FROM concepts
             WHERE project_id = ?1
             ORDER BY score DESC, use_count DESC
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![project_id, limit as i64], row_to_concept)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Boost a concept: set `last_used = now` and increment `use_count` by 1.
    ///
    /// No-op (returns `Ok(())`) when the concept does not exist yet — callers
    /// should call `upsert` first if they want to guarantee persistence.
    pub fn boost(&self, project_id: &str, name: &str) -> BrainResult<()> {
        let id = stable_id(project_id, name);
        let now = unix_now();
        let conn = self
            .conn
            .lock()
            .map_err(|e| BrainError::ConceptStore(format!("lock poisoned: {e}")))?;

        conn.execute(
            "UPDATE concepts
             SET last_used = ?1, use_count = use_count + 1
             WHERE id = ?2",
            params![now, id],
        )?;

        Ok(())
    }

    /// Decay stale concepts: multiply score by 0.95 for every concept whose
    /// `last_used` is more than `older_than_days` days in the past.
    ///
    /// Returns the number of rows updated. Scores below 0.01 are clamped to
    /// 0.01 rather than zeroed so stale concepts remain discoverable and can
    /// recover through use rather than disappearing silently.
    pub fn decay_stale(&self, older_than_days: u32) -> BrainResult<usize> {
        let now = unix_now();
        let cutoff = now - (older_than_days as i64) * 86_400;
        let conn = self
            .conn
            .lock()
            .map_err(|e| BrainError::ConceptStore(format!("lock poisoned: {e}")))?;

        let changed = conn.execute(
            "UPDATE concepts
             SET score = MAX(0.01, score * 0.95)
             WHERE last_used < ?1",
            params![cutoff],
        )?;

        Ok(changed)
    }

    /// Total number of concepts stored in the database (across all projects).
    ///
    /// Used by `mneme doctor` to surface row counts in the persistence box.
    pub fn total_count(&self) -> BrainResult<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| BrainError::ConceptStore(format!("lock poisoned: {e}")))?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM concepts", [], |row| row.get(0))?;

        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Schema bootstrap (idempotent — safe to re-run on every open)
// ---------------------------------------------------------------------------

const CONCEPTS_INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS concepts (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    description TEXT,
    examples    TEXT,
    score       REAL NOT NULL DEFAULT 0.5,
    first_seen  INTEGER NOT NULL,
    last_used   INTEGER NOT NULL,
    use_count   INTEGER NOT NULL DEFAULT 1
);

-- Covers list_for_project WHERE project_id = ?
CREATE INDEX IF NOT EXISTS idx_concepts_project
    ON concepts(project_id);

-- Covers get() lookup and upsert conflict detection by (project_id, name)
-- Also accelerates "does this project+name pair exist?" prefix scans.
CREATE INDEX IF NOT EXISTS idx_concepts_name
    ON concepts(project_id, name);
"#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Stable, deterministic primary key for a (name, project_id) pair.
///
/// Takes the first 16 bytes (32 hex chars) of `SHA-256(project_id || '\0' || name)`.
/// The null-byte separator prevents `("ab", "c")` from colliding with `("a", "bc")`.
pub fn stable_id(project_id: &str, name: &str) -> String {
    let mut h = Sha256::new();
    h.update(project_id.as_bytes());
    h.update(b"\0");
    h.update(name.as_bytes());
    let digest = h.finalize();
    // First 16 bytes = 32 hex chars — collision probability is negligible for
    // any realistic number of (project, concept) pairs (birthday bound at ~2^64).
    let mut s = String::with_capacity(32);
    for byte in &digest[..16] {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// Current unix timestamp in whole seconds.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn row_to_concept(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredConcept> {
    Ok(StoredConcept {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        description: row.get(4)?,
        examples: row.get(5)?,
        score: row.get(6)?,
        first_seen: row.get(7)?,
        last_used: row.get(8)?,
        use_count: row.get(9)?,
    })
}

// ---------------------------------------------------------------------------
// Convenience builder
// ---------------------------------------------------------------------------

impl StoredConcept {
    /// Build a new `StoredConcept` with auto-generated `id`, `first_seen`,
    /// and `last_used` fields.
    pub fn new(
        project_id: impl Into<String>,
        name: impl Into<String>,
        kind: impl Into<String>,
    ) -> Self {
        let project_id = project_id.into();
        let name = name.into();
        let kind = kind.into();
        let now = unix_now();
        let id = stable_id(&project_id, &name);
        Self {
            id,
            project_id,
            name,
            kind,
            description: None,
            examples: None,
            score: 0.5,
            first_seen: now,
            last_used: now,
            use_count: 1,
        }
    }

    /// Set the confidence score (clamped to `[0.0, 1.0]`).
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = score.clamp(0.0, 1.0);
        self
    }

    /// Set the human-readable description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the JSON-encoded examples array.
    pub fn with_examples(mut self, examples: impl Into<String>) -> Self {
        self.examples = Some(examples.into());
        self
    }
}

// ---------------------------------------------------------------------------
// Warn on poisoned lock (no panic in production paths)
// ---------------------------------------------------------------------------

/// Log a warning and return an error when a Mutex is poisoned.
///
/// Mutex poisoning happens if a thread panics while holding the lock. Rather
/// than propagating the panic to unrelated callers we log a warning and
/// return a `BrainError::ConceptStore` so the caller can degrade gracefully.
#[allow(dead_code)]
fn lock_err(msg: impl std::fmt::Display) -> BrainError {
    warn!("concept store mutex poisoned: {msg}");
    BrainError::ConceptStore(format!("mutex poisoned: {msg}"))
}
