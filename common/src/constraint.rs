use serde::{Deserialize, Serialize};

use crate::ids::RowId;
use crate::time::Timestamp;

/// Where a constraint applies. Per-project unless promoted to user/global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintScope {
    Global,
    User,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: Option<RowId>,
    pub scope: ConstraintScope,
    /// Short rule identifier, e.g., "no-hardcoded-colors".
    pub rule_id: String,
    /// Verbatim user-facing rule statement.
    pub rule: String,
    /// Why the rule exists (background / past incident).
    pub why: String,
    /// When/where this kicks in.
    pub how_to_apply: String,
    /// File globs the constraint applies to. Empty = all files.
    pub applies_to: Vec<String>,
    /// Link to the source file/line where the constraint was declared.
    pub source: Option<String>,
    pub created_at: Timestamp,
}
