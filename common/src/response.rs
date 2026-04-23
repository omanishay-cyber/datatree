use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DbError;
use crate::layer::DbLayer;

/// Uniform envelope returned by every datatree storage operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<DbErrorWire>,
    pub meta: ResponseMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub latency_ms: u64,
    pub cache_hit: bool,
    pub source_db: DbLayer,
    pub query_id: Uuid,
    pub schema_version: u32,
}

/// Wire-friendly error representation (DbError is not Serialize-friendly
/// because of the `From<rusqlite::Error>` impl carrying live data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbErrorWire {
    pub kind: String,
    pub message: String,
    pub detail: Option<String>,
}

impl<T> Response<T> {
    pub fn ok(data: T, meta: ResponseMeta) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta,
        }
    }

    pub fn err(e: DbError, meta: ResponseMeta) -> Self {
        let wire = DbErrorWire {
            kind: variant_name(&e).to_string(),
            message: e.to_string(),
            detail: None,
        };
        Self {
            success: false,
            data: None,
            error: Some(wire),
            meta,
        }
    }
}

fn variant_name(e: &DbError) -> &'static str {
    match e {
        DbError::NotFound => "not_found",
        DbError::Corrupted { .. } => "corrupted",
        DbError::Locked { .. } => "locked",
        DbError::Timeout { .. } => "timeout",
        DbError::SchemaMismatch { .. } => "schema_mismatch",
        DbError::SerializationFailure => "serialization_failure",
        DbError::DiskFull { .. } => "disk_full",
        DbError::PermissionDenied => "permission_denied",
        DbError::InternalPanic { .. } => "internal_panic",
        DbError::Sqlite(_) => "sqlite",
    }
}
