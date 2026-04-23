use serde::{Deserialize, Serialize};

/// One logical storage layer. Maps to a file on disk in the project shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DbLayer {
    Graph,
    History,
    ToolCache,
    Tasks,
    Semantic,
    Git,
    Memory,
    Errors,
    Multimodal,
    Deps,
    Tests,
    Perf,
    Findings,
    Agents,
    Refactors,
    Contracts,
    Insights,
    LiveState,
    Telemetry,
    Corpus,
    Audit,
    /// Wiki: auto-generated markdown knowledge pages from Leiden communities
    /// and god-nodes. Append-only pages table keyed by (community_id, version).
    Wiki,
    /// Architecture: coupling matrix + betweenness centrality + risk index
    /// snapshots. Append-only; each snapshot is a new row.
    Architecture,
    /// Cross-project meta-database (singleton, not per-project).
    Meta,
}

impl DbLayer {
    /// File name within the project shard folder.
    pub fn file_name(&self) -> &'static str {
        match self {
            Self::Graph => "graph.db",
            Self::History => "history.db",
            Self::ToolCache => "tool_cache.db",
            Self::Tasks => "tasks.db",
            Self::Semantic => "semantic.db",
            Self::Git => "git.db",
            Self::Memory => "memory.db",
            Self::Errors => "errors.db",
            Self::Multimodal => "multimodal.db",
            Self::Deps => "deps.db",
            Self::Tests => "tests.db",
            Self::Perf => "perf.db",
            Self::Findings => "findings.db",
            Self::Agents => "agents.db",
            Self::Refactors => "refactors.db",
            Self::Contracts => "contracts.db",
            Self::Insights => "insights.db",
            Self::LiveState => "livestate.db",
            Self::Telemetry => "telemetry.db",
            Self::Corpus => "corpus.db",
            Self::Audit => "audit.db",
            Self::Wiki => "wiki.db",
            Self::Architecture => "architecture.db",
            Self::Meta => "meta.db",
        }
    }

    pub fn all_per_project() -> &'static [DbLayer] {
        &[
            Self::Graph,
            Self::History,
            Self::ToolCache,
            Self::Tasks,
            Self::Semantic,
            Self::Git,
            Self::Memory,
            Self::Errors,
            Self::Multimodal,
            Self::Deps,
            Self::Tests,
            Self::Perf,
            Self::Findings,
            Self::Agents,
            Self::Refactors,
            Self::Contracts,
            Self::Insights,
            Self::LiveState,
            Self::Telemetry,
            Self::Corpus,
            Self::Audit,
            Self::Wiki,
            Self::Architecture,
        ]
    }
}
