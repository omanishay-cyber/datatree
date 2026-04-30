//! Shared types for mneme.
//!
//! Every mneme crate depends on this. Types defined here are the
//! lingua franca between supervisor, store, parsers, scanners, brain,
//! livebus, multimodal-bridge, and cli.

pub mod error;
pub mod ids;
pub mod layer;
pub mod paths;
pub mod project;
pub mod response;
pub mod time;
pub mod node;
pub mod edge;
pub mod finding;
pub mod constraint;
pub mod decision;
pub mod step;
pub mod event;
pub mod jobs;
pub mod query;
pub mod worker_ipc;

pub use error::{DbError, DtError, DtResult};
pub use ids::{ProjectId, RowId, SessionId, SnapshotId, StepId, NodeId, EdgeId};
pub use layer::DbLayer;
pub use paths::PathManager;
pub use project::{Project, ShardHandle};
pub use response::{Response, ResponseMeta};
pub use time::Timestamp;
pub use node::{Node, NodeKind};
pub use edge::{Edge, EdgeKind, Confidence};
pub use finding::{Finding, Severity};
pub use constraint::{Constraint, ConstraintScope};
pub use decision::Decision;
pub use step::{Step, StepStatus};
pub use event::{Event, EventTopic};
pub use jobs::{Job, JobId, JobOutcome};
pub use query::{BlastItem, GodNode, RecallHit};
