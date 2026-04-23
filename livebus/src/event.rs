//! Event envelope and typed payload variants.
//!
//! Every message that flows through the bus is wrapped in [`Event`]. The
//! `payload` field carries arbitrary JSON; the [`EventPayload`] enum and the
//! per-variant structs (e.g. [`FileChanged`]) provide typed convenience
//! constructors so workers don't have to hand-build JSON literals.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The event envelope flowing through the bus.
///
/// `topic` is the *concrete* (non-wildcard) topic the publisher emitted on.
/// Subscribers can register wildcard *patterns* such as `project.*.file_changed`
/// — see [`crate::topic_matches`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Concrete topic, e.g. `project.abc123.file_changed`.
    pub topic: String,

    /// UTC timestamp of when the publisher created the event.
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,

    /// Optional Claude Code session id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Optional project hash (matches the `<hash>` token in the topic).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_hash: Option<String>,

    /// Free-form payload. Workers may use [`EventPayload`] helpers to fill
    /// this with a typed shape, but downstream subscribers only see JSON.
    pub payload: Value,
}

impl Event {
    /// Construct an event from a typed payload.
    pub fn from_typed(
        topic: impl Into<String>,
        session_id: Option<String>,
        project_hash: Option<String>,
        payload: EventPayload,
    ) -> Self {
        Self {
            topic: topic.into(),
            timestamp: Utc::now(),
            session_id,
            project_hash,
            payload: serde_json::to_value(&payload).unwrap_or(Value::Null),
        }
    }

    /// Construct an event with a raw JSON payload.
    pub fn from_json(
        topic: impl Into<String>,
        session_id: Option<String>,
        project_hash: Option<String>,
        payload: Value,
    ) -> Self {
        Self {
            topic: topic.into(),
            timestamp: Utc::now(),
            session_id,
            project_hash,
            payload,
        }
    }

    /// Encode as a single-line JSON string suitable for SSE `data:` framing.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Typed payload variants. The serialized form is internally tagged with
/// `kind` so consumers can branch without inspecting the topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventPayload {
    FileChanged(FileChanged),
    TestStatus(TestStatus),
    DriftFinding(DriftFinding),
    SubagentEvent(SubagentEvent),
    CompactionDetected(CompactionDetected),
    StepAdvanced(StepAdvanced),
    HealthUpdate(HealthUpdate),
    DegradedMode(DegradedMode),
    /// Escape hatch for ad-hoc payloads not yet promoted to a typed variant.
    Raw(Value),
}

/// `project.<hash>.file_changed` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChanged {
    pub path: String,
    pub change_kind: FileChangeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// `project.<hash>.test_status` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStatus {
    pub suite: String,
    pub status: TestState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing_tests: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestState {
    Running,
    Passed,
    Failed,
    Errored,
    Skipped,
}

/// `project.<hash>.drift_finding` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftFinding {
    pub finding_id: String,
    pub severity: DriftSeverity,
    pub category: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// `project.<hash>.subagent_event` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentEvent {
    pub agent_name: String,
    pub phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,
}

/// `session.<id>.compaction_detected` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionDetected {
    pub session_id: String,
    pub trigger: String,
    pub previous_token_count: u64,
    pub current_token_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_handle: Option<String>,
}

/// `session.<id>.step_advanced` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAdvanced {
    pub session_id: String,
    pub step_index: u32,
    pub step_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roadmap_id: Option<String>,
}

/// `system.health` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthUpdate {
    pub component: String,
    pub status: String,
    pub uptime_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Value>,
}

/// `system.degraded_mode` payload — emitted when a subscriber is evicted or
/// the bus enters back-pressure throttling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradedMode {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscriber_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped_count: Option<u64>,
}

#[cfg(test)]
mod event_tests {
    use super::*;

    #[test]
    fn roundtrip_file_changed() {
        let payload = EventPayload::FileChanged(FileChanged {
            path: "src/main.rs".into(),
            change_kind: FileChangeKind::Modified,
            bytes: Some(42),
            content_hash: None,
        });
        let ev = Event::from_typed(
            "project.abc123.file_changed",
            None,
            Some("abc123".into()),
            payload,
        );
        let line = ev.to_json_line().unwrap();
        let back: Event = serde_json::from_str(&line).unwrap();
        assert_eq!(back.topic, "project.abc123.file_changed");
        assert_eq!(back.project_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn raw_payload_roundtrip() {
        let raw = serde_json::json!({"hello": "world"});
        let ev = Event::from_json("system.health", None, None, raw.clone());
        let s = ev.to_json_line().unwrap();
        let back: Event = serde_json::from_str(&s).unwrap();
        assert_eq!(back.payload, raw);
    }
}
