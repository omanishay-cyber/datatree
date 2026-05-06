use thiserror::Error;

use crate::time::Timestamp;

pub type DtResult<T> = Result<T, DtError>;

#[derive(Debug, Error)]
pub enum DtError {
    #[error("database error: {0}")]
    Db(#[from] DbError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid project root: {0}")]
    InvalidProjectRoot(String),

    #[error("path traversal blocked: {0}")]
    PathTraversal(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Error, Clone)]
pub enum DbError {
    #[error("not found")]
    NotFound,

    #[error("corrupted: {detail}")]
    Corrupted { detail: String },

    #[error("locked by {holder} since {since}")]
    Locked { holder: String, since: Timestamp },

    #[error("timeout after {elapsed_ms}ms")]
    Timeout { elapsed_ms: u64 },

    #[error("schema mismatch: expected v{expected}, found v{found}")]
    SchemaMismatch { expected: u32, found: u32 },

    #[error("serialization failure")]
    SerializationFailure,

    /// SQLITE_FULL: the destination filesystem cannot accept any more
    /// writes. `available_bytes` is the free space at the moment the
    /// error was observed, or `None` when the From<rusqlite::Error>
    /// conversion produced this variant — that conversion has no path
    /// context, so it cannot stat the filesystem. Callers that DO know
    /// the path (e.g. inject.rs after a `conn.execute()` failure) can
    /// build this variant directly via `disk_full_for_path()` to fill
    /// in the real number.
    ///
    /// LOW fix (2026-05-05 audit): the type previously hard-coded
    /// `available_bytes: 0`, which read as "the filesystem has zero
    /// free bytes" rather than "we don't know". Operators couldn't
    /// distinguish "literally full" from "we lost the path on the way
    /// out the door". `Option` makes the unknown case honest.
    #[error("disk full: {} bytes free", available_bytes.map_or_else(|| "unknown".to_string(), |b| b.to_string()))]
    DiskFull { available_bytes: Option<u64> },

    #[error("permission denied")]
    PermissionDenied,

    #[error("internal panic: {backtrace}")]
    InternalPanic { backtrace: String },

    #[error("rusqlite: {0}")]
    Sqlite(String),

    /// HIGH-10 fix (2026-05-05 audit): two callers supplied the same
    /// ProjectId (SHA-256 of canonical path) but different root strings.
    /// This is either a canonicalization bug or a genuine hash collision
    /// (astronomically unlikely). Either way, silent overwrite is worse
    /// than a loud failure — the user must run `mneme rebuild` with the
    /// correct path to resolve.
    #[error(
        "project id collision: id={id} has existing root `{existing_root}` \
         but incoming root is `{incoming_root}` — run `mneme rebuild` to resolve"
    )]
    ProjectIdCollision {
        id: String,
        existing_root: String,
        incoming_root: String,
    },
}

impl DbError {
    /// Build a `DbError::DiskFull` with the real free-space number for
    /// `path`'s filesystem. Falls back to `None` (unknown) on stat
    /// failure — the caller already knows the disk is full, so
    /// reporting "unknown free bytes" instead of crashing the whole
    /// error path is the right behaviour. Best-effort stat: uses
    /// `fs2::available_space` when `fs2` is in the dep tree, else
    /// `None`.
    pub fn disk_full_for_path(path: &std::path::Path) -> Self {
        let bytes = fs2::available_space(path).ok();
        DbError::DiskFull {
            available_bytes: bytes,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            match err.code {
                rusqlite::ErrorCode::DatabaseCorrupt => {
                    return DbError::Corrupted {
                        detail: e.to_string(),
                    }
                }
                rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::PermissionDenied => {
                    return DbError::PermissionDenied
                }
                rusqlite::ErrorCode::DiskFull => {
                    return DbError::DiskFull {
                        available_bytes: None,
                    }
                }
                _ => {}
            }
        }
        DbError::Sqlite(e.to_string())
    }
}
