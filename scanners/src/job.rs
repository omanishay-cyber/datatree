//! Scan job + scan result envelopes that flow through the worker pool MPSC.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::scanner::Finding;

/// A unit of work pushed into the scan-worker MPSC. The supervisor or the
/// parse-worker constructs these.
#[derive(Debug, Clone)]
pub struct ScanJob {
    /// Absolute path of the file to scan (already canonicalized).
    pub file_path: PathBuf,
    /// File contents loaded by the caller. Scanners must not re-read.
    pub content: Arc<String>,
    /// Optional AST handle from the parse-worker. `None` means scanners
    /// that need an AST will skip themselves.
    pub ast_id: Option<u64>,
    /// If non-empty, only these scanner names are invoked. Empty = all.
    pub scanner_filter: Vec<String>,
    /// Job correlation id used by the store-worker to batch findings.
    pub job_id: u64,
}

impl ScanJob {
    /// Build a job that runs every applicable scanner.
    #[must_use]
    pub fn new(job_id: u64, file_path: PathBuf, content: Arc<String>) -> Self {
        Self {
            file_path,
            content,
            ast_id: None,
            scanner_filter: Vec::new(),
            job_id,
        }
    }

    /// Returns true when this job should run the scanner with `name`.
    #[must_use]
    pub fn allows_scanner(&self, name: &str) -> bool {
        self.scanner_filter.is_empty() || self.scanner_filter.iter().any(|n| n == name)
    }
}

/// Result envelope returned to the store-worker. Contains every finding
/// produced for a given job plus per-scanner timing/error metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Echo of [`ScanJob::job_id`].
    pub job_id: u64,
    /// Findings collated across every scanner that ran.
    pub findings: Vec<Finding>,
    /// Wall-clock duration of the entire job in milliseconds.
    pub scan_duration_ms: u64,
    /// Names of scanners that errored. The other scanners' results are
    /// still present in `findings` (failure isolation).
    pub failed_scanners: Vec<String>,
}

impl ScanResult {
    /// Empty result with the given job id.
    #[must_use]
    pub fn empty(job_id: u64) -> Self {
        Self {
            job_id,
            findings: Vec::new(),
            scan_duration_ms: 0,
            failed_scanners: Vec::new(),
        }
    }
}
