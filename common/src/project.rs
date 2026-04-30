use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;
use crate::time::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub root: PathBuf,
    pub name: String,
    pub created_at: Timestamp,
    pub last_indexed_at: Option<Timestamp>,
    pub schema_version: u32,
}

/// A handle to a project's shard. Returned by Builder/Finder.
#[derive(Debug, Clone)]
pub struct ShardHandle {
    pub project: Project,
}

impl ShardHandle {
    pub fn id(&self) -> &ProjectId {
        &self.project.id
    }
}
