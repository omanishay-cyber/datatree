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

    #[error("disk full: {available_bytes} bytes free")]
    DiskFull { available_bytes: u64 },

    #[error("permission denied")]
    PermissionDenied,

    #[error("internal panic: {backtrace}")]
    InternalPanic { backtrace: String },

    #[error("rusqlite: {0}")]
    Sqlite(String),
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            match err.code {
                rusqlite::ErrorCode::DatabaseCorrupt => {
                    return DbError::Corrupted { detail: e.to_string() }
                }
                rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::PermissionDenied => {
                    return DbError::PermissionDenied
                }
                rusqlite::ErrorCode::DiskFull => {
                    return DbError::DiskFull { available_bytes: 0 }
                }
                _ => {}
            }
        }
        DbError::Sqlite(e.to_string())
    }
}
