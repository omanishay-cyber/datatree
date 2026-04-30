use serde::{Deserialize, Serialize};

use crate::ids::{RowId, SessionId};
use crate::time::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: Option<RowId>,
    pub session_id: Option<SessionId>,
    pub topic: String,
    pub problem: String,
    pub chosen: String,
    pub reasoning: String,
    pub alternatives_considered: Vec<String>,
    pub artifacts: Vec<String>,
    pub created_at: Timestamp,
}
