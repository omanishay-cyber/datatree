use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self {
        Timestamp(Utc::now())
    }

    pub fn as_unix_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }

    pub fn from_unix_millis(ms: i64) -> Self {
        Timestamp(DateTime::from_timestamp_millis(ms).unwrap_or_else(Utc::now))
    }

    /// Render as a directory-name-safe string: `2026-04-23-14-30-00`.
    pub fn as_dirname(&self) -> String {
        self.0.format("%Y-%m-%d-%H-%M-%S").to_string()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}
