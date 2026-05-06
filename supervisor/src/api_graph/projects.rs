//! `/api/projects` handler + supporting types and helpers, extracted
//! from `api_graph/mod.rs` (HIGH-45 split).
//!
//! Owns the full project-discovery surface: the response types
//! (`DiscoveredProject`, `ProjectsResponse`), the meta-db reader
//! (`MetaProjectRow`, `load_meta_projects`), the on-disk fallback
//! mtime helper (`newest_db_mtime_iso`), and the shard counter
//! (`count_table`). All of these are private to the api_graph module
//! tree — the only public symbol is `api_projects` which build_router
//! consumes.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use std::path::PathBuf;

use super::ApiGraphState;

/// One discovered shard under `<MNEME_HOME>/projects/<id>/`.
///
/// Wire shape kept stable for legacy callers (`id`, `path`, `has_graph_db`
/// are all the original fields) while the picker-oriented fields
/// (`hash`, `display_name`, `canonical_path`, `indexed_files`, `nodes`,
/// `edges`, `last_indexed_at`) are added alongside them. The frontend
/// reads the new fields via `vision/src/api/projects.ts`; older callers
/// that only know the original three keep working unchanged.
#[derive(Debug, Clone, Serialize)]
pub(super) struct DiscoveredProject {
    /// Hex project id (the SHA-256 hash of the project root path).
    /// Kept for back-compat; the picker uses the alias `hash`.
    pub(super) id: String,
    /// Alias for `id` exposed under the friendlier name the picker
    /// stores in `?project=<hash>` and `localStorage`. Same value.
    pub(super) hash: String,
    /// Absolute path to the project directory under
    /// `<MNEME_HOME>/projects/`. Useful for diagnostics.
    pub(super) path: PathBuf,
    /// Human-readable name from `meta.db::projects.name`, falling back
    /// to the hash itself when the row is missing.
    pub(super) display_name: String,
    /// Original project root that was hashed to produce `id`. Read from
    /// `meta.db::projects.root`; `None` when the meta-db row is missing.
    pub(super) canonical_path: Option<String>,
    /// `true` when `graph.db` exists in the project directory.
    pub(super) has_graph_db: bool,
    /// Count of `files` rows in `graph.db`. `0` when the shard is
    /// missing or the table can't be read.
    pub(super) indexed_files: i64,
    /// Count of `nodes` rows in `graph.db`.
    pub(super) nodes: i64,
    /// Count of `edges` rows in `graph.db`.
    pub(super) edges: i64,
    /// ISO-8601 timestamp from `meta.db::projects.last_indexed_at`,
    /// falling back to the newest `*.db` mtime on disk when the
    /// meta-db row hasn't been stamped yet (older builds).
    pub(super) last_indexed_at: Option<String>,
}

/// Response for `GET /api/projects`.
#[derive(Debug, Clone, Serialize)]
pub(super) struct ProjectsResponse {
    /// All discovered project directories (whether or not they have a
    /// graph.db). The picker disables entries with `has_graph_db == false`
    /// and surfaces them as "no shard" so the user sees the project
    /// exists but isn't queryable yet.
    pub(super) projects: Vec<DiscoveredProject>,
    /// Path that was scanned, for diagnostics.
    pub(super) projects_root: PathBuf,
}

/// One row from `meta.db::projects`. Used to enrich the `/api/projects`
/// response with human-readable names and canonical paths.
struct MetaProjectRow {
    name: String,
    root: String,
    last_indexed_at: Option<String>,
}

/// Read every row from `meta.db::projects` into a hash-keyed map. Returns
/// an empty map when meta.db doesn't exist (fresh install) or any read
/// fails — every consumer treats missing data as "no extra info" and
/// falls back to the legacy hash-only display.
fn load_meta_projects(state: &ApiGraphState) -> std::collections::HashMap<String, MetaProjectRow> {
    let meta_path = state.paths.meta_db();
    if !meta_path.is_file() {
        return std::collections::HashMap::new();
    }
    let conn = match rusqlite::Connection::open_with_flags(
        &meta_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(error = %e, db = %meta_path.display(), "open meta.db failed");
            return std::collections::HashMap::new();
        }
    };
    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
    let mut out = std::collections::HashMap::new();
    let mut stmt = match conn.prepare("SELECT id, name, root, last_indexed_at FROM projects") {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e, "meta.db: projects table missing");
            return out;
        }
    };
    let rows = match stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    }) {
        Ok(it) => it,
        Err(e) => {
            tracing::debug!(error = %e, "meta.db: projects scan failed");
            return out;
        }
    };
    for r in rows.flatten() {
        out.insert(
            r.0,
            MetaProjectRow {
                name: r.1,
                root: r.2,
                last_indexed_at: r.3,
            },
        );
    }
    out
}

