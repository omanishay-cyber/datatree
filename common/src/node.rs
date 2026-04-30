use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::NodeId;
use crate::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Class,
    Function,
    Method,
    Test,
    Type,
    Interface,
    Enum,
    Module,
    Constant,
    Decision,
    Concept,
    Doc,
    /// Multimodal corpus item (PDF page, image, audio segment, ...).
    CorpusItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub file_path: Option<PathBuf>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub language: Option<String>,
    pub parent_qualified: Option<String>,
    pub signature: Option<String>,
    pub modifiers: Option<String>,
    pub is_test: bool,
    pub file_hash: Option<String>,
    pub summary: Option<String>,
    pub embedding_id: Option<i64>,
    pub extra: serde_json::Value,
    pub updated_at: Timestamp,
}
