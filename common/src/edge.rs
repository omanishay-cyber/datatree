use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::EdgeId;
use crate::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Calls,
    ImportsFrom,
    Inherits,
    Implements,
    Contains,
    TestedBy,
    DependsOn,
    References,
    SemanticallySimilarTo,
    RationaleFor,
    Cites,
    Uses,
    Instantiates,
    /// Cross-modal: code node ↔ doc/image/audio concept.
    Mentions,
}

/// Per design §10.3 + §21.2.2, every edge is tagged with a confidence
/// label and a continuous score (0.0 - 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Confidence {
    Extracted,
    Inferred,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub kind: EdgeKind,
    pub source_qualified: String,
    pub target_qualified: String,
    pub confidence: Confidence,
    /// 0.0 - 1.0; EXTRACTED is always 1.0.
    pub confidence_score: f32,
    pub file_path: Option<PathBuf>,
    pub line: Option<u32>,
    pub source_extractor: String,
    pub extra: serde_json::Value,
    pub updated_at: Timestamp,
}