/// Newest `*.db` mtime under `dir` as an ISO-8601 string. Used as a
/// fall-back `last_indexed_at` when meta.db hasn't stamped the project
/// yet (older builds, in-flight first-build).
fn newest_db_mtime_iso(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut newest: Option<std::time::SystemTime> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("db") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(t) = meta.modified() {
                newest = Some(match newest {
                    Some(prev) if prev >= t => prev,
                    _ => t,
                });
            }
        }
    }
    let t = newest?;
    let dt = chrono::DateTime::<chrono::Utc>::from(t);
    Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Sum a single COUNT(*) query against an open shard. Returns 0 on any
/// error so callers don't have to special-case missing tables.
fn count_table(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
}

/// `GET /api/projects` — list every directory under
/// `<MNEME_HOME>/projects/` augmented with summary stats and the
/// human-readable name from `meta.db::projects`.
///
/// The picker in `vision/src/App.tsx::ProjectPicker` calls this on
/// mount to populate the dropdown. Entries are sorted by
/// `last_indexed_at` descending so the most-recently-built project
/// surfaces first; ties fall back to hash-alphabetical so the order
/// stays stable when nothing has been built yet.
pub(super) async fn api_projects(State(state): State<ApiGraphState>) -> impl IntoResponse {
    let projects_root = state.paths.root().join("projects");
    let mut projects: Vec<DiscoveredProject> = Vec::new();
    let meta = load_meta_projects(&state);

    // Read the directory; if it doesn't exist (fresh install with no
    // build yet), return an empty list — that's a valid state.
    let entries = match std::fs::read_dir(&projects_root) {
        Ok(it) => it,
        Err(e) => {
            tracing::debug!(
                path = %projects_root.display(),
                error = %e,
                "api/projects: projects dir not present yet"
            );
            return Json(ProjectsResponse {
                projects,
                projects_root,
            });
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue, // non-UTF-8 dir name — skip silently
        };
        let graph_db = path.join("graph.db");
        let has_graph_db = graph_db.is_file();

        // Summary counts: nodes/edges/files from graph.db. Each open
        // is read-only and bounded by busy_timeout; failures degrade
        // to zero rather than killing the whole listing.
        let (nodes_count, edges_count, files_count) = if has_graph_db {
            match rusqlite::Connection::open_with_flags(
                &graph_db,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            ) {
                Ok(conn) => {
                    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
                    let n = count_table(&conn, "SELECT COUNT(*) FROM nodes");
                    let e = count_table(&conn, "SELECT COUNT(*) FROM edges");
                    let f = count_table(&conn, "SELECT COUNT(*) FROM files");
                    (n, e, f)
                }
                Err(_) => (0, 0, 0),
            }
        } else {
            (0, 0, 0)
        };

        // Friendly metadata from meta.db, falling back to the hash and
        // a fresh-on-disk mtime when the row hasn't been written yet.
        let (display_name, canonical_path, last_indexed_at) = match meta.get(&id) {
            Some(row) => (
                row.name.clone(),
                Some(row.root.clone()),
                row.last_indexed_at
                    .clone()
                    .or_else(|| newest_db_mtime_iso(&path)),
            ),
            None => (id.clone(), None, newest_db_mtime_iso(&path)),
        };

        projects.push(DiscoveredProject {
            id: id.clone(),
            hash: id,
            path,
            display_name,
            canonical_path,
            has_graph_db,
            indexed_files: files_count,
            nodes: nodes_count,
            edges: edges_count,
            last_indexed_at,
        });
    }

    // Most-recently-indexed first; ties broken by hash for stable
    // ordering when nothing has been built. `None` last so unbuilt
    // projects sink to the bottom of the dropdown.
    projects.sort_by(|a, b| match (&b.last_indexed_at, &a.last_indexed_at) {
        (Some(b_t), Some(a_t)) => b_t.cmp(a_t).then_with(|| a.hash.cmp(&b.hash)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.hash.cmp(&b.hash),
    });

    Json(ProjectsResponse {
        projects,
        projects_root,
    })
}
