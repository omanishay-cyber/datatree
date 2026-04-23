use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::RowId;
use crate::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: Option<RowId>,
    pub rule_id: String,
    pub scanner: String,
    pub severity: Severity,
    pub file: PathBuf,
    pub line_start: u32,
    pub line_end: u32,
    pub column_start: u32,
    pub column_end: u32,
    pub message: String,
    pub suggestion: Option<String>,
    pub auto_fixable: bool,
    pub created_at: Timestamp,
}
