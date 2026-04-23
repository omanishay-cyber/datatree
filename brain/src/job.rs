//! Job + Result enums sent over the worker MPSC channel.

use serde::{Deserialize, Serialize};

use crate::concept::Concept;
use crate::leiden::Community;
use crate::NodeId;

/// 64-bit job correlation id (caller-supplied; opaque to the worker).
pub type JobId = u64;

/// Work item submitted to the BRAIN worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrainJob {
    /// Embed a single text. Returns [`BrainResult::Embedding`].
    Embed {
        id: JobId,
        node: Option<NodeId>,
        text: String,
    },

    /// Embed many texts in one batch. Cheaper per-item than [`BrainJob::Embed`].
    EmbedBatch {
        id: JobId,
        items: Vec<(Option<NodeId>, String)>,
    },

    /// Run Leiden community detection over an opaque adjacency list
    /// `(src, dst, weight)`. The runner does not own the graph store; the
    /// caller is responsible for materialising edges from disk.
    Cluster {
        id: JobId,
        edges: Vec<(NodeId, NodeId, f32)>,
        seed: Option<u64>,
    },

    /// Extract concepts from a chunk of text/code. `kind` is a free-form
    /// hint ("code", "comment", "readme", "doc") that biases the extractor.
    ExtractConcepts {
        id: JobId,
        node: Option<NodeId>,
        kind: String,
        text: String,
    },

    /// Produce a one-sentence summary for a function (signature + body).
    Summarize {
        id: JobId,
        node: Option<NodeId>,
        signature: String,
        body: String,
    },

    /// Graceful shutdown.
    Shutdown,
}

impl BrainJob {
    pub fn id(&self) -> JobId {
        match self {
            BrainJob::Embed { id, .. }
            | BrainJob::EmbedBatch { id, .. }
            | BrainJob::Cluster { id, .. }
            | BrainJob::ExtractConcepts { id, .. }
            | BrainJob::Summarize { id, .. } => *id,
            BrainJob::Shutdown => 0,
        }
    }
}

/// Result returned from the worker for each non-shutdown job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrainResult {
    Embedding {
        id: JobId,
        node: Option<NodeId>,
        vector: Vec<f32>,
    },
    EmbeddingBatch {
        id: JobId,
        vectors: Vec<(Option<NodeId>, Vec<f32>)>,
    },
    Clusters {
        id: JobId,
        communities: Vec<Community>,
    },
    Concepts {
        id: JobId,
        node: Option<NodeId>,
        concepts: Vec<Concept>,
    },
    Summary {
        id: JobId,
        node: Option<NodeId>,
        summary: String,
    },
    Error {
        id: JobId,
        message: String,
    },
}

impl BrainResult {
    pub fn id(&self) -> JobId {
        match self {
            BrainResult::Embedding { id, .. }
            | BrainResult::EmbeddingBatch { id, .. }
            | BrainResult::Clusters { id, .. }
            | BrainResult::Concepts { id, .. }
            | BrainResult::Summary { id, .. }
            | BrainResult::Error { id, .. } => *id,
        }
    }
}
