use serde::{Deserialize, Serialize};

use crate::ids::{ProjectId, SessionId};
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventTopic {
    FileChanged { project: ProjectId },
    TestStatus { project: ProjectId },
    DriftFinding { project: ProjectId },
    SubagentEvent { project: ProjectId },
    CompactionDetected { session: SessionId },
    StepAdvanced { session: SessionId },
    SystemHealth,
    SystemDegradedMode,
    DecisionRecorded { project: ProjectId },
    ConstraintAdded { project: ProjectId },
    ResumptionEmitted { session: SessionId },
}

impl EventTopic {
    /// Wire-format topic string for SSE/WS subscription.
    pub fn as_topic(&self) -> String {
        match self {
            Self::FileChanged { project } => format!("project.{}.file_changed", project),
            Self::TestStatus { project } => format!("project.{}.test_status", project),
            Self::DriftFinding { project } => format!("project.{}.drift_finding", project),
            Self::SubagentEvent { project } => format!("project.{}.subagent_event", project),
            Self::CompactionDetected { session } => format!("session.{}.compaction_detected", session),
            Self::StepAdvanced { session } => format!("session.{}.step_advanced", session),
            Self::SystemHealth => "system.health".into(),
            Self::SystemDegradedMode => "system.degraded_mode".into(),
            Self::DecisionRecorded { project } => format!("project.{}.decision_recorded", project),
            Self::ConstraintAdded { project } => format!("project.{}.constraint_added", project),
            Self::ResumptionEmitted { session } => format!("session.{}.resumption_emitted", session),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub topic: String,
    pub timestamp: Timestamp,
    pub session_id: Option<SessionId>,
    pub project_hash: Option<String>,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn new(topic: EventTopic, payload: serde_json::Value) -> Self {
        let topic_str = topic.as_topic();
        let (session_id, project_hash) = match &topic {
            EventTopic::CompactionDetected { session }
            | EventTopic::StepAdvanced { session }
            | EventTopic::ResumptionEmitted { session } => (Some(session.clone()), None),
            EventTopic::FileChanged { project }
            | EventTopic::TestStatus { project }
            | EventTopic::DriftFinding { project }
            | EventTopic::SubagentEvent { project }
            | EventTopic::DecisionRecorded { project }
            | EventTopic::ConstraintAdded { project } => (None, Some(project.to_string())),
            EventTopic::SystemHealth | EventTopic::SystemDegradedMode => (None, None),
        };
        Self {
            topic: topic_str,
            timestamp: Timestamp::now(),
            session_id,
            project_hash,
            payload,
        }
    }
}
