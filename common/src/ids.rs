use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Stable, deterministic identifier for a project derived from its
/// canonical absolute path. Used to name the per-project shard folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(String);

impl ProjectId {
    /// Construct from an absolute path. Hashes the canonicalized form.
    pub fn from_path(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let canon = dunce::canonicalize(path.as_ref())?;
        let s = canon.to_string_lossy();
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        let digest = h.finalize();
        Ok(ProjectId(hex(&digest)))
    }

    /// Construct directly from a known hash (e.g., from URL/CLI).
    pub fn from_hash(hash: impl Into<String>) -> Self {
        ProjectId(hash.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RowId(pub i64);

impl fmt::Display for RowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    pub fn new() -> Self {
        SessionId(Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string())
    }
    pub fn from_str(s: impl Into<String>) -> Self {
        SessionId(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(String);

impl SnapshotId {
    pub fn from_str(s: impl Into<String>) -> Self {
        SnapshotId(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Hierarchical step id like "1", "1.1", "1.1.1".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepId(String);

impl StepId {
    pub fn root(n: u32) -> Self {
        StepId(n.to_string())
    }
    pub fn child(parent: &StepId, n: u32) -> Self {
        StepId(format!("{}.{}", parent.0, n))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn parent(&self) -> Option<StepId> {
        let pos = self.0.rfind('.')?;
        Some(StepId(self.0[..pos].to_string()))
    }
}

impl fmt::Display for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub i64);

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}
