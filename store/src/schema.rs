//! SQL schemas for every DbLayer. Versioned, append-only.
//!
//! New schema versions add columns; never drop, never rename. To rename
//! conceptually, add a new column and stop writing the old one.

use datatree_common::layer::DbLayer;

pub const SCHEMA_VERSION: u32 = 1;

/// Returns the CREATE-TABLE-and-INDEX SQL for a layer.
pub fn schema_sql(layer: DbLayer) -> &'static str {
    match layer {
        DbLayer::Graph => GRAPH_SQL,
        DbLayer::History => HISTORY_SQL,
        DbLayer::ToolCache => TOOL_CACHE_SQL,
        DbLayer::Tasks => TASKS_SQL,
        DbLayer::Semantic => SEMANTIC_SQL,
        DbLayer::Git => GIT_SQL,
        DbLayer::Memory => MEMORY_SQL,
        DbLayer::Errors => ERRORS_SQL,
        DbLayer::Multimodal => MULTIMODAL_SQL,
        DbLayer::Deps => DEPS_SQL,
        DbLayer::Tests => TESTS_SQL,
        DbLayer::Perf => PERF_SQL,
        DbLayer::Findings => FINDINGS_SQL,
        DbLayer::Agents => AGENTS_SQL,
        DbLayer::Refactors => REFACTORS_SQL,
        DbLayer::Contracts => CONTRACTS_SQL,
        DbLayer::Insights => INSIGHTS_SQL,
        DbLayer::LiveState => LIVE_STATE_SQL,
        DbLayer::Telemetry => TELEMETRY_SQL,
        DbLayer::Corpus => CORPUS_SQL,
        DbLayer::Audit => AUDIT_SQL,
        DbLayer::Meta => META_SQL,
    }
}

const VERSION_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

pub fn version_table_sql() -> &'static str {
    VERSION_TABLE
}

const GRAPH_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT UNIQUE NOT NULL,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER,
    language TEXT,
    parent_qualified TEXT,
    signature TEXT,
    modifiers TEXT,
    is_test INTEGER NOT NULL DEFAULT 0,
    file_hash TEXT,
    summary TEXT,
    embedding_id INTEGER,
    extra TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_nodes_qualified ON nodes(qualified_name);
CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);

CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    source_qualified TEXT NOT NULL,
    target_qualified TEXT NOT NULL,
    confidence TEXT NOT NULL,
    confidence_score REAL NOT NULL DEFAULT 1.0,
    file_path TEXT,
    line INTEGER,
    source_extractor TEXT NOT NULL,
    extra TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_qualified);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_qualified);
CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);

CREATE TABLE IF NOT EXISTS files (
    path TEXT PRIMARY KEY,
    sha256 TEXT NOT NULL,
    language TEXT,
    last_parsed_at TEXT NOT NULL DEFAULT (datetime('now')),
    line_count INTEGER,
    byte_count INTEGER
);

CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
    name, qualified_name, file_path, signature, summary,
    content='nodes', content_rowid='id', tokenize='porter'
);

CREATE TABLE IF NOT EXISTS hyperedges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    members TEXT NOT NULL,
    confidence TEXT NOT NULL,
    confidence_score REAL NOT NULL DEFAULT 1.0,
    extra TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

const HISTORY_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS turns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    token_count INTEGER,
    extra TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id, timestamp);

CREATE VIRTUAL TABLE IF NOT EXISTS turns_fts USING fts5(
    content, content='turns', content_rowid='id', tokenize='porter'
);

CREATE TABLE IF NOT EXISTS decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT,
    topic TEXT NOT NULL,
    problem TEXT NOT NULL,
    chosen TEXT NOT NULL,
    reasoning TEXT NOT NULL,
    alternatives TEXT NOT NULL DEFAULT '[]',
    artifacts TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_decisions_topic ON decisions(topic);

CREATE TABLE IF NOT EXISTS system_reminders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    text TEXT NOT NULL,
    received_at TEXT NOT NULL
);
"#;

const TOOL_CACHE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool TEXT NOT NULL,
    params_hash TEXT NOT NULL,
    params TEXT NOT NULL,
    result TEXT NOT NULL,
    session_id TEXT,
    cached_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    hit_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(tool, params_hash)
);
CREATE INDEX IF NOT EXISTS idx_tool_calls_lookup ON tool_calls(tool, params_hash);
CREATE INDEX IF NOT EXISTS idx_tool_calls_session ON tool_calls(session_id);
"#;

