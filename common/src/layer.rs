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
    /// Conventions: inferred project conventions (naming, imports, tests,
    /// etc.) produced by the Convention Learner (F3). Append-only —
    /// re-running `mneme build` inserts fresh rows with updated confidence
    /// rather than mutating existing ones.
    Conventions,
    /// Federated pattern fingerprints (Moat 4). Append-only. Stores local
    /// SimHash+MinHash fingerprints derived from Convention Learner output
    /// and the concept graph. No source code leaves this shard; upload is
    /// strictly opt-in (see `brain::federated`). `source_file` is present
    /// locally but is NEVER included in any upload payload.
    Federated,
    /// Concept memory (v0.4 Wave 3.3). Persists recalled / extracted
    /// concepts across daemon restarts. One row per (project_id, name)
    /// pair; scores decay over time; use_count drives re-ranking.
    /// Previous storage was in-memory only; v0.4.0 first run creates
    /// this shard fresh (no data migration — ephemeral data is gone on
    /// every restart anyway).
    Concepts,
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
            Self::Conventions => "conventions.db",
            Self::Federated => "federated.db",
            Self::Concepts => "concepts.db",
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
            Self::Conventions,
            Self::Federated,
            Self::Concepts,
        ]
    }
}
