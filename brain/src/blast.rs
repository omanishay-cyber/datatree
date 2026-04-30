//! F7 — Blast Radius risk-scoring upgrade.
//!
//! Replaces the old flat "list of dependents" with a structured
//! [`BlastReport`]: direct consumers, transitive consumers, affected
//! tests, and optionally the step-ledger entries whose acceptance
//! criteria assumed the target's current shape.
//!
//! Risk is computed from the shape of the report by
//! [`compute_risk`] — a deterministic, transparent rule (see doc-comment
//! on the function for the thresholds).

use serde::{Deserialize, Serialize};

/// A reference into the graph / source tree. Kept minimal so the same
/// shape can be serialised across the Rust ↔ TS IPC boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeRef {
    /// Fully-qualified identifier (e.g. `module::fn`, `src/auth.ts:login`).
    pub qualified_name: String,
    /// Containing file path (absolute or workspace-relative).
    pub file: Option<String>,
    /// 1-based line number, if known.
    pub line: Option<u32>,
    /// Graph kind (`"function"`, `"file"`, `"test"`, ...).
    pub kind: String,
}

impl CodeRef {
    pub fn new(qualified_name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            qualified_name: qualified_name.into(),
            file: None,
            line: None,
            kind: kind.into(),
        }
    }
}

/// Severity ladder. Serialises as lowercase strings to match the MCP
/// `SeverityEnum` (`info | low | medium | high | critical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Structured blast-radius report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastReport {
    /// Nodes with an incoming `calls` / `imports` edge directly to the target.
    pub direct_consumers: Vec<CodeRef>,
    /// Nodes reachable via 2+ hops in the consumer direction.
    pub transitive_consumers: Vec<CodeRef>,
    /// Any `test_*` / `*_spec` node transitively consuming the target.
    pub tests_affected: Vec<CodeRef>,
    /// Step-ledger entry IDs whose acceptance criteria assumed the
    /// target's current shape. Empty when F1 isn't populated.
    pub decisions_assumed: Vec<String>,
    /// Overall risk classification.
    pub risk: RiskLevel,
}

impl BlastReport {
    /// Total unique consumers (direct ∪ transitive).
    pub fn total_consumers(&self) -> usize {
        self.direct_consumers.len() + self.transitive_consumers.len()
    }
}

/// Scoring rule — intentionally simple and transparent so users can
/// predict the output without reading code. Thresholds tuned to match the
/// blueprint spec:
///
/// - `direct > 5`           → at least Medium
/// - `transitive > 20`      → at least High
/// - `direct > 15`          → at least High
/// - any `decisions_assumed` → Critical
/// - `tests_affected == 0`  → bump one level (untested code is riskier)
///
/// The bumps compose, but the ceiling is Critical.
pub fn compute_risk(
    direct: usize,
    transitive: usize,
    tests: usize,
    decisions: usize,
) -> RiskLevel {
    if decisions > 0 {
        return RiskLevel::Critical;
    }

    let mut level = RiskLevel::Low;
    if direct > 5 {
        level = RiskLevel::Medium;
    }
    if direct > 15 || transitive > 20 {
        level = RiskLevel::High;
    }
    if direct + transitive > 100 {
        level = RiskLevel::Critical;
    }

    // Untested-code penalty.
    if tests == 0 && (direct + transitive) > 0 {
        level = bump(level);
    }
    level
}

fn bump(l: RiskLevel) -> RiskLevel {
    match l {
        RiskLevel::Low => RiskLevel::Medium,
        RiskLevel::Medium => RiskLevel::High,
        RiskLevel::High => RiskLevel::Critical,
        RiskLevel::Critical => RiskLevel::Critical,
    }
}

/// Build a report from pre-collected refs. The scanner / IPC layer owns
/// the graph walk; this function exists so the scoring rule is the
/// single source of truth (tested in isolation, reused by MCP).
pub fn build_report(
    direct_consumers: Vec<CodeRef>,
    transitive_consumers: Vec<CodeRef>,
    tests_affected: Vec<CodeRef>,
    decisions_assumed: Vec<String>,
) -> BlastReport {
    let risk = compute_risk(
        direct_consumers.len(),
        transitive_consumers.len(),
        tests_affected.len(),
        decisions_assumed.len(),
    );
    BlastReport {
        direct_consumers,
        transitive_consumers,
        tests_affected,
        decisions_assumed,
        risk,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_for_isolated_target() {
        assert_eq!(compute_risk(0, 0, 0, 0), RiskLevel::Low);
    }

    #[test]
    fn medium_above_5_direct() {
        assert_eq!(compute_risk(6, 0, 5, 0), RiskLevel::Medium);
    }

    #[test]
    fn high_above_20_transitive() {
        assert_eq!(compute_risk(1, 25, 3, 0), RiskLevel::High);
    }

    #[test]
    fn critical_when_decisions_touched() {
        assert_eq!(compute_risk(0, 0, 0, 1), RiskLevel::Critical);
    }

    #[test]
    fn untested_code_bumps_level() {
        // Medium + untested → High.
        assert_eq!(compute_risk(6, 0, 0, 0), RiskLevel::High);
    }

    #[test]
    fn build_report_uses_compute_risk() {
        let r = build_report(
            vec![CodeRef::new("a::b", "function"); 3],
            vec![],
            vec![CodeRef::new("test_b", "test")],
            vec![],
        );
        assert_eq!(r.risk, RiskLevel::Low);
        assert_eq!(r.total_consumers(), 3);
    }
}