const TASKS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS steps (
    step_id TEXT PRIMARY KEY,
    parent_step_id TEXT REFERENCES steps(step_id),
    session_id TEXT NOT NULL,
    description TEXT NOT NULL,
    acceptance_cmd TEXT,
    acceptance_check TEXT NOT NULL DEFAULT 'null',
    status TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    verification_proof TEXT,
    artifacts TEXT NOT NULL DEFAULT '{}',
    notes TEXT NOT NULL DEFAULT '',
    blocker TEXT,
    drift_score INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_steps_session ON steps(session_id, status);
CREATE INDEX IF NOT EXISTS idx_steps_parent ON steps(parent_step_id);

CREATE TABLE IF NOT EXISTS roadmaps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    title TEXT NOT NULL,
    source_md TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

const SEMANTIC_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id INTEGER,
    text_hash TEXT NOT NULL,
    model TEXT NOT NULL,
    vector BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(text_hash, model)
);
CREATE INDEX IF NOT EXISTS idx_emb_node ON embeddings(node_id);

CREATE TABLE IF NOT EXISTS concepts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT UNIQUE NOT NULL,
    summary TEXT,
    embedding_id INTEGER REFERENCES embeddings(id),
    god_node_score REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS communities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    level INTEGER NOT NULL DEFAULT 0,
    parent_id INTEGER REFERENCES communities(id),
    cohesion REAL NOT NULL DEFAULT 0.0,
    size INTEGER NOT NULL DEFAULT 0,
    dominant_language TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS community_membership (
    community_id INTEGER NOT NULL REFERENCES communities(id),
    node_qualified TEXT NOT NULL,
    PRIMARY KEY(community_id, node_qualified)
);
"#;

const GIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS commits (
    sha TEXT PRIMARY KEY,
    author_name TEXT,
    author_email TEXT,
    committed_at TEXT NOT NULL,
    message TEXT NOT NULL,
    parent_sha TEXT
);
CREATE INDEX IF NOT EXISTS idx_commits_time ON commits(committed_at);

CREATE TABLE IF NOT EXISTS commit_files (
    sha TEXT NOT NULL REFERENCES commits(sha),
    file_path TEXT NOT NULL,
    additions INTEGER NOT NULL DEFAULT 0,
    deletions INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(sha, file_path)
);
CREATE INDEX IF NOT EXISTS idx_commit_files_path ON commit_files(file_path);

CREATE TABLE IF NOT EXISTS blame (
    file_path TEXT NOT NULL,
    line INTEGER NOT NULL,
    sha TEXT NOT NULL,
    author TEXT,
    PRIMARY KEY(file_path, line)
);
"#;

const MEMORY_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    rule TEXT NOT NULL,
    why TEXT NOT NULL,
    how_to_apply TEXT NOT NULL,
    applies_to TEXT NOT NULL DEFAULT '[]',
    source TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(scope, rule_id)
);

CREATE TABLE IF NOT EXISTS constraints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    rule TEXT NOT NULL,
    why TEXT NOT NULL,
    how_to_apply TEXT NOT NULL,
    applies_to TEXT NOT NULL DEFAULT '[]',
    source TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(scope, rule_id)
);
"#;

const ERRORS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    error_hash TEXT UNIQUE NOT NULL,
    message TEXT NOT NULL,
    stack TEXT,
    file_path TEXT,
    fix_summary TEXT,
    fix_diff TEXT,
    encounters INTEGER NOT NULL DEFAULT 1,
    first_seen TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_errors_hash ON errors(error_hash);
"#;

const MULTIMODAL_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    sha256 TEXT NOT NULL,
    media_type TEXT NOT NULL,
    extracted_text TEXT,
    elements TEXT,
    transcript TEXT,
    extracted_at TEXT NOT NULL DEFAULT (datetime('now')),
    extractor_version TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_type ON media(media_type);

CREATE VIRTUAL TABLE IF NOT EXISTS media_fts USING fts5(
    extracted_text, transcript, content='media', content_rowid='id', tokenize='porter'
);

CREATE TABLE IF NOT EXISTS screenshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    captured_at TEXT NOT NULL,
    path TEXT NOT NULL,
    media_id INTEGER REFERENCES media(id),
    label TEXT,
    diff_from_previous TEXT
);
"#;

