//! Cross-process job schema for supervisor-mediated worker dispatch.
//!
//! Added in v0.3 so `mneme build` can hand parse/scan/embed work to the
//! already-spawned worker pool instead of running the full pipeline
//! inline in the CLI process. The CLI submits `Job`s via IPC; the
//! supervisor fans them out to the matching worker pool; workers report
//! results back.
//!
//! `JobId` is opaque to the CLI and used only to correlate completions.
//! For v0.3 MVP we do NOT persist the queue across supervisor restarts —
//! a crash that loses an in-flight job fails the CLI's watchdog and the
//! user re-runs `mneme build`. Durable queue is tracked as future work.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Opaque job identifier. Monotonic within a supervisor lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub u64);

impl JobId {
    /// Allocate the next monotonically-increasing id.
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        JobId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One unit of work dispatched from the CLI to the supervisor's worker
/// pool.
///
/// Keep each variant self-contained — the supervisor serialises it as a
/// JSON line straight to the target worker's stdin, so every field the
/// worker needs must be reachable from the variant itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Job {
    /// Tree-sitter parse + extract on a single file. Handled by the
    /// `parser-worker-*` pool.
    Parse {
        /// Canonical absolute path to the source file.
        file_path: PathBuf,
        /// Project shard this parse belongs to (used by the worker when
        /// it forwards nodes/edges to the store).
        shard_root: PathBuf,
    },
    /// Run registered scanners over an already-parsed file. Handled by
    /// the `scanner-worker-*` pool.
    Scan {
        /// File the scanners should inspect.
        file_path: PathBuf,
        /// Optional AST id produced by the Parse job (allows scanner to
        /// skip its own reparse when the parser worker pushed the tree
        /// into a shared cache).
        ast_id: Option<u64>,
        /// Project shard root.
        shard_root: PathBuf,
    },
    /// Compute an embedding for one qualified node. Handled by the
    /// `brain-worker` pool.
    Embed {
        /// Fully-qualified node name (matches the `qualified_name`
        /// column in the graph db).
        node_qualified: String,
        /// Text to embed (typically the node's source + docstring).
        text: String,
        /// Project shard root.
        shard_root: PathBuf,
    },
    /// Ingest a single markdown/doc file into the docs layer. Handled
    /// by the `md-ingest-worker` pool.
    Ingest {
        /// Absolute path to the .md file.
        md_file: PathBuf,
        /// Project shard root.
        shard_root: PathBuf,
    },
}

impl Job {
    /// Which worker pool prefix this job targets. The supervisor uses
    /// this to select a child from the pool matching this prefix.
    pub fn pool_prefix(&self) -> &'static str {
        match self {
            Job::Parse { .. } => "parser-worker-",
            Job::Scan { .. } => "scanner-worker-",
            Job::Embed { .. } => "brain-worker",
            Job::Ingest { .. } => "md-ingest-worker",
        }
    }

    /// Human-readable label for logs / error messages.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Job::Parse { .. } => "parse",
            Job::Scan { .. } => "scan",
            Job::Embed { .. } => "embed",
            Job::Ingest { .. } => "ingest",
        }
    }
}

/// Completion status reported by a worker via `WorkerCompleteJob`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum JobOutcome {
    /// Worker finished the job successfully. Payload is the serialised
    /// worker-native result (e.g. `ParseResult` json) — the CLI decodes
    /// it based on the original `Job` variant.
    Ok {
        /// Result JSON (opaque here; typed at callsite).
        #[serde(default)]
        payload: Option<serde_json::Value>,
    },
    /// Worker encountered an error. The job is not re-queued — the CLI
    /// decides whether to skip or abort.
    Err {
        /// Human-readable error message.
        message: String,
    },
}

impl JobOutcome {
    /// Convenience: `true` iff outcome is `Ok`.
    pub fn is_ok(&self) -> bool {
        matches!(self, JobOutcome::Ok { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_id_is_monotonic() {
        let a = JobId::next();
        let b = JobId::next();
        assert!(b.0 > a.0);
    }

    #[test]
    fn parse_job_round_trips_through_json() {
        let j = Job::Parse {
            file_path: PathBuf::from("/tmp/foo.rs"),
            shard_root: PathBuf::from("/home/u/.mneme/projects/abc"),
        };
        let s = serde_json::to_string(&j).unwrap();
        let back: Job = serde_json::from_str(&s).unwrap();
        match back {
            Job::Parse { file_path, .. } => assert_eq!(file_path, PathBuf::from("/tmp/foo.rs")),
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn pool_prefix_matches_worker_naming() {
        let j = Job::Parse {
            file_path: "x".into(),
            shard_root: "y".into(),
        };
        assert_eq!(j.pool_prefix(), "parser-worker-");
        let j = Job::Embed {
            node_qualified: "foo::bar".into(),
            text: "fn bar() {}".into(),
            shard_root: "y".into(),
        };
        assert_eq!(j.pool_prefix(), "brain-worker");
    }

    #[test]
    fn outcome_is_ok_helper() {
        let ok = JobOutcome::Ok { payload: None };
        let err = JobOutcome::Err {
            message: "boom".into(),
        };
        assert!(ok.is_ok());
        assert!(!err.is_ok());
    }
}
