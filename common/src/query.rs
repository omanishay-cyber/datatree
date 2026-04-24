//! Shared query payload types used by both the CLI and the supervisor.
//!
//! The CLI's `recall`, `blast`, and `godnodes` subcommands can either talk
//! to the running supervisor over IPC (preferred — benefits from the
//! supervisor's query cache + concurrent-read pooling) or open the
//! project's `graph.db` directly when the supervisor is down. Both paths
//! must produce the same record shape so the printer is agnostic to the
//! source.
//!
//! These structs are carried over the wire inside
//! `supervisor::ControlResponse::{RecallResults,BlastResults,GodNodesResults}`
//! (and the CLI's mirror `IpcResponse`). They MUST stay additive — the
//! CLI already prints every field, and breaking a field rename would ship
//! blank columns to users on mixed-version rollouts.
//!
//! Wire format notes:
//!   * All fields are `serde::{Serialize, Deserialize}`.
//!   * Optional columns in `graph.db` use `Option<T>` here so the
//!     SQL-read path can round-trip NULL values without data loss.
//!   * Field names match what the direct-DB path in the CLI already
//!     prints today; do not rename.

use serde::{Deserialize, Serialize};

/// One result row from `recall`. Mirrors the columns the CLI's direct-DB
/// reader selects from `nodes` (and the FTS5 virtual table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallHit {
    /// Node kind tag (e.g. `"fn"`, `"concept"`, `"file"`).
    pub kind: String,
    /// Short display name of the node.
    pub name: String,
    /// Fully-qualified name — unique within the project.
    pub qualified_name: String,
    /// Source file path, if any (NULL for synthetic nodes).
    pub file_path: Option<String>,
    /// 1-based start line in `file_path`, if any.
    pub line_start: Option<i64>,
}

/// One result row from `blast`. The CLI prints the `qualified_name`
/// alongside its layer depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastItem {
    /// Qualified name of the dependent node.
    pub qualified_name: String,
    /// BFS layer depth (1 = direct dependent, 2 = one hop, ...).
    pub depth: usize,
}

/// One result row from `godnodes`. Most-connected concepts first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNode {
    /// Fully-qualified name (unique within the project).
    pub qualified_name: String,
    /// Node kind tag.
    pub kind: String,
    /// Short display name.
    pub name: String,
    /// Source file path, if any.
    pub file_path: Option<String>,
    /// Combined fan-in + fan-out degree.
    pub degree: i64,
    /// Inbound edge count.
    pub fan_in: i64,
    /// Outbound edge count.
    pub fan_out: i64,
}