const DEPS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS dependencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    package TEXT NOT NULL,
    version TEXT NOT NULL,
    ecosystem TEXT NOT NULL,
    license TEXT,
    is_dev INTEGER NOT NULL DEFAULT 0,
    last_upgrade TEXT,
    UNIQUE(ecosystem, package)
);

CREATE TABLE IF NOT EXISTS vulnerabilities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    advisory_id TEXT UNIQUE NOT NULL,
    package TEXT NOT NULL,
    affected_versions TEXT NOT NULL,
    severity TEXT NOT NULL,
    summary TEXT NOT NULL,
    discovered_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

const TESTS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS test_files (
    file_path TEXT PRIMARY KEY,
    framework TEXT,
    last_run_at TEXT,
    last_status TEXT,
    runtime_ms INTEGER
);

CREATE TABLE IF NOT EXISTS test_coverage (
    function_qualified TEXT NOT NULL,
    test_file TEXT NOT NULL REFERENCES test_files(file_path),
    coverage_pct REAL,
    PRIMARY KEY(function_qualified, test_file)
);

CREATE TABLE IF NOT EXISTS flaky_tests (
    test_id TEXT PRIMARY KEY,
    flake_count INTEGER NOT NULL DEFAULT 0,
    last_flake_at TEXT
);
"#;

const PERF_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS baselines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric TEXT NOT NULL,
    value REAL NOT NULL,
    unit TEXT,
    captured_at TEXT NOT NULL DEFAULT (datetime('now')),
    git_sha TEXT,
    notes TEXT
);
CREATE INDEX IF NOT EXISTS idx_baselines_metric ON baselines(metric, captured_at);
"#;

const FINDINGS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id TEXT NOT NULL,
    scanner TEXT NOT NULL,
    severity TEXT NOT NULL,
    file TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    column_start INTEGER NOT NULL,
    column_end INTEGER NOT NULL,
    message TEXT NOT NULL,
    suggestion TEXT,
    auto_fixable INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_findings_file ON findings(file);
CREATE INDEX IF NOT EXISTS idx_findings_severity ON findings(severity);
CREATE INDEX IF NOT EXISTS idx_findings_open ON findings(resolved_at) WHERE resolved_at IS NULL;
"#;

const AGENTS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS subagent_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,
    transcript TEXT,
    summary TEXT,
    cost_tokens INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subagent_session ON subagent_runs(session_id);
"#;

const REFACTORS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS refactors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    description TEXT NOT NULL,
    before_snapshot TEXT,
    after_snapshot TEXT,
    diff TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
    reverted_at TEXT
);
"#;

const CONTRACTS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS contracts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    contract_kind TEXT NOT NULL,
    name TEXT NOT NULL,
    schema TEXT NOT NULL,
    producer TEXT,
    consumers TEXT NOT NULL DEFAULT '[]',
    file_path TEXT,
    UNIQUE(contract_kind, name)
);
"#;

const INSIGHTS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS insights (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    title TEXT NOT NULL,
    body_md TEXT NOT NULL,
    generated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

const LIVE_STATE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS file_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor TEXT,
    happened_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_file_events_path_time ON file_events(file_path, happened_at);
"#;

const TELEMETRY_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool TEXT NOT NULL,
    latency_ms INTEGER NOT NULL,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL DEFAULT 1,
    happened_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_calls_tool_time ON calls(tool, happened_at);
"#;

const CORPUS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS corpus_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    item_type TEXT NOT NULL,
    extracted_at TEXT NOT NULL DEFAULT (datetime('now')),
    payload TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_corpus_path ON corpus_items(file_path);
"#;

const AUDIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    layer TEXT NOT NULL,
    target TEXT,
    prev_value_hash TEXT,
    new_value_hash TEXT,
    happened_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_audit_layer_time ON audit_log(layer, happened_at);
"#;

const META_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    root TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_indexed_at TEXT,
    schema_version INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projects_root ON projects(root);

CREATE TABLE IF NOT EXISTS project_links (
    a TEXT NOT NULL REFERENCES projects(id),
    b TEXT NOT NULL REFERENCES projects(id),
    relation TEXT NOT NULL,
    PRIMARY KEY(a, b, relation)
);
"#;
