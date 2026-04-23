use serde::{Deserialize, Serialize};

use crate::ids::{SessionId, StepId};
use crate::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepStatus {
    NotStarted,
    InProgress,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: StepId,
    pub parent: Option<StepId>,
    pub session_id: SessionId,
    pub description: String,
    /// Shell command that returns 0 on success.
    pub acceptance_cmd: Option<String>,
    /// Structured check (e.g., {"file_exists": "..."}).
    pub acceptance_check: serde_json::Value,
    pub status: StepStatus,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub verification_proof: Option<String>,
    /// {"files_created": [...], "files_modified": [...]}
    pub artifacts: serde_json::Value,
    pub notes: String,
    pub blocker: Option<String>,
    pub drift_score: u32,
}

impl Step {
    pub fn new(id: StepId, session: SessionId, description: impl Into<String>) -> Self {
        Self {
            id,
            parent: None,
            session_id: session,
            description: description.into(),
            acceptance_cmd: None,
            acceptance_check: serde_json::Value::Null,
            status: StepStatus::NotStarted,
            started_at: None,
            completed_at: None,
            verification_proof: None,
            artifacts: serde_json::json!({}),
            notes: String::new(),
            blocker: None,
            drift_score: 0,
        }
    }
}
