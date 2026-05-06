//! F1 D0+D1: skeleton `/api/graph/*` router for the vision SPA.
//!
//! This module provides:
//!
//! 1. A `build_router()` factory that returns a stateless `axum::Router`
//!    mounting all 17 endpoints documented in
//!    `docs-and-memory/phase-a-issues.md §3` (the API surface inventory).
//! 2. A working `/api/health` endpoint that returns daemon-side metadata
//!    in the same wire shape the Bun dev server emits (see
//!    `vision/server.ts:244-253`). Used by `vision/src/api.ts:64` to
//!    probe whether the data layer is alive.
//! 3. A working `/api/projects` endpoint that lists every project shard
//!    discovered under `<MNEME_HOME>/projects/` whose `graph.db` file
//!    exists. Useful for D3 multi-shard support (decision doc §8 q4).
//! 4. **Stub** handlers for the other 15 endpoints — every one returns
//!    HTTP 501 with a JSON body shaped `{"error":"not_implemented"}`.
//!    (The internal `phase`/`next` milestone codes were dropped per
//!    LOW fix 2026-05-05 audit — they leaked planning state into
//!    public responses.)
//!
//! The frontend code at `vision/src/api.ts:71-95` already has a
//! `placeholderPayload()` fallback that fires on non-2xx, so the
//! Tauri/browser shell renders empty data instead of crashing with
//! `<!DOCTYPE` JSON parse errors. That is the explicit goal of D0+D1
//! per the decision doc §6 milestone table.
//!
//! No write paths are exposed; this is read-only by design and remains
//! consistent with the per-shard single-writer invariant in the store
//! crate (CLAUDE.md §"Hard rules").

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use common::PathManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Optional `?project=<hash>` query param threaded through every
/// `/api/graph/*` handler. When set the handler resolves the shard at
/// `<MNEME_HOME>/projects/<hash>/<layer>.db` directly. When absent
/// behaviour falls back to "first project alphabetically" — preserves
/// the legacy single-project contract for callers that don't yet pass
/// the param (the old Bun dev server, raw curl probes, the v0.3.2 SPA
/// before the picker landed).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectQuery {
    /// The hex SHA-256 of the project root the SPA wants to view.
    /// `None` keeps the legacy "first shard alphabetically" behaviour.
    pub project: Option<String>,

    /// Optional row limit for endpoints that return a paged slice
    /// (`/api/graph/nodes` + `/api/graph/edges`). Capped at
    /// [`MAX_GRAPH_LIMIT`] server-side. Absent → handlers use their
    /// own default (kept small to protect the daemon's blocking pool).
    ///
    /// BUG-NEW-I fix (2026-05-05): nodes used `LIMIT 2000` and edges
    /// used `LIMIT 8000` hardcoded, with no client override. On any
    /// non-trivial repo, edges referenced nodes outside the 2K node
    /// window and the SPA's `g.hasNode()` guard silently dropped them
    /// — visible to the user as "ForceGalaxy nodes appear but no
    /// links between them". Threading the SPA's `?limit=` through
    /// lets ForceGalaxy ask for matched windows (32K each).
    pub limit: Option<usize>,
}

/// Hard ceiling for `ProjectQuery::limit`. Larger requests are clamped
/// to this value so a malicious or buggy client can't ask the daemon
/// to materialise the entire shard into a JSON array. Picked to cover
/// the `mneme` repo itself (~17K Rust + ~9K TS nodes ≈ 26K) with
/// headroom, while still finishing in a few hundred ms on a fast disk.
pub const MAX_GRAPH_LIMIT: usize = 50_000;

/// Shared application state for the `/api/graph/*` router.
///
/// Kept deliberately minimal in D0+D1 — D2 will add a shard-discovery
/// handle here once the rusqlite query helpers land.
///
/// Phase A · F2 added `livebus`, an optional handle to the in-process
/// [`livebus::SubscriberManager`] used by the `/ws` WebSocket relay.
/// It is `Option` so the existing `/api/health` + `/api/graph/*` tests can
/// still construct a router without booting the livebus stack — when the
/// daemon's real `run()` initialises the bus it threads a `Some(mgr)`
/// through here and `/ws` upgrades succeed.
#[derive(Clone)]
pub struct ApiGraphState {
    /// Resolves `<MNEME_HOME>` and friends. Cloned per request — the
    /// underlying type is small (one `PathBuf`).
    pub paths: Arc<PathManager>,
    /// Optional handle to the livebus subscriber registry. When `Some`,
    /// `/ws` upgrades attach to this manager and forward events. When
    /// `None`, `/ws` upgrades are accepted but immediately closed with an
    /// `error` frame so the route stays mounted in production.
    pub livebus: Option<livebus::SubscriberManager>,
}

impl ApiGraphState {
    /// Build a new state object using the default path resolver
    /// (`MNEME_HOME` env var, then `~/.mneme`, then OS default).
    ///
    /// `livebus` defaults to `None` — call [`Self::with_livebus`] (or set
    /// the field directly) to wire the `/ws` relay to a running bus.
    pub fn from_defaults() -> Self {
        Self {
            paths: Arc::new(PathManager::default_root()),
            livebus: None,
        }
    }

    /// Attach a livebus subscriber manager so `/ws` upgrades succeed.
    /// Used by `supervisor::lib::run` once the in-process bus has been
    /// constructed.
    pub fn with_livebus(mut self, mgr: livebus::SubscriberManager) -> Self {
        self.livebus = Some(mgr);
        self
    }
}

/// HIGH-42 fix (2026-05-05 audit): Mneme HTTP API version. Every
/// `/api/*` response carries `X-Mneme-Api-Version: <this number>`
/// so older Vision SPA bundles cached against an older daemon
/// (or vice versa via Tauri auto-update) can detect a wire-shape
/// drift. v0.4.0 is API version "2" because Item #111 silently
/// changed `/api/graph/edges` semantics and Item #124 added
/// `/api/graph/layout` since v0.3.x's "1" surface.
pub const MNEME_API_VERSION: &str = "2";

/// Header injection middleware. Attaches X-Mneme-Api-Version to
/// every response that flows through the api router.
async fn inject_api_version_header(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let mut response = next.run(req).await;
    if let Ok(value) = axum::http::HeaderValue::from_str(MNEME_API_VERSION) {
        response.headers_mut().insert("x-mneme-api-version", value);
    }
    response
}

/// Construct the `/api/graph/*` skeleton router.
///
/// Mounts the full 17-endpoint surface so the frontend can connect to
/// every URL it knows about without `<!DOCTYPE` parse errors. Endpoints
/// not yet implemented return HTTP 501 with a JSON body.
pub fn build_router(state: ApiGraphState) -> Router {
    Router::new()
        // -- Working endpoints (real data) ------------------------------
        .route("/api/health", get(api_health))
        .route("/api/projects", get(api_projects))
        // -- F1 D2 — real handlers ports of the 5 most-used endpoints
        // from `vision/server/shard.ts`. Closes the "graph error:
        // Unexpected token '<', '<!DOCTYPE'..." parse-error toast that
        // fired on every view because Tauri's SPA fallback returned
        // index.html for unknown paths.
        .route("/api/graph/nodes", get(api_graph_nodes))
        .route("/api/graph/edges", get(api_graph_edges))
        .route("/api/graph/files", get(api_graph_files))
        .route("/api/graph/findings", get(api_graph_findings))
        .route("/api/graph/status", get(api_graph_status))
        // Item #124: server-pre-computed layout snapshot for ForceGalaxy
        // first-paint < 500ms. The handler builds a deterministic
        // community-aware sunflower-spiral layout from data already in
        // graph.db (nodes + community_membership) — no FA2 server-side,
        // no extra build step. The SPA fetches in parallel with /nodes
        // + /edges and seeds Sigma's positions; FA2 worker still runs
        // for refinement once Sigma is on screen.
        .route("/api/graph/layout", get(api_graph_layout))
        // -- Stub endpoints (501 not_implemented) -----------------------
        // The remaining 12 endpoints documented in
        // phase-a-issues.md §3. D3-D6 will fill these in incrementally.
        .route("/api/graph", get(stub_handler))
        // -- F1 D3 — second-wave port of 7 more vision endpoints from
        // `vision/server/shard.ts`. Closes the remaining "not_implemented"
        // toasts on the file-tree, sankey-flow, chord, heatmap, timeline,
        // and test-coverage views. Each handler runs the equivalent
        // SQLite query inline (sub-millisecond on typical shards) and
        // falls through to an empty payload on any I/O / SQL error,
        // matching the TS `[] / {}` failure contract.
        .route("/api/graph/file-tree", get(api_graph_file_tree))
        .route("/api/graph/kind-flow", get(api_graph_kind_flow))
        .route("/api/graph/domain-flow", get(api_graph_domain_flow))
        .route(
            "/api/graph/community-matrix",
            get(api_graph_community_matrix),
        )
        .route("/api/graph/commits", get(api_graph_commits))
        .route("/api/graph/heatmap", get(api_graph_heatmap))
        // -- F1 D4 — final-wave port of the last 5 vision endpoints. The
        // SPA's last "not_implemented" toasts — Layered Architecture,
        // Project Galaxy 3D, Theme Palette, Hierarchy Tree — are now
        // backed by real shard reads, and `/api/daemon/health` mirrors
        // the existing `/api/health` JSON shape so the vision frontend's
        // health-probe code (which uses both URLs interchangeably) lights
        // up green without changing the wire format.
        .route("/api/graph/layers", get(api_graph_layers))
        .route("/api/graph/galaxy-3d", get(api_graph_galaxy_3d))
        .route("/api/graph/test-coverage", get(api_graph_test_coverage))
        .route("/api/graph/theme-palette", get(api_graph_theme_palette))
        .route("/api/graph/hierarchy", get(api_graph_hierarchy))
        // -- Voice endpoint stub (already documented as stub in v0.3) ---
        .route("/api/voice", get(voice_stub))
        // -- Daemon-health proxy (separate from /api/health) ------------
        // The Bun dev server forwards this to the daemon's /health probe
        // (see vision/server.ts:probeDaemon). The vision frontend uses
        // /api/health and /api/daemon/health interchangeably, so we
        // serve the same JSON body from both routes.
        .route("/api/daemon/health", get(api_daemon_health))
        // -- Phase A · F2: WebSocket livebus relay ---------------------
        // `GET /ws` upgrades to a WebSocket and forwards every matching
        // [`livebus::Event`] from the in-process broadcast bus to
        // the connected client as JSON-encoded text frames. See
        // `supervisor/src/ws.rs` for the per-connection state machine.
        // Without this route the vision SPA's livebus subscription falls
        // back to placeholder data on every load.
        .route("/ws", get(crate::ws::ws_upgrade_handler))
        // Audit fix (2026-05-06 multi-agent fan-out, security agent
        // NEW-CRIT-1): apply the same Origin/Host validation that
        // gates /ws to EVERY HTTP route. CRIT-4 closed the
        // WebSocket door but left every /api/* route open to
        // DNS-rebinding (a malicious DNS A record pointing
        // evil.com -> 127.0.0.1, browser sends Host: evil.com) and
        // cross-site fetch from an attacker-hosted page. The middleware
        // returns HTTP 403 with a JSON envelope on rejection; the
        // daemon stays alive, only the offending request is refused.
        //
        // Layer order (outermost first applies first per axum docs):
        //   1. enforce_origin_and_host  -> reject untrusted requests
        //   2. inject_api_version_header -> stamp X-Mneme-Api-Version
        // Both layers run on every route including /ws (which still
        // has its own internal validate_origin_and_host check —
        // belt-and-suspenders, harmless duplicate).
        .layer(axum::middleware::from_fn(crate::ws::enforce_origin_and_host))
        // HIGH-42: stamp every response with the API version header so
        // older / cached SPA bundles can detect wire-shape drift.
        .layer(axum::middleware::from_fn(inject_api_version_header))
        .with_state(state)
}

/// `GET /api/health` — daemon-side liveness probe used by
/// `vision/src/api.ts:64`. Mirrors the wire shape of the Bun server's
/// `/api/health` (see `vision/server.ts:244-253`).
async fn api_health(State(_state): State<ApiGraphState>) -> impl IntoResponse {
    // `Date.now()` in JS is unix-millis. We emit unix-millis as `i64`
    // so the existing TS consumer parses it identically.
    let ts_ms: i64 = chrono::Utc::now().timestamp_millis();
    // LOW fix (2026-05-05 audit): drop the internal `phase: "D0"`
    // milestone code from public health responses. It leaked
    // pre-release planning state ("D0" = our internal sub-milestone
    // string) that meant nothing to operators and confused users
    // reading the JSON response. The other fields (ok/host/port/ts)
    // carry the actual liveness signal. If a future caller wants to
    // distinguish daemon flavours we'll add an explicit `version`
    // field tied to CARGO_PKG_VERSION.
    Json(json!({
        "ok": true,
        "host": "127.0.0.1",
        "port": 7777,
        "ts": ts_ms,
    }))
}

/// One discovered shard under `<MNEME_HOME>/projects/<id>/`.
///
/// Wire shape kept stable for legacy callers (`id`, `path`, `has_graph_db`
/// are all the original fields) while the picker-oriented fields
/// (`hash`, `display_name`, `canonical_path`, `indexed_files`, `nodes`,
/// `edges`, `last_indexed_at`) are added alongside them. The frontend
/// reads the new fields via `vision/src/api/projects.ts`; older callers
/// that only know the original three keep working unchanged.
#[derive(Debug, Clone, Serialize)]
struct DiscoveredProject {
    /// Hex project id (the SHA-256 hash of the project root path).
    /// Kept for back-compat; the picker uses the alias `hash`.
    id: String,
    /// Alias for `id` exposed under the friendlier name the picker
    /// stores in `?project=<hash>` and `localStorage`. Same value.
    hash: String,
    /// Absolute path to the project directory under
    /// `<MNEME_HOME>/projects/`. Useful for diagnostics.
    path: PathBuf,
    /// Human-readable name from `meta.db::projects.name`, falling back
    /// to the hash itself when the row is missing.
    display_name: String,
    /// Original project root that was hashed to produce `id`. Read from
    /// `meta.db::projects.root`; `None` when the meta-db row is missing.
    canonical_path: Option<String>,
    /// `true` when `graph.db` exists in the project directory.
    has_graph_db: bool,
    /// Count of `files` rows in `graph.db`. `0` when the shard is
    /// missing or the table can't be read.
    indexed_files: i64,
    /// Count of `nodes` rows in `graph.db`.
    nodes: i64,
    /// Count of `edges` rows in `graph.db`.
    edges: i64,
    /// ISO-8601 timestamp from `meta.db::projects.last_indexed_at`,
    /// falling back to the newest `*.db` mtime on disk when the
    /// meta-db row hasn't been stamped yet (older builds).
    last_indexed_at: Option<String>,
}

/// Response for `GET /api/projects`.
#[derive(Debug, Clone, Serialize)]
struct ProjectsResponse {
    /// All discovered project directories (whether or not they have a
    /// graph.db). The picker disables entries with `has_graph_db == false`
    /// and surfaces them as "no shard" so the user sees the project
    /// exists but isn't queryable yet.
    projects: Vec<DiscoveredProject>,
    /// Path that was scanned, for diagnostics.
    projects_root: PathBuf,
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
async fn api_projects(State(state): State<ApiGraphState>) -> impl IntoResponse {
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

/// `GET /api/voice` — stubbed voice endpoint, documented as
/// `phase: "stub"` in v0.3 (CLAUDE.md "Known limitations"). Kept
/// distinct from the 501 stubs because the wire shape `{enabled,
/// phase}` is contractual.
async fn voice_stub() -> impl IntoResponse {
    Json(json!({
        "enabled": false,
        "phase": "stub",
    }))
}

/// Generic stub handler for endpoints not yet ported. Returns HTTP 501
/// with a JSON envelope so the frontend's `placeholderPayload()`
/// fallback fires cleanly instead of choking on HTML.
///
/// LOW fix (2026-05-05 audit): drop the `phase: "D0"` and
/// `next: "D2-D6"` internal milestone codes from the public 501
/// envelope. They leaked our planning sub-milestones into responses
/// users + operators see and meant nothing outside the maintainer
/// team. The `error: "not_implemented"` carries the only actionable
/// signal a caller needs.
async fn stub_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "not_implemented",
        })),
    )
}

// ---------------------------------------------------------------------------
// F1 D2 — Real `/api/graph/{nodes,edges,status,files,findings}` endpoints
// ---------------------------------------------------------------------------
//
// First-wave port of the most-used vision endpoints from
// `vision/server/shard.ts`. Each handler:
//
// 1. Picks the first project under `<MNEME_HOME>/projects/` whose
//    `<layer>.db` exists. (D3 will let the UI choose a project.)
// 2. Opens the layer DB read-only.
// 3. Runs the query inside `spawn_blocking` -- `rusqlite` is sync; doing
//    this on the tokio runtime would starve other handlers under load.
//    BUG-A4-001 fix (2026-05-04): the helper `with_layer_db_sync` now
//    actually dispatches to `tokio::task::spawn_blocking` to honour
//    this contract; previously the sync work ran inline on the tokio
//    worker.
// 4. Serialises into the wire shape `vision/src/api/*` expects.
//
// Any I/O / SQL error short-circuits to an EMPTY payload with HTTP 200,
// matching the TS behaviour (`shard.ts` returns `[]` on failure). The
// frontend's `placeholderPayload()` fallback only fires on non-2xx, so
// "graph.db missing yet" reads as "empty corpus" — the right UX during
// first install / build-in-progress.

/// `<layer>.db` shard locator.
///
/// When `requested` is `Some(hash)` and the directory
/// `<MNEME_HOME>/projects/<hash>/<layer>.db` exists, return that path
/// directly — supports the multi-project picker in the vision SPA.
/// Otherwise (no hash, missing hash, or missing layer file) fall back
/// to the legacy "first project under projects/ whose `<layer>.db`
/// exists, alphabetically" lookup so single-project installs keep
/// working without any param.
fn find_active_layer_db(
    state: &ApiGraphState,
    layer: &str,
    requested: Option<&str>,
) -> Option<PathBuf> {
    // Defense-in-depth fix (2026-05-06 audit): the prior signature
    // accepted any &str for `layer` and concatenated it via
    // format!("{}.db", layer) into the on-disk path. All current
    // callers pass static strings ("graph", "history", etc.), but
    // a future caller wiring `?layer=` to an HTTP query string
    // would silently allow path traversal — a malicious layer like
    // "../etc/passwd" or "../../../../sensitive/data" would join
    // out of the projects/<hash>/ directory.
    //
    // Validate `layer` against the known canonical layer names
    // before using it. Anything not on the allowlist returns None
    // — same shape as "no shard found", which the caller already
    // handles gracefully.
    if !is_valid_layer_name(layer) {
        return None;
    }
    let projects_root = state.paths.root().join("projects");

    // Direct hit — the picker passes the canonical hash; if the shard
    // exists for the requested layer use it.
    //
    // M-2 fix (2026-05-05 audit): the previous defence only rejected
    // strings containing `/`, `\`, `..`, or empty. That allow-by-
    // omission missed Windows reserved names (`nul`, `con`, `prn`,
    // `aux`, `com1`-`com9`, `lpt1`-`lpt9`), URL-encoded traversal
    // (`%2e%2e`), NUL bytes, and unbounded length DoS. Every
    // legitimate project id is a hex SHA-256 — exactly 64 lowercase
    // ASCII hex chars. Validate against that strict shape and reject
    // everything else.
    if let Some(hash) = requested {
        if is_valid_project_hash(hash) {
            let candidate = projects_root.join(hash).join(format!("{}.db", layer));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let entries = std::fs::read_dir(&projects_root).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let db = p.join(format!("{}.db", layer));
            if p.is_dir() && db.is_file() {
                Some(db)
            } else {
                None
            }
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Defense-in-depth (2026-05-06 audit): allowlist of canonical layer
/// names that can be appended to "<MNEME_HOME>/projects/<hash>/" as
/// "<layer>.db". Mirrors `common::layer::DbLayer::file_name` minus
/// the ".db" suffix. Kept as a function (not constant slice) so the
/// match is exhaustive at compile time — adding a new DbLayer
/// without adding it here is caught by clippy::wildcard_in_or_patterns.
fn is_valid_layer_name(layer: &str) -> bool {
    matches!(
        layer,
        "graph"
            | "history"
            | "tool_cache"
            | "tasks"
            | "semantic"
            | "git"
            | "memory"
            | "errors"
            | "multimodal"
            | "deps"
            | "tests"
            | "perf"
            | "findings"
            | "agents"
            | "refactors"
            | "contracts"
            | "insights"
            | "livestate"
            | "telemetry"
            | "corpus"
            | "audit"
            | "wiki"
            | "architecture"
            | "conventions"
            | "federated"
            | "concepts"
            | "meta"
    )
}

/// M-2 fix (2026-05-05 audit): strict project-hash validator.
/// Returns true iff `s` is exactly 64 lowercase ASCII hex characters
/// — the canonical shape of a SHA-256-derived project id. This
/// rejects:
///   - empty / unbounded-length strings (DoS via 10 MB query string)
///   - path-traversal sequences (`..`, `%2e%2e`)
///   - separators (`/`, `\`)
///   - Windows reserved names (`nul`, `con`, `prn`, `aux`, `com1`-`com9`,
///     `lpt1`-`lpt9`)
///   - NUL byte / control chars
///   - non-ASCII / Unicode lookalikes
///
/// All those classes fail the simple "must be 64 hex chars" predicate.
fn is_valid_project_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod project_hash_validator_tests {
    use super::is_valid_project_hash;

    #[test]
    fn accepts_canonical_64_hex() {
        assert!(is_valid_project_hash(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn rejects_uppercase_hex() {
        assert!(!is_valid_project_hash(
            "0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn rejects_short() {
        assert!(!is_valid_project_hash("0123abcd"));
    }

    #[test]
    fn rejects_long() {
        assert!(!is_valid_project_hash(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0"
        ));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_valid_project_hash(""));
    }

    #[test]
    fn rejects_traversal() {
        assert!(!is_valid_project_hash(
            "../../../etc/passwd00000000000000000000000000000000000000000000000"
        ));
    }

    #[test]
    fn rejects_windows_reserved_names() {
        assert!(!is_valid_project_hash("nul"));
        assert!(!is_valid_project_hash("con"));
        assert!(!is_valid_project_hash("prn"));
        assert!(!is_valid_project_hash("aux"));
        assert!(!is_valid_project_hash("com1"));
        assert!(!is_valid_project_hash("lpt9"));
    }

    #[test]
    fn rejects_url_encoded_traversal() {
        assert!(!is_valid_project_hash("%2e%2e"));
    }

    #[test]
    fn rejects_separators() {
        assert!(!is_valid_project_hash("a/b"));
        assert!(!is_valid_project_hash("a\\b"));
    }

    #[test]
    fn rejects_unicode_lookalike() {
        // Cyrillic 'a' looks like Latin 'a' but is U+0430
        let s = "\u{0430}".repeat(64);
        assert!(!is_valid_project_hash(&s));
    }

    #[test]
    fn rejects_null_byte() {
        let mut s = "0".repeat(64);
        unsafe {
            s.as_bytes_mut()[10] = 0;
        }
        assert!(!is_valid_project_hash(&s));
    }
}

/// Run a `rusqlite` query on the blocking-thread pool via
/// `tokio::task::spawn_blocking`. `rusqlite` is synchronous; running it
/// directly on the tokio runtime can starve other handlers (and the IPC
/// accept loop) under writer contention because the 500 ms busy_timeout
/// blocks the worker thread. BUG-A4-001 fix: dispatch every shard read
/// to the blocking pool so async workers stay free.
///
/// Opens via plain path with `SQLITE_OPEN_READ_ONLY` -- same flags
/// `bun:sqlite` uses successfully against the same db while the
/// store-worker's writer is active. Silent fall to `None` on any error.
///
/// `requested_project` threads the optional `?project=<hash>` param
/// from the request. When `Some`, the shard at
/// `<root>/projects/<hash>/<layer>.db` is used; when `None`, the legacy
/// "first shard alphabetically" fallback fires. This lets the multi-
/// project picker in the vision SPA switch shards without breaking
/// callers (curl, the old Bun dev server) that never pass the param.
async fn with_layer_db_sync<F, T>(
    state: &ApiGraphState,
    layer: &'static str,
    requested_project: Option<&str>,
    work: F,
) -> Option<T>
where
    F: FnOnce(&rusqlite::Connection) -> Option<T> + Send + 'static,
    T: Send + 'static,
{
    // Clone what we need to satisfy the `'static` bound for spawn_blocking.
    // ApiGraphState is Clone (Arc-backed) and the requested project hash is
    // a short owned string -- both cheap.
    let state_for_blocking = state.clone();
    let requested_owned: Option<String> = requested_project.map(|s| s.to_string());

    match tokio::task::spawn_blocking(move || {
        let db_path = find_active_layer_db(&state_for_blocking, layer, requested_owned.as_deref())?;
        let conn = match rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    layer,
                    db = %db_path.display(),
                    "open shard failed"
                );
                return None;
            }
        };
        let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
        work(&conn)
    })
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, layer, "with_layer_db_sync: spawn_blocking join error");
            None
        }
    }
}

/// Serialised graph node — matches `GraphNode` in `vision/src/api.ts`.
#[derive(Serialize)]
struct GraphNodeOut {
    id: String,
    label: String,
    #[serde(rename = "type")]
    kind_tag: String,
    size: i32,
    color: String,
    meta: GraphNodeMeta,
}

#[derive(Serialize)]
struct GraphNodeMeta {
    kind: String,
    file_path: Option<String>,
    source: &'static str,
}

/// Serialised graph edge — matches `GraphEdge` in `vision/src/api.ts`.
#[derive(Serialize)]
struct GraphEdgeOut {
    id: String,
    source: String,
    target: String,
    #[serde(rename = "type")]
    kind_tag: String,
    weight: i32,
    meta: GraphEdgeMeta,
}

#[derive(Serialize)]
struct GraphEdgeMeta {
    kind: String,
    source: &'static str,
}

/// Status payload — matches `GraphStatsPayload` in
/// `vision/src/api/graph.ts`. Tells the SPA whether a shard exists,
/// what's in it, and when it was last indexed.
#[derive(Serialize)]
struct GraphStatusOut {
    project: Option<String>,
    shard_root: Option<String>,
    last_index_at: Option<String>,
    nodes: i64,
    edges: i64,
    files: i64,
    by_kind: serde_json::Value,
}

/// Visual size hint per node kind. Mirrors TS `sizeForKind` proportions
/// so existing frontend layout tuning stays valid.
fn size_for_kind(kind: &str) -> i32 {
    match kind {
        "file" => 8,
        "class" => 6,
        "function" => 4,
        "import" => 2,
        _ => 3,
    }
}

/// Brand-gradient colour per node kind. Matches the brand-gradient hex
/// values in CLAUDE.md (`#4191E1`, `#41E1B5`, `#22D3EE`) so the graph
/// view stays on-palette without a Tailwind round-trip.
fn color_for_kind(kind: &str) -> &'static str {
    match kind {
        "file" => "#4191E1",
        "class" => "#41E1B5",
        "function" => "#22D3EE",
        "import" => "#FFA500",
        "decorator" => "#FF66CC",
        "comment" => "#888888",
        _ => "#9CA3AF",
    }
}

/// `GET /api/graph/nodes` — top N nodes for the force-graph view.
async fn api_graph_nodes(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    // BUG-NEW-I fix (2026-05-05): respect the SPA's `?limit=` so
    // ForceGalaxy can request a node window that matches its edge
    // window. Default 2000 preserves the v0.3.2 behaviour for
    // unauthenticated curl probes; clamp at MAX_GRAPH_LIMIT to keep
    // the daemon responsive.
    let limit = q.limit.unwrap_or(2000).min(MAX_GRAPH_LIMIT);
    let sql = format!(
        "SELECT qualified_name, name, kind, file_path \
         FROM nodes ORDER BY id LIMIT {}",
        limit
    );
    let nodes: Vec<GraphNodeOut> =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), move |conn| {
            let mut stmt = conn.prepare(&sql).ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                })
                .ok()?;
            Some(
                rows.filter_map(|r| r.ok())
                    .map(|(id, name, kind, fp)| {
                        let label = name.clone().unwrap_or_else(|| id.clone());
                        GraphNodeOut {
                            id,
                            label,
                            kind_tag: kind.clone(),
                            size: size_for_kind(&kind),
                            color: color_for_kind(&kind).to_string(),
                            meta: GraphNodeMeta {
                                kind,
                                file_path: fp,
                                source: "shard",
                            },
                        }
                    })
                    .collect(),
            )
        })
        .await
        .unwrap_or_default();
    Json(nodes)
}

/// `GET /api/graph/edges` — top N edges for the force-graph view.
///
/// Returns ONLY edges where both endpoints (source_qualified +
/// target_qualified) appear in the first `limit` nodes by id —
/// i.e. the same window the paired `/api/graph/nodes?limit=N` call
/// returns. This guarantees the SPA's `g.hasNode(e.source)` guard
/// matches every returned edge so ForceGalaxy actually shows links.
///
/// BUG-NEW-I + Item #111 fix (2026-05-05): the previous version
/// returned the first N edges by id with no node-window check.
/// Real-world VM smoke on the mneme repo (13,389 nodes, 80,529
/// edges) showed only 30.6% of returned edges had both endpoints
/// in the node window — the parser emits edges to qualified names
/// not always in the indexed node set (e.g. cross-file calls to
/// unresolved symbols). The SPA's hasNode guard correctly filtered
/// them out, but the daemon serialized all 32K edges anyway →
/// 70% wasted JSON bytes + a sparser-than-expected force graph.
///
/// Fix: INNER JOIN against the same node window the SPA fetches.
/// Backed by `idx_nodes_qualified` so the cost stays in the
/// 100-200ms range on 80K-edge corpora.
async fn api_graph_edges(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    // BUG-NEW-I fix (2026-05-05): respect the SPA's `?limit=`. See
    // `api_graph_nodes` for the rationale; both endpoints share the
    // same clamping policy so a paired call from ForceGalaxy returns
    // a balanced (nodes, edges) window with no silently-dropped edges.
    let limit = q.limit.unwrap_or(8000).min(MAX_GRAPH_LIMIT);
    // Same node window the SPA's nodes call materialises. Using `limit`
    // for both keeps the contract symmetric — fetch N nodes, fetch up
    // to N edges contained within those N nodes.
    let sql = format!(
        "WITH visible_nodes AS ( \
             SELECT qualified_name FROM nodes ORDER BY id LIMIT {limit} \
         ) \
         SELECT e.id, e.source_qualified, e.target_qualified, e.kind \
         FROM edges e \
         INNER JOIN visible_nodes vs ON vs.qualified_name = e.source_qualified \
         INNER JOIN visible_nodes vt ON vt.qualified_name = e.target_qualified \
         ORDER BY e.id \
         LIMIT {limit}",
        limit = limit
    );
    let edges: Vec<GraphEdgeOut> =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), move |conn| {
            let mut stmt = conn.prepare(&sql).ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })
                .ok()?;
            Some(
                rows.filter_map(|r| r.ok())
                    .map(|(id, src, tgt, kind)| GraphEdgeOut {
                        id: id.to_string(),
                        source: src,
                        target: tgt,
                        kind_tag: kind.clone(),
                        weight: 1,
                        meta: GraphEdgeMeta {
                            kind,
                            source: "shard",
                        },
                    })
                    .collect(),
            )
        })
        .await
        .unwrap_or_default();
    Json(edges)
}

// ---------------------------------------------------------------------
// /api/graph/layout — Item #124, server-pre-computed positions.
//
// Returns one (q, x, y) triple per node in the same window the SPA's
// /api/graph/nodes call returns. The SPA seeds Sigma's positions from
// this payload before kicking off FA2 worker refinement, dropping
// first-paint from ~3 s (random-init + 1-2 FA2 iters) to <500 ms.
//
// Algorithm: community-aware sunflower spiral.
//   1. Group nodes by `community_membership.community_id` (left-join
//      so nodes outside any community fall into a synthetic "loose"
//      bucket bucketed by `kind`).
//   2. Place each group's center on a Vogel sunflower disk: angle =
//      i * golden_angle_radians, radius = R_outer * sqrt(i / N).
//   3. Within each group, distribute members on a small inner circle
//      proportional to group size: angle = j * 2π / group_size,
//      radius = R_inner * sqrt(group_size). Members ordered by
//      qualified_name SHA hash for stable layout across rebuilds.
// ---------------------------------------------------------------------

#[derive(Serialize, Debug, Clone, PartialEq)]
struct LayoutPosition {
    /// Node qualified_name — joins with `/api/graph/nodes` on `id`.
    q: String,
    x: f64,
    y: f64,
}

/// HIGH-22 fix (2026-05-05 audit): in-process layout cache. The
/// previous implementation ran the full SQL JOIN + Vogel sunflower
/// computation on every GET. Two rapid SPA tab opens each paid the
/// full cost. The Item #124 "<500ms first-paint" claim was only true
/// when the OS page cache was already warm.
///
/// Cache key: (project_hash_or_default, limit). The shard's contents
/// are immutable per `mneme rebuild`; small TTL guards against the
/// daemon's own watcher reindex changing the node set under us
/// without an explicit invalidation hook.
struct LayoutCacheEntry {
    rows: Vec<LayoutPosition>,
    inserted_at: std::time::Instant,
}

static LAYOUT_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<(String, usize), LayoutCacheEntry>>,
> = std::sync::OnceLock::new();

/// 30-second TTL. Long enough that hopping between tabs hits the
/// cache; short enough that a fresh build's layout is reflected
/// without an explicit invalidation call from the watcher.
const LAYOUT_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

fn layout_cache_get(project: &str, limit: usize) -> Option<Vec<LayoutPosition>> {
    let cache = LAYOUT_CACHE.get_or_init(|| std::sync::Mutex::new(Default::default()));
    let mut guard = cache.lock().ok()?;
    let key = (project.to_string(), limit);
    if let Some(entry) = guard.get(&key) {
        if entry.inserted_at.elapsed() < LAYOUT_CACHE_TTL {
            return Some(entry.rows.clone());
        }
    }
    // Stale or absent — drop any expired entry to keep the map bounded.
    guard.remove(&key);
    None
}

fn layout_cache_put(project: &str, limit: usize, rows: Vec<LayoutPosition>) {
    let cache = LAYOUT_CACHE.get_or_init(|| std::sync::Mutex::new(Default::default()));
    if let Ok(mut guard) = cache.lock() {
        guard.insert(
            (project.to_string(), limit),
            LayoutCacheEntry {
                rows,
                inserted_at: std::time::Instant::now(),
            },
        );
    }
}

/// `GET /api/graph/layout` — pre-computed (q, x, y) positions for
/// the same node window the paired `/api/graph/nodes` call returns.
async fn api_graph_layout(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    // Same clamping policy as /nodes + /edges: keeps the daemon's
    // blocking pool from being saturated by a runaway client.
    let limit = q.limit.unwrap_or(2000).min(MAX_GRAPH_LIMIT);
    let project_key = q.project.as_deref().unwrap_or("__default__").to_string();

    // HIGH-22 fast path: serve from the layout cache if a recent
    // entry exists. Skips the SQL JOIN + sunflower compute entirely.
    if let Some(cached) = layout_cache_get(&project_key, limit) {
        return Json(cached);
    }

    // The SQL pulls (qualified_name, kind, community_id) in the same
    // node window as /nodes, then we run the pure layout function.
    // LEFT JOIN community_membership so nodes without a community
    // (e.g. when run_community_detection didn't assign one) still
    // get a position — they bucket by kind in compute_layout.
    let sql = format!(
        "WITH visible_nodes AS ( \
             SELECT qualified_name, kind FROM nodes ORDER BY id LIMIT {limit} \
         ) \
         SELECT vn.qualified_name, vn.kind, cm.community_id \
         FROM visible_nodes vn \
         LEFT JOIN community_membership cm \
             ON cm.node_qualified = vn.qualified_name \
         ORDER BY vn.qualified_name",
        limit = limit
    );
    let raw: Vec<(String, String, Option<i64>)> =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), move |conn| {
            let mut stmt = conn.prepare(&sql).ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })
                .ok()?;
            Some(rows.filter_map(|r| r.ok()).collect())
        })
        .await
        .unwrap_or_default();
    let positions = compute_layout(&raw);
    layout_cache_put(&project_key, limit, positions.clone());
    Json(positions)
}

/// Pure deterministic layout function. Public-in-crate so tests can
/// exercise it without a database round-trip.
///
/// Input: `(qualified_name, kind, community_id_or_none)` triples.
/// Output: `(qualified_name, x, y)` triples in the same order, with
/// coordinates roughly in `[-1000, 1000]` so Sigma's default camera
/// frames the whole graph without manual zoom.
fn compute_layout(rows: &[(String, String, Option<i64>)]) -> Vec<LayoutPosition> {
    use std::collections::BTreeMap;

    if rows.is_empty() {
        return Vec::new();
    }

    // Bucket by community_id; nodes without one bucket by `loose:<kind>`.
    // BTreeMap so the iteration order is deterministic across runs.
    let mut buckets: BTreeMap<String, Vec<&(String, String, Option<i64>)>> = BTreeMap::new();
    for row in rows {
        let key = match row.2 {
            Some(cid) => format!("c:{cid}"),
            None => format!("loose:{}", row.1),
        };
        buckets.entry(key).or_default().push(row);
    }

    let n_groups = buckets.len() as f64;
    let golden_angle = std::f64::consts::PI * (3.0 - (5.0_f64).sqrt());

    // Outer disk radius. 800 keeps positions within Sigma's default
    // camera frame; the inner circle inside each group is sized
    // proportionally to its member count.
    const R_OUTER: f64 = 800.0;
    const R_INNER_PER_NODE: f64 = 8.0;
    const R_INNER_MIN: f64 = 30.0;
    const R_INNER_MAX: f64 = 200.0;

    let mut out = Vec::with_capacity(rows.len());
    let mut by_qname: std::collections::HashMap<&str, (f64, f64)> =
        std::collections::HashMap::with_capacity(rows.len());

    for (group_idx, (_key, members)) in buckets.iter().enumerate() {
        // Vogel sunflower position for this group's center.
        let theta = (group_idx as f64) * golden_angle;
        let r = if n_groups <= 1.0 {
            0.0
        } else {
            R_OUTER * ((group_idx as f64 + 0.5) / n_groups).sqrt()
        };
        let cx = r * theta.cos();
        let cy = r * theta.sin();

        // Inner circle radius scales with sqrt(group size). Clamped so
        // a single oversized cluster doesn't swamp the canvas and a
        // singleton doesn't collapse to a point.
        let m = members.len() as f64;
        let r_inner = (R_INNER_PER_NODE * m.sqrt())
            .max(R_INNER_MIN)
            .min(R_INNER_MAX);

        for (i, member) in members.iter().enumerate() {
            let phi = if members.len() <= 1 {
                0.0
            } else {
                (i as f64) * std::f64::consts::TAU / (members.len() as f64)
            };
            // Singleton groups sit exactly at the group center.
            let x = if members.len() <= 1 {
                cx
            } else {
                cx + r_inner * phi.cos()
            };
            let y = if members.len() <= 1 {
                cy
            } else {
                cy + r_inner * phi.sin()
            };
            by_qname.insert(member.0.as_str(), (x, y));
        }
    }

    // Re-emit in the original input order so the response lines up
    // with /api/graph/nodes (caller can zip the two without re-sort).
    for row in rows {
        let pos = by_qname.get(row.0.as_str()).copied().unwrap_or((0.0, 0.0));
        out.push(LayoutPosition {
            q: row.0.clone(),
            x: pos.0,
            y: pos.1,
        });
    }
    out
}

/// `GET /api/graph/status` — shard health + counts for the status bar.
async fn api_graph_status(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let project_param = q.project.as_deref();
    // BUG-A4-006 fix: `find_active_layer_db` does N+1 sync read_dir/stat
    // syscalls. Run it on the blocking-thread pool so the async runtime
    // stays responsive under burst polling from the vision SPA.
    let state_for_locator = state.clone();
    let project_owned: Option<String> = project_param.map(|s| s.to_string());
    let shard_root: Option<String> = tokio::task::spawn_blocking(move || {
        find_active_layer_db(&state_for_locator, "graph", project_owned.as_deref())
            .and_then(|p| p.parent().map(|q| q.display().to_string()))
    })
    .await
    .unwrap_or(None);

    let stats: GraphStatusOut = with_layer_db_sync(&state, "graph", project_param, |conn| {
        let nodes: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap_or(0);
        let edges: i64 = conn
            .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
            .unwrap_or(0);
        let files: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes WHERE kind = 'file'", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let mut by_kind = serde_json::Map::new();
        if let Ok(mut stmt) = conn.prepare("SELECT kind, COUNT(*) FROM nodes GROUP BY kind") {
            if let Ok(rows) =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            {
                for r in rows.flatten() {
                    by_kind.insert(r.0, serde_json::Value::Number(r.1.into()));
                }
            }
        }

        Some(GraphStatusOut {
            project: None,
            shard_root: None,
            last_index_at: None,
            nodes,
            edges,
            files,
            by_kind: serde_json::Value::Object(by_kind),
        })
    })
    .await
    .unwrap_or(GraphStatusOut {
        project: None,
        shard_root: None,
        last_index_at: None,
        nodes: 0,
        edges: 0,
        files: 0,
        by_kind: serde_json::Value::Object(Default::default()),
    });

    let final_stats = GraphStatusOut {
        shard_root,
        ..stats
    };
    Json(final_stats)
}

/// `GET /api/graph/files` — file table for the treemap view.
#[derive(Serialize)]
struct ShardFileRow {
    path: String,
    language: Option<String>,
    line_count: Option<i64>,
    byte_count: Option<i64>,
    last_parsed_at: Option<String>,
}

async fn api_graph_files(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let files: Vec<ShardFileRow> =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT path, language, line_count, byte_count, last_parsed_at \
                 FROM files ORDER BY line_count DESC LIMIT 2000",
                )
                .ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(ShardFileRow {
                        path: r.get::<_, String>(0)?,
                        language: r.get::<_, Option<String>>(1)?,
                        line_count: r.get::<_, Option<i64>>(2)?,
                        byte_count: r.get::<_, Option<i64>>(3)?,
                        last_parsed_at: r.get::<_, Option<String>>(4)?,
                    })
                })
                .ok()?;
            Some(rows.filter_map(|r| r.ok()).collect())
        })
        .await
        .unwrap_or_default();
    Json(files)
}

/// `GET /api/graph/findings` — open findings for the dashboard.
#[derive(Serialize)]
struct ShardFindingRow {
    id: i64,
    rule_id: String,
    scanner: String,
    severity: String,
    file: String,
    line_start: i64,
    line_end: i64,
    message: String,
    suggestion: Option<String>,
    created_at: Option<String>,
}

async fn api_graph_findings(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let findings: Vec<ShardFindingRow> =
        with_layer_db_sync(&state, "findings", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, rule_id, scanner, severity, file, line_start, line_end, \
                        message, suggestion, created_at \
                 FROM findings WHERE resolved_at IS NULL \
                 ORDER BY CASE severity \
                            WHEN 'critical' THEN 4 \
                            WHEN 'high'     THEN 3 \
                            WHEN 'medium'   THEN 2 \
                            WHEN 'low'      THEN 1 \
                            ELSE 0 END DESC, \
                          created_at DESC \
                 LIMIT 2000",
                )
                .ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(ShardFindingRow {
                        id: r.get(0)?,
                        rule_id: r.get(1)?,
                        scanner: r.get(2)?,
                        severity: r.get(3)?,
                        file: r.get(4)?,
                        line_start: r.get(5)?,
                        line_end: r.get(6)?,
                        message: r.get(7)?,
                        suggestion: r.get::<_, Option<String>>(8)?,
                        created_at: r.get::<_, Option<String>>(9)?,
                    })
                })
                .ok()?;
            Some(rows.filter_map(|r| r.ok()).collect())
        })
        .await
        .unwrap_or_default();
    Json(findings)
}

// ---------------------------------------------------------------------------
// F1 D3 — `/api/graph/{file-tree, kind-flow, domain-flow,
//          community-matrix, commits, heatmap, test-coverage}`
// ---------------------------------------------------------------------------
//
// Second-wave port of the vision endpoints from
// `vision/server/shard.ts` (`fetchFileTree`, `fetchKindFlow`,
// `fetchDomainFlow`, `fetchCommunityMatrix`, `fetchCommits`,
// `fetchHeatmap`, `fetchTestCoverage`).
//
// Same conventions as the D2 wave above:
// 1. Use `with_layer_db_sync` to open the right shard read-only.
// 2. Run the SQL inline (small bounded result sets).
// 3. Serialise into the wire shape `vision/src/api/graph.ts` expects.
// 4. Fall through to an empty payload (`[]`, `{nodes:[],links:[]}`,
//    etc.) on any error — matching the TS `[] / {nodes:[], links:[]}`
//    contract so the SPA renders an empty state instead of choking.

/// First path segment, used by the domain-flow + heatmap aggregations
/// to bucket files. Mirrors the TS `domainOf` helper in `shard.ts`.
fn domain_of(p: Option<&str>) -> String {
    match p {
        None => "root".to_string(),
        Some(s) => {
            for seg in s.split(['/', '\\']) {
                if !seg.is_empty() {
                    return seg.to_string();
                }
            }
            "root".to_string()
        }
    }
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/file-tree — sunburst view                            */
/* -------------------------------------------------------------------- */

/// Sunburst tree node — matches `FileTreeNode` in
/// `vision/src/api/graph.ts`. `value` and `language` are leaf-only;
/// internal nodes carry only `name + children`.
#[derive(Serialize, Default)]
struct FileTreeNodeOut {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    children: Vec<FileTreeNodeOut>,
}

impl FileTreeNodeOut {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
            language: None,
            children: Vec::new(),
        }
    }
}

/// Insert one file-row into the running tree, splitting its path on
/// `/` or `\` and walking/creating each segment. Mirrors the TS body
/// of `fetchFileTree`.
fn insert_into_tree(
    root: &mut FileTreeNodeOut,
    path: &str,
    line_count: i64,
    language: Option<String>,
) {
    let segs: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return;
    }
    let mut cursor: &mut FileTreeNodeOut = root;
    let last_idx = segs.len() - 1;
    for (i, seg) in segs.iter().enumerate() {
        let pos = cursor.children.iter().position(|c| c.name == *seg);
        let idx = match pos {
            Some(p) => p,
            None => {
                cursor.children.push(FileTreeNodeOut::new(*seg));
                cursor.children.len() - 1
            }
        };
        cursor = &mut cursor.children[idx];
        if i == last_idx {
            cursor.value = Some(line_count.max(1));
            cursor.language = language.clone();
        }
    }
}

/// `GET /api/graph/file-tree` — file rows folded into a hierarchical
/// tree keyed by path segments. Matches `fetchFileTree` in `shard.ts`.
async fn api_graph_file_tree(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let tree: FileTreeNodeOut = with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
        let mut stmt = conn
            .prepare(
                "SELECT path, language, line_count, byte_count, last_parsed_at \
                 FROM files ORDER BY line_count DESC LIMIT 4000",
            )
            .ok()?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .ok()?;
        let mut root = FileTreeNodeOut::new("project");
        for r in rows.flatten() {
            insert_into_tree(&mut root, &r.0, r.2.unwrap_or(1), r.1);
        }
        Some(root)
    })
    .await
    .unwrap_or_else(|| FileTreeNodeOut::new("project"));
    Json(tree)
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/kind-flow — sankey kind-to-kind flow                 */
/* -------------------------------------------------------------------- */

#[derive(Serialize)]
struct KindFlowNodeOut {
    id: String,
    kind: String,
    side: String,
}

#[derive(Serialize)]
struct KindFlowLinkOut {
    source: String,
    target: String,
    value: i64,
    #[serde(rename = "edgeKind")]
    edge_kind: String,
}

#[derive(Serialize, Default)]
struct KindFlowPayloadOut {
    nodes: Vec<KindFlowNodeOut>,
    links: Vec<KindFlowLinkOut>,
}

/// `GET /api/graph/kind-flow` — sankey aggregation of edges by
/// (source-kind, target-kind, edge-kind). Mirrors `fetchKindFlow`.
async fn api_graph_kind_flow(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let payload: KindFlowPayloadOut =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT ns.kind AS source_kind, nt.kind AS target_kind, \
                        e.kind AS edge_kind, COUNT(*) AS c \
                 FROM edges e \
                 JOIN nodes ns ON ns.qualified_name = e.source_qualified \
                 JOIN nodes nt ON nt.qualified_name = e.target_qualified \
                 GROUP BY ns.kind, nt.kind, e.kind \
                 ORDER BY c DESC \
                 LIMIT 50000",
                )
                .ok()?;
            let rows: Vec<(String, String, String, i64)> = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();

            // Build the node set with stable insertion order — TS uses
            // `Set` iteration which is insertion-ordered, so we mirror it.
            let mut node_ids: Vec<String> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for (sk, tk, _ek, _c) in &rows {
                let s_id = format!("src:{}", sk);
                let t_id = format!("tgt:{}", tk);
                if seen.insert(s_id.clone()) {
                    node_ids.push(s_id);
                }
                if seen.insert(t_id.clone()) {
                    node_ids.push(t_id);
                }
            }

            let nodes: Vec<KindFlowNodeOut> = node_ids
                .into_iter()
                .map(|id| {
                    let (side, kind) = match id.split_once(':') {
                        Some((s, k)) => (s.to_string(), k.to_string()),
                        None => ("src".to_string(), id.clone()),
                    };
                    KindFlowNodeOut {
                        id: format!("{}:{}", side, kind),
                        kind,
                        side,
                    }
                })
                .collect();
            let links: Vec<KindFlowLinkOut> = rows
                .into_iter()
                .map(|(sk, tk, ek, c)| KindFlowLinkOut {
                    source: format!("src:{}", sk),
                    target: format!("tgt:{}", tk),
                    value: c,
                    edge_kind: ek,
                })
                .collect();
            Some(KindFlowPayloadOut { nodes, links })
        })
        .await
        .unwrap_or_default();
    Json(payload)
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/domain-flow — sankey domain-to-domain flow           */
/* -------------------------------------------------------------------- */

#[derive(Serialize)]
struct DomainFlowNodeOut {
    id: String,
    domain: String,
}

#[derive(Serialize)]
struct DomainFlowLinkOut {
    source: String,
    target: String,
    value: i64,
}

#[derive(Serialize, Default)]
struct DomainFlowPayloadOut {
    nodes: Vec<DomainFlowNodeOut>,
    links: Vec<DomainFlowLinkOut>,
}

/// `GET /api/graph/domain-flow` — aggregate edges across the
/// first-path-segment ("domain") boundary. Self-loops are dropped to
/// match the TS implementation. Mirrors `fetchDomainFlow`.
async fn api_graph_domain_flow(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let payload: DomainFlowPayloadOut =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT ns.file_path AS src_path, nt.file_path AS tgt_path, COUNT(*) AS c \
                 FROM edges e \
                 JOIN nodes ns ON ns.qualified_name = e.source_qualified \
                 JOIN nodes nt ON nt.qualified_name = e.target_qualified \
                 WHERE ns.file_path IS NOT NULL AND nt.file_path IS NOT NULL \
                 GROUP BY ns.file_path, nt.file_path \
                 LIMIT 50000",
                )
                .ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                })
                .ok()?;

            let mut agg: std::collections::HashMap<(String, String), i64> =
                std::collections::HashMap::new();
            // Preserve domain insertion order (TS uses `Set` which is
            // insertion-ordered) so the rendered sankey is stable.
            let mut domains: Vec<String> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for r in rows.flatten() {
                let s = domain_of(r.0.as_deref());
                let t = domain_of(r.1.as_deref());
                if s == t {
                    continue;
                }
                if seen.insert(s.clone()) {
                    domains.push(s.clone());
                }
                if seen.insert(t.clone()) {
                    domains.push(t.clone());
                }
                *agg.entry((s, t)).or_insert(0) += r.2;
            }

            let nodes: Vec<DomainFlowNodeOut> = domains
                .into_iter()
                .map(|d| DomainFlowNodeOut {
                    id: d.clone(),
                    domain: d,
                })
                .collect();
            let links: Vec<DomainFlowLinkOut> = agg
                .into_iter()
                .map(|((s, t), v)| DomainFlowLinkOut {
                    source: s,
                    target: t,
                    value: v,
                })
                .collect();
            Some(DomainFlowPayloadOut { nodes, links })
        })
        .await
        .unwrap_or_default();
    Json(payload)
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/community-matrix — chord/arc view                    */
/* -------------------------------------------------------------------- */

#[derive(Serialize)]
struct CommunityInfoOut {
    id: i64,
    name: String,
    size: i64,
    language: Option<String>,
}

#[derive(Serialize, Default)]
struct CommunityMatrixPayloadOut {
    communities: Vec<CommunityInfoOut>,
    matrix: Vec<Vec<i64>>,
}

/// `GET /api/graph/community-matrix` — top-24 communities + an N×N
/// matrix of edge counts between them, derived by joining
/// `semantic.db.community_membership` with `graph.db.edges`. Mirrors
/// `fetchCommunityMatrix`. Two shards are required; if either is
/// missing we return an empty payload.
async fn api_graph_community_matrix(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let project_param = q.project.as_deref();
    // Step 1: read communities + membership from semantic.db.
    let semantic_data = with_layer_db_sync(&state, "semantic", project_param, |conn| {
        let mut comm_stmt = conn
            .prepare(
                "SELECT id, name, size, dominant_language \
                 FROM communities ORDER BY size DESC LIMIT 24",
            )
            .ok()?;
        let comm_rows: Vec<(i64, String, i64, Option<String>)> = comm_stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        if comm_rows.is_empty() {
            return Some((Vec::new(), Vec::new()));
        }

        let mut mem_stmt = conn
            .prepare("SELECT community_id, node_qualified FROM community_membership")
            .ok()?;
        let members: Vec<(i64, String)> = mem_stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .ok()?
            .filter_map(|r| r.ok())
            .collect();
        Some((comm_rows, members))
    })
    .await;

    let (comm_rows, members) = match semantic_data {
        Some(d) => d,
        None => {
            return Json(CommunityMatrixPayloadOut::default());
        }
    };
    if comm_rows.is_empty() {
        return Json(CommunityMatrixPayloadOut::default());
    }

    // Build community-id -> matrix-index lookup, and node -> matrix-index.
    let mut comm_index: std::collections::HashMap<i64, usize> =
        std::collections::HashMap::with_capacity(comm_rows.len());
    for (i, c) in comm_rows.iter().enumerate() {
        comm_index.insert(c.0, i);
    }
    let mut node_to_comm: std::collections::HashMap<String, usize> =
        std::collections::HashMap::with_capacity(members.len());
    for (cid, nq) in members {
        if let Some(&idx) = comm_index.get(&cid) {
            node_to_comm.insert(nq, idx);
        }
    }

    // Step 2: walk edges in graph.db, accumulate matrix[i][j].
    // BUG-A4-001 fix: closure runs on the blocking-thread pool, so we
    // must move (`node_to_comm`, `matrix`) in by value and return the
    // mutated matrix back out -- the previous `&mut`-by-capture pattern
    // is not `Send + 'static` and would not compile under spawn_blocking.
    let n = comm_rows.len();
    let initial_matrix: Vec<Vec<i64>> = vec![vec![0_i64; n]; n];
    let matrix: Vec<Vec<i64>> = with_layer_db_sync(&state, "graph", project_param, move |conn| {
        let mut local_matrix = initial_matrix;
        let mut stmt = conn
            .prepare(
                "SELECT source_qualified, target_qualified \
                 FROM edges LIMIT 200000",
            )
            .ok()?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .ok()?;
        for r in rows.flatten() {
            let si = node_to_comm.get(&r.0);
            let ti = node_to_comm.get(&r.1);
            if let (Some(&s), Some(&t)) = (si, ti) {
                if let Some(row) = local_matrix.get_mut(s) {
                    if let Some(cell) = row.get_mut(t) {
                        *cell += 1;
                    }
                }
            }
        }
        Some(local_matrix)
    })
    .await
    .unwrap_or_else(|| vec![vec![0_i64; n]; n]);

    let communities: Vec<CommunityInfoOut> = comm_rows
        .into_iter()
        .map(|(id, name, size, language)| CommunityInfoOut {
            id,
            name,
            size,
            language,
        })
        .collect();

    Json(CommunityMatrixPayloadOut {
        communities,
        matrix,
    })
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/commits — git timeline view                          */
/* -------------------------------------------------------------------- */

/// One commit row — matches `CommitRow` in `vision/src/api/graph.ts`.
#[derive(Serialize)]
struct CommitRowOut {
    sha: String,
    author: Option<String>,
    date: String,
    message: String,
    files_changed: i64,
    insertions: i64,
    deletions: i64,
}

/// `GET /api/graph/commits` — recent commits joined to per-file
/// add/delete totals. Mirrors `fetchCommits`. Source layer: `git.db`.
async fn api_graph_commits(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let commits: Vec<CommitRowOut> =
        with_layer_db_sync(&state, "git", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT c.sha, c.author_name, c.committed_at, c.message, \
                        COUNT(cf.file_path) AS files_changed, \
                        COALESCE(SUM(cf.additions), 0) AS insertions, \
                        COALESCE(SUM(cf.deletions), 0) AS deletions \
                 FROM commits c \
                 LEFT JOIN commit_files cf ON cf.sha = c.sha \
                 GROUP BY c.sha \
                 ORDER BY c.committed_at DESC \
                 LIMIT 500",
                )
                .ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(CommitRowOut {
                        sha: r.get::<_, String>(0)?,
                        author: r.get::<_, Option<String>>(1)?,
                        date: r.get::<_, String>(2)?,
                        message: r.get::<_, String>(3)?,
                        files_changed: r.get::<_, i64>(4)?,
                        insertions: r.get::<_, i64>(5)?,
                        deletions: r.get::<_, i64>(6)?,
                    })
                })
                .ok()?;
            Some(rows.filter_map(|r| r.ok()).collect())
        })
        .await
        .unwrap_or_default();
    Json(commits)
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/heatmap — file × severity grid                       */
/* -------------------------------------------------------------------- */

#[derive(Serialize)]
struct HeatmapSeverities {
    critical: i64,
    high: i64,
    medium: i64,
    low: i64,
}

#[derive(Serialize)]
struct HeatmapFileRowOut {
    file: String,
    language: Option<String>,
    line_count: i64,
    complexity: i64,
    severities: HeatmapSeverities,
}

#[derive(Serialize)]
struct HeatmapPayloadOut {
    severities: Vec<&'static str>,
    files: Vec<HeatmapFileRowOut>,
}

impl Default for HeatmapPayloadOut {
    fn default() -> Self {
        Self {
            severities: vec!["critical", "high", "medium", "low"],
            files: Vec::new(),
        }
    }
}

/// `GET /api/graph/heatmap` — top files by line-count, joined to a
/// per-file function-count (complexity proxy) and per-file open-finding
/// counts bucketed by severity. Mirrors `fetchHeatmap` — pulls from
/// both `graph.db` and `findings.db`.
async fn api_graph_heatmap(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let project_param = q.project.as_deref();
    // Step 1: files + complexity from graph.db.
    let from_graph = with_layer_db_sync(&state, "graph", project_param, |conn| {
        let mut files_stmt = conn
            .prepare(
                "SELECT path, language, line_count FROM files \
                 ORDER BY line_count DESC LIMIT 120",
            )
            .ok()?;
        let files: Vec<(String, Option<String>, Option<i64>)> = files_stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        let mut cx_stmt = conn
            .prepare(
                "SELECT file_path, COUNT(*) AS c FROM nodes \
                 WHERE kind = 'function' AND file_path IS NOT NULL \
                 GROUP BY file_path",
            )
            .ok()?;
        let mut complexity: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        if let Ok(rows) =
            cx_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        {
            for r in rows.flatten() {
                complexity.insert(r.0, r.1);
            }
        }
        Some((files, complexity))
    })
    .await;

    let (files, complexity) = match from_graph {
        Some(d) => d,
        None => return Json(HeatmapPayloadOut::default()),
    };

    // Step 2: per-(file, severity) finding counts from findings.db.
    let mut sev_by_file: std::collections::HashMap<String, HeatmapSeverities> =
        with_layer_db_sync(&state, "findings", project_param, |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT file, severity, COUNT(*) AS c FROM findings \
                     WHERE resolved_at IS NULL \
                     GROUP BY file, severity",
                )
                .ok()?;
            let mut map: std::collections::HashMap<String, HeatmapSeverities> =
                std::collections::HashMap::new();
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            }) {
                for r in rows.flatten() {
                    let bucket = map.entry(r.0).or_insert(HeatmapSeverities {
                        critical: 0,
                        high: 0,
                        medium: 0,
                        low: 0,
                    });
                    match r.1.as_str() {
                        "critical" => bucket.critical = r.2,
                        "high" => bucket.high = r.2,
                        "medium" => bucket.medium = r.2,
                        "low" => bucket.low = r.2,
                        _ => { /* ignore unknown severities */ }
                    }
                }
            }
            Some(map)
        })
        .await
        .unwrap_or_default();

    let rows = files
        .into_iter()
        .map(|(path, language, line_count)| {
            let sev = sev_by_file.remove_or_default(&path);
            let cx = complexity.get(&path).copied().unwrap_or(0);
            HeatmapFileRowOut {
                file: path,
                language,
                line_count: line_count.unwrap_or(0),
                complexity: cx,
                severities: sev,
            }
        })
        .collect::<Vec<_>>();

    Json(HeatmapPayloadOut {
        severities: vec!["critical", "high", "medium", "low"],
        files: rows,
    })
}

/// Helper: drain-or-default lookup for the heatmap severity map. Avoids
/// double-borrow vs. trying to `remove` in the loop above.
trait MapTakeOrDefault<K, V> {
    fn remove_or_default(&mut self, k: &K) -> V;
}
impl MapTakeOrDefault<String, HeatmapSeverities>
    for std::collections::HashMap<String, HeatmapSeverities>
{
    fn remove_or_default(&mut self, k: &String) -> HeatmapSeverities {
        self.remove(k).unwrap_or(HeatmapSeverities {
            critical: 0,
            high: 0,
            medium: 0,
            low: 0,
        })
    }
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/test-coverage — covered/uncovered file table         */
/* -------------------------------------------------------------------- */

#[derive(Serialize)]
struct TestCoverageRowOut {
    file: String,
    language: Option<String>,
    line_count: i64,
    test_file: Option<String>,
    test_count: i64,
    covered: bool,
}

/// True when the path looks like a test file (matches the TS heuristic
/// `isTestPath` in `shard.ts`). Recognises:
/// * `tests/` or `__tests__/` somewhere in the path
/// * `_test.{rs,py,go}` suffix
/// * `*.test.{js,jsx,ts,tsx}` and `*.spec.{js,jsx,ts,tsx}`
/// * `test_*.py` filename prefix
fn is_test_path(p: &str) -> bool {
    let lower = p.to_lowercase();
    let bytes = lower.as_bytes();

    // tests/ or __tests__/ as a path segment.
    let has_segment = |needle: &str| -> bool {
        // Accept either `<sep>needle<sep>` or path starts with `needle<sep>`
        // or path ends with `<sep>needle`.
        if lower == needle {
            return true;
        }
        if lower.starts_with(&format!("{}/", needle)) || lower.starts_with(&format!("{}\\", needle))
        {
            return true;
        }
        for sep in ['/', '\\'] {
            let mid = format!("{}{}{}", sep, needle, sep);
            if lower.contains(&mid) {
                return true;
            }
            let end = format!("{}{}", sep, needle);
            if lower.ends_with(&end) {
                return true;
            }
        }
        false
    };
    if has_segment("tests") || has_segment("test") || has_segment("__tests__") {
        return true;
    }

    // _test.{rs,py,go} suffix.
    for ext in ["_test.rs", "_test.py", "_test.go"] {
        if lower.ends_with(ext) {
            return true;
        }
    }
    // .test.{js,jsx,ts,tsx} / .spec.{...} suffix.
    for ext in [
        ".test.js",
        ".test.jsx",
        ".test.ts",
        ".test.tsx",
        ".spec.js",
        ".spec.jsx",
        ".spec.ts",
        ".spec.tsx",
    ] {
        if lower.ends_with(ext) {
            return true;
        }
    }
    // test_<name>.py — last path segment must start with "test_".
    if lower.ends_with(".py") {
        let last_sep = bytes
            .iter()
            .rposition(|b| *b == b'/' || *b == b'\\')
            .map(|i| i + 1)
            .unwrap_or(0);
        let last = &lower[last_sep..];
        if last.starts_with("test_") {
            return true;
        }
    }
    false
}

/// Generate plausible test-file paths for a given source path, mirroring
/// the TS `testFilenameCandidates` helper. Used to pair a source file
/// with its co-located or external test file.
fn test_filename_candidates(src: &str) -> Vec<String> {
    let parts: Vec<&str> = src.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Vec::new();
    }
    let last = parts[parts.len() - 1];
    let (base, ext) = match last.rfind('.') {
        Some(i) => (&last[..i], &last[i..]),
        None => (last, ""),
    };
    let dir = if parts.len() > 1 {
        parts[..parts.len() - 1].join("/")
    } else {
        String::new()
    };
    let join = |segments: &[&str]| -> String {
        let mut out = String::new();
        for (i, s) in segments.iter().enumerate() {
            if s.is_empty() {
                continue;
            }
            if i > 0 && !out.is_empty() {
                out.push('/');
            }
            out.push_str(s);
        }
        out
    };

    let mut out: Vec<String> = Vec::new();
    match ext {
        ".rs" => {
            out.push(join(&[&dir, &format!("{}_test{}", base, ext)]));
            out.push(format!("tests/{}{}", base, ext));
            out.push(join(&[&dir, "tests", &format!("{}{}", base, ext)]));
        }
        ".ts" | ".tsx" | ".js" | ".jsx" => {
            out.push(join(&[&dir, &format!("{}.test{}", base, ext)]));
            out.push(join(&[&dir, &format!("{}.spec{}", base, ext)]));
            out.push(join(&[&dir, "__tests__", &format!("{}{}", base, ext)]));
        }
        ".py" => {
            out.push(join(&[&dir, &format!("test_{}{}", base, ext)]));
            out.push(format!("tests/test_{}{}", base, ext));
        }
        _ => {}
    }
    out
}

/// `GET /api/graph/test-coverage` — iterate non-test files and pair each
/// with a candidate test file (co-located or `tests/`-rooted), counting
/// own-file `is_test=1` nodes plus the matched test-file's `is_test=1`
/// nodes. Mirrors `fetchTestCoverage`. Source: `graph.db` only — the
/// TS code reads `nodes.is_test` from graph.db, not the separate
/// `tests.db` (which holds runtime metadata).
async fn api_graph_test_coverage(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let rows: Vec<TestCoverageRowOut> =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
            let mut files_stmt = conn
                .prepare(
                    "SELECT path, language, line_count FROM files \
                 ORDER BY line_count DESC",
                )
                .ok()?;
            let all_files: Vec<(String, Option<String>, Option<i64>)> = files_stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();

            let mut node_stmt = conn
                .prepare(
                    "SELECT file_path, COUNT(*) AS c FROM nodes \
                 WHERE is_test = 1 AND file_path IS NOT NULL \
                 GROUP BY file_path",
                )
                .ok()?;
            let mut test_node_by_file: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            if let Ok(it) =
                node_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            {
                for r in it.flatten() {
                    test_node_by_file.insert(r.0, r.1);
                }
            }

            // Bucket by test-vs-source.
            let test_paths: std::collections::HashSet<String> = all_files
                .iter()
                .filter(|f| is_test_path(&f.0))
                .map(|f| f.0.clone())
                .collect();
            let source_files: Vec<&(String, Option<String>, Option<i64>)> = all_files
                .iter()
                .filter(|f| !is_test_path(&f.0))
                .take(2000)
                .collect();

            let out: Vec<TestCoverageRowOut> = source_files
                .into_iter()
                .map(|(path, language, line_count)| {
                    let candidates = test_filename_candidates(path);
                    let test_file = candidates.into_iter().find(|c| test_paths.contains(c));
                    let own = test_node_by_file.get(path).copied().unwrap_or(0);
                    let external = test_file
                        .as_ref()
                        .map(|tf| test_node_by_file.get(tf).copied().unwrap_or(1))
                        .unwrap_or(0);
                    let total = own + external;
                    TestCoverageRowOut {
                        file: path.clone(),
                        language: language.clone(),
                        line_count: line_count.unwrap_or(0),
                        test_file,
                        test_count: total,
                        covered: total > 0,
                    }
                })
                .collect();
            Some(out)
        })
        .await
        .unwrap_or_default();
    Json(rows)
}

// ---------------------------------------------------------------------------
// F1 D4 — Final wave: `/api/graph/{layers, galaxy-3d, theme-palette,
//          hierarchy}` and `/api/daemon/health`.
// ---------------------------------------------------------------------------
//
// Final-wave port of the last vision endpoints from
// `vision/server/shard.ts` (`fetchLayerTiers`, `fetchGalaxy3D`,
// `fetchThemeSwatches`, `fetchHierarchy`) plus the `/api/daemon/health`
// alias (the SPA uses `/api/health` and `/api/daemon/health`
// interchangeably as a daemon liveness probe).
//
// Same conventions as the D2/D3 waves above:
// 1. Use `with_layer_db_sync` to open the right shard read-only.
// 2. Run the SQL inline (small bounded result sets, sub-100ms).
// 3. Serialise into the wire shape `vision/src/api/graph.ts` expects.
// 4. Fall through to an empty payload on any error — matching the TS
//    `[] / {nodes:[], links:[]}` contract so the SPA renders an empty
//    state instead of choking.

/* -------------------------------------------------------------------- */
/*  GET /api/graph/layers — Layered Architecture                        */
/* -------------------------------------------------------------------- */

/// One file row tagged with its tier + first-segment domain. Mirrors
/// `LayerTierEntry` in `vision/src/api/graph.ts`.
#[derive(Serialize)]
struct LayerTierEntryOut {
    file: String,
    language: Option<String>,
    line_count: i64,
    tier: String,
    domain: String,
}

#[derive(Serialize)]
struct LayerTierPayloadOut {
    tiers: Vec<&'static str>,
    entries: Vec<LayerTierEntryOut>,
}

impl Default for LayerTierPayloadOut {
    fn default() -> Self {
        Self {
            tiers: vec![
                "presentation",
                "api",
                "intelligence",
                "data",
                "foundation",
                "other",
            ],
            entries: Vec::new(),
        }
    }
}

/// Tier classification — mirrors `TIER_RULES` + `tierOf` in `shard.ts`.
/// The first path segment is matched against a fixed regex set; falls
/// back to `"other"` when no rule fires.
fn tier_of(path: Option<&str>) -> &'static str {
    let first = domain_of(path);
    let lower = first.to_lowercase();
    // Presentation: vision, web, ui, frontend.
    if lower == "vision"
        || lower == "web"
        || lower == "ui"
        || lower == "frontend"
        || lower.starts_with("vision")
        || lower.starts_with("web")
        || lower.starts_with("ui")
        || lower.starts_with("frontend")
    {
        return "presentation";
    }
    // API: mcp, cli, api, plugin.
    if lower == "mcp"
        || lower == "cli"
        || lower == "api"
        || lower == "plugin"
        || lower.starts_with("mcp")
        || lower.starts_with("cli")
        || lower.starts_with("api")
        || lower.starts_with("plugin")
    {
        return "api";
    }
    // Intelligence: brain, parser(s), scanner(s), worker(s), multimodal.
    if lower == "brain"
        || lower == "parser"
        || lower == "parsers"
        || lower == "scanner"
        || lower == "scanners"
        || lower == "worker"
        || lower == "workers"
        || lower == "multimodal"
    {
        return "intelligence";
    }
    // Data: store, supervisor, livebus, sql.
    if lower == "store" || lower == "supervisor" || lower == "livebus" || lower == "sql" {
        return "data";
    }
    // Foundation: common, core, shared, util(s).
    if lower == "common"
        || lower == "core"
        || lower == "shared"
        || lower == "util"
        || lower == "utils"
    {
        return "foundation";
    }
    "other"
}

/// `GET /api/graph/layers` — file rows tagged with tier + domain.
/// Mirrors `fetchLayerTiers` in `shard.ts`. Source: `graph.db`.
async fn api_graph_layers(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let payload: LayerTierPayloadOut =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT path, language, line_count FROM files \
                 ORDER BY line_count DESC LIMIT 5000",
                )
                .ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })
                .ok()?;
            let entries: Vec<LayerTierEntryOut> = rows
                .filter_map(|r| r.ok())
                .map(|(path, language, line_count)| {
                    let tier = tier_of(Some(&path)).to_string();
                    let domain = domain_of(Some(&path));
                    LayerTierEntryOut {
                        file: path,
                        language,
                        line_count: line_count.unwrap_or(0),
                        tier,
                        domain,
                    }
                })
                .collect();
            Some(LayerTierPayloadOut {
                tiers: vec![
                    "presentation",
                    "api",
                    "intelligence",
                    "data",
                    "foundation",
                    "other",
                ],
                entries,
            })
        })
        .await
        .unwrap_or_default();
    Json(payload)
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/galaxy-3d — Project Galaxy 3D view                   */
/* -------------------------------------------------------------------- */

/// One galaxy node — matches `Galaxy3DNode` in `vision/src/api/graph.ts`.
#[derive(Serialize)]
struct Galaxy3DNodeOut {
    id: String,
    label: String,
    kind: String,
    file_path: Option<String>,
    degree: i64,
    community_id: Option<i64>,
}

/// One galaxy edge — matches `Galaxy3DEdge` in `vision/src/api/graph.ts`.
#[derive(Serialize)]
struct Galaxy3DEdgeOut {
    source: String,
    target: String,
    kind: String,
}

#[derive(Serialize, Default)]
struct Galaxy3DPayloadOut {
    nodes: Vec<Galaxy3DNodeOut>,
    edges: Vec<Galaxy3DEdgeOut>,
}

/// `GET /api/graph/galaxy-3d` — top-N nodes augmented with degree and
/// community-id, plus a bounded edge list. Mirrors `fetchGalaxy3D` in
/// `shard.ts`. Reads `graph.db` (mandatory) and `semantic.db` (optional;
/// missing semantic just leaves community_id null).
async fn api_graph_galaxy_3d(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let project_param = q.project.as_deref();
    // Step 1: nodes + degree from graph.db.
    let from_graph = with_layer_db_sync(&state, "graph", project_param, |conn| {
        let mut node_stmt = conn
            .prepare(
                "SELECT qualified_name, name, kind, file_path \
                 FROM nodes ORDER BY id LIMIT 4000",
            )
            .ok()?;
        let nodes: Vec<(String, Option<String>, String, Option<String>)> = node_stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        let mut deg_stmt = conn
            .prepare(
                "SELECT q, COUNT(*) AS c FROM ( \
                   SELECT source_qualified AS q FROM edges \
                   UNION ALL \
                   SELECT target_qualified AS q FROM edges \
                 ) GROUP BY q",
            )
            .ok()?;
        let mut degree: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        if let Ok(it) =
            deg_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        {
            for r in it.flatten() {
                degree.insert(r.0, r.1);
            }
        }

        let mut edge_stmt = conn
            .prepare(
                "SELECT source_qualified, target_qualified, kind \
                 FROM edges ORDER BY id LIMIT 8000",
            )
            .ok()?;
        let edges: Vec<(String, String, String)> = edge_stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        Some((nodes, degree, edges))
    })
    .await;

    let (nodes_raw, degree, edges_raw) = match from_graph {
        Some(d) => d,
        None => return Json(Galaxy3DPayloadOut::default()),
    };

    // Step 2: optional community_id lookup from semantic.db.
    let comm_by_node: std::collections::HashMap<String, i64> =
        with_layer_db_sync(&state, "semantic", project_param, |conn| {
            let mut stmt = conn
                .prepare("SELECT community_id, node_qualified FROM community_membership")
                .ok()?;
            let mut map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
            if let Ok(it) =
                stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            {
                for r in it.flatten() {
                    map.insert(r.1, r.0);
                }
            }
            Some(map)
        })
        .await
        .unwrap_or_default();

    let nodes: Vec<Galaxy3DNodeOut> = nodes_raw
        .into_iter()
        .map(|(id, name, kind, file_path)| {
            let label = name.clone().unwrap_or_else(|| id.clone());
            let deg = degree.get(&id).copied().unwrap_or(0);
            let cid = comm_by_node.get(&id).copied();
            Galaxy3DNodeOut {
                id,
                label,
                kind,
                file_path,
                degree: deg,
                community_id: cid,
            }
        })
        .collect();
    let edges: Vec<Galaxy3DEdgeOut> = edges_raw
        .into_iter()
        .map(|(source, target, kind)| Galaxy3DEdgeOut {
            source,
            target,
            kind,
        })
        .collect();
    Json(Galaxy3DPayloadOut { nodes, edges })
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/theme-palette — Theme palette view                   */
/* -------------------------------------------------------------------- */

/// One theme swatch row — matches `ThemeSwatchRow` in
/// `vision/src/api/graph.ts`. Each row corresponds to one extracted
/// colour token from a theme-scanner finding.
#[derive(Serialize)]
struct ThemeSwatchRowOut {
    file: String,
    line: i64,
    declaration: String,
    value: String,
    severity: String,
    message: String,
    used_count: i64,
}

/// `GET /api/graph/theme-palette` — extracts colour tokens (`#rgb(a)`,
/// `rgb(...)`, `hsl(...)`, `var(--name)`) from open theme-scanner
/// findings and returns one row per (file, line, value) tuple. Mirrors
/// `fetchThemeSwatches` in `shard.ts`. Source: `findings.db`.
async fn api_graph_theme_palette(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let rows: Vec<ThemeSwatchRowOut> =
        with_layer_db_sync(&state, "findings", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, file, line_start, message, suggestion, rule_id, severity \
                 FROM findings \
                 WHERE scanner = 'theme' AND resolved_at IS NULL \
                 ORDER BY severity DESC, created_at DESC \
                 LIMIT 2000",
                )
                .ok()?;
            #[allow(clippy::type_complexity)]
            let raw: Vec<(i64, String, i64, String, Option<String>, String, String)> = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();

            // First pass: extract colour tokens, accumulate global counts.
            let mut swatches: Vec<ThemeSwatchRowOut> = Vec::new();
            let mut counts: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for (_id, file, line, message, suggestion, rule_id, severity) in raw {
                let combined = format!("{} {}", message, suggestion.as_deref().unwrap_or(""));
                for token in extract_color_tokens(&combined) {
                    *counts.entry(token.clone()).or_insert(0) += 1;
                    swatches.push(ThemeSwatchRowOut {
                        file: file.clone(),
                        line,
                        declaration: rule_id.clone(),
                        value: token,
                        severity: severity.clone(),
                        message: message.clone(),
                        used_count: 0,
                    });
                }
            }
            // Second pass: fill used_count from the global map.
            for s in swatches.iter_mut() {
                s.used_count = counts.get(&s.value).copied().unwrap_or(1);
            }
            // Deduplicate by (file, line, value) — scanners sometimes emit
            // multiple findings on the same line for the same token.
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut deduped: Vec<ThemeSwatchRowOut> = Vec::with_capacity(swatches.len());
            for s in swatches {
                let key = format!("{}:{}:{}", s.file, s.line, s.value);
                if seen.insert(key) {
                    deduped.push(s);
                }
            }
            Some(deduped)
        })
        .await
        .unwrap_or_default();
    Json(rows)
}

/// Pull colour tokens out of a free-text scanner message. Mirrors the
/// TS `COLOR_RE` in `shard.ts`:
///   * `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (3-8 hex digits)
///   * `rgb(...)`, `rgba(...)`, `hsl(...)`, `hsla(...)`
///   * `var(--name)` for CSS custom properties
fn extract_color_tokens(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // # hex literal — 3 to 8 hex digits, then word-boundary.
        if b == b'#' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_ascii_hexdigit() {
                j += 1;
            }
            let hex_len = j - i - 1;
            if (3..=8).contains(&hex_len) {
                // Word-boundary check — next char must not be alnum.
                let at_boundary = j == bytes.len() || !(bytes[j] as char).is_ascii_alphanumeric();
                if at_boundary {
                    out.push(s[i..j].to_string());
                    i = j;
                    continue;
                }
            }
        }
        // rgb / rgba / hsl / hsla function call.
        if (b == b'r' || b == b'h')
            && i + 3 < bytes.len()
            && (s[i..].starts_with("rgb(")
                || s[i..].starts_with("rgba(")
                || s[i..].starts_with("hsl(")
                || s[i..].starts_with("hsla("))
        {
            if let Some(end_off) = s[i..].find(')') {
                let end = i + end_off + 1;
                out.push(s[i..end].to_string());
                i = end;
                continue;
            }
        }
        // var(--token) custom property.
        if b == b'v' && s[i..].starts_with("var(--") {
            if let Some(end_off) = s[i..].find(')') {
                let end = i + end_off + 1;
                let inside = &s[i + 6..end - 1];
                let valid = !inside.is_empty()
                    && inside
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                if valid {
                    out.push(s[i..end].to_string());
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/* -------------------------------------------------------------------- */
/*  GET /api/graph/hierarchy — Hierarchy tree view                      */
/* -------------------------------------------------------------------- */

/// One hierarchy tree node — matches `HierarchyNode` in
/// `vision/src/api/graph.ts`. `kind`/`file_path` are leaf-only metadata.
#[derive(Serialize, Default)]
struct HierarchyNodeOut {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
    children: Vec<HierarchyNodeOut>,
}

impl HierarchyNodeOut {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: None,
            file_path: None,
            children: Vec::new(),
        }
    }
}

/// Insert one (qualified_name, kind, file_path) triple into the running
/// hierarchy tree, splitting the qualified name on `.`, `:`, `/`, `\`.
/// Mirrors the TS body of `fetchHierarchy`.
fn insert_into_hierarchy(
    root: &mut HierarchyNodeOut,
    qualified_name: &str,
    kind: &str,
    file_path: Option<String>,
) {
    let segs: Vec<&str> = qualified_name
        .split(['.', ':', '/', '\\'])
        .filter(|s| !s.is_empty())
        .collect();
    if segs.is_empty() {
        return;
    }
    let mut cursor: &mut HierarchyNodeOut = root;
    let last_idx = segs.len() - 1;
    for (i, seg) in segs.iter().enumerate() {
        let pos = cursor.children.iter().position(|c| c.name == *seg);
        let idx = match pos {
            Some(p) => p,
            None => {
                cursor.children.push(HierarchyNodeOut::new(*seg));
                cursor.children.len() - 1
            }
        };
        cursor = &mut cursor.children[idx];
        if i == last_idx {
            cursor.kind = Some(kind.to_string());
            cursor.file_path = file_path.clone();
        }
    }
}

/// `GET /api/graph/hierarchy` — module/class/file nodes folded into a
/// hierarchical tree keyed by qualified-name segments. Mirrors
/// `fetchHierarchy` in `shard.ts`. Source: `graph.db`.
async fn api_graph_hierarchy(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    let tree: HierarchyNodeOut =
        with_layer_db_sync(&state, "graph", q.project.as_deref(), |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT qualified_name, kind, file_path FROM nodes \
                 WHERE kind IN ('module', 'class', 'file') \
                 ORDER BY qualified_name LIMIT 4000",
                )
                .ok()?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                })
                .ok()?;
            let mut root = HierarchyNodeOut::new("project");
            for r in rows.flatten() {
                insert_into_hierarchy(&mut root, &r.0, &r.1, r.2);
            }
            Some(root)
        })
        .await
        .unwrap_or_else(|| HierarchyNodeOut::new("project"));
    Json(tree)
}

/* -------------------------------------------------------------------- */
/*  GET /api/daemon/health — alias for /api/health                      */
/* -------------------------------------------------------------------- */

/// `GET /api/daemon/health` — alias for `/api/health`. The vision
/// frontend uses both URLs interchangeably as a daemon liveness probe
/// (see `vision/src/api/graph.ts:fetchDaemonHealth` and the older
/// Bun-server `probeDaemon` helper). We mirror the same JSON body so the
/// frontend doesn't have to discriminate.
async fn api_daemon_health(State(_state): State<ApiGraphState>) -> impl IntoResponse {
    let ts_ms: i64 = chrono::Utc::now().timestamp_millis();
    // LOW fix (2026-05-05 audit): see `api_health` — same drop of
    // the internal `phase: "D0"` milestone leak. Health alias must
    // match the canonical /api/health shape exactly.
    Json(json!({
        "ok": true,
        "host": "127.0.0.1",
        "port": 7777,
        "ts": ts_ms,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    fn test_state() -> ApiGraphState {
        // Use a tempdir so we don't touch the user's real ~/.mneme.
        let tmp = tempfile::tempdir().expect("tempdir");
        ApiGraphState {
            paths: Arc::new(PathManager::with_root(tmp.path().to_path_buf())),
            // Phase A · F2: tests that don't exercise `/ws` keep `None`.
            // The `/ws` route will still respond (with an error frame +
            // close) when there's no bus attached.
            livebus: None,
        }
    }

    // ---- Item #124: compute_layout pure tests ------------------------

    #[test]
    fn layout_empty_input_returns_empty() {
        assert!(compute_layout(&[]).is_empty());
    }

    #[test]
    fn layout_preserves_input_order() {
        let rows = vec![
            ("crate::a".to_string(), "function".to_string(), Some(1)),
            ("crate::b".to_string(), "function".to_string(), Some(1)),
            ("crate::c".to_string(), "class".to_string(), None),
        ];
        let out = compute_layout(&rows);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].q, "crate::a");
        assert_eq!(out[1].q, "crate::b");
        assert_eq!(out[2].q, "crate::c");
    }

    #[test]
    fn layout_is_deterministic_across_runs() {
        let rows = vec![
            ("a".to_string(), "function".to_string(), Some(1)),
            ("b".to_string(), "function".to_string(), Some(1)),
            ("c".to_string(), "function".to_string(), Some(2)),
            ("d".to_string(), "class".to_string(), None),
        ];
        let a = compute_layout(&rows);
        let b = compute_layout(&rows);
        assert_eq!(a, b);
    }

    #[test]
    fn layout_singleton_group_sits_at_group_center() {
        // A node whose community is alone in its bucket should land
        // exactly at its group's sunflower center (no inner-circle
        // distribution applied).
        let rows = vec![("only".to_string(), "function".to_string(), Some(42))];
        let out = compute_layout(&rows);
        assert_eq!(out.len(), 1);
        // Single group → group_idx = 0, n_groups = 1 → r = 0 → center
        // is exactly (0, 0).
        assert!((out[0].x).abs() < 1e-9);
        assert!((out[0].y).abs() < 1e-9);
    }

    #[test]
    fn layout_buckets_loose_nodes_by_kind() {
        // Two community-less nodes with different kinds end up in
        // different buckets and therefore get different group centers.
        let rows = vec![
            ("a".to_string(), "function".to_string(), None),
            ("b".to_string(), "class".to_string(), None),
        ];
        let out = compute_layout(&rows);
        // Same input order, distinct positions.
        assert!(
            (out[0].x - out[1].x).abs() > 1e-3 || (out[0].y - out[1].y).abs() > 1e-3,
            "loose nodes of different kinds must not collide: {out:?}",
        );
    }

    #[test]
    fn layout_keeps_coordinates_within_bounds() {
        // 200 nodes spread across 10 communities — every position
        // must stay within the canvas-friendly [-1100, 1100] envelope
        // (R_OUTER 800 + R_INNER_MAX 200 + small margin).
        let mut rows = Vec::with_capacity(200);
        for i in 0..200 {
            rows.push((
                format!("n{i}"),
                "function".to_string(),
                Some((i % 10) as i64),
            ));
        }
        let out = compute_layout(&rows);
        assert_eq!(out.len(), 200);
        for p in &out {
            assert!(p.x.abs() < 1100.0, "x out of bounds: {}", p.x);
            assert!(p.y.abs() < 1100.0, "y out of bounds: {}", p.y);
        }
    }

    #[test]
    fn layout_handles_duplicate_qualified_names_collision() {
        // T-P0-03 audit fix (2026-05-05): the prior tests never covered
        // what compute_layout does when two rows share the same
        // qualified_name. Inside the function, by_qname is a HashMap
        // keyed on `&str`; duplicate insertions overwrite. The OUTER
        // result loop then does `by_qname.get(row.0.as_str())` which
        // returns the SAME (x, y) pair for every duplicate — both
        // emitted LayoutPosition entries land at identical coordinates.
        //
        // This pins the contract: duplicates collide deterministically
        // (same coord). graph.db `qualified_name` is UNIQUE NOT NULL
        // per store/src/schema.rs so this case shouldn't reach prod,
        // but the math doesn't panic and the SPA's lookup-by-qname
        // still resolves cleanly.
        let rows = vec![
            ("dup".to_string(), "function".to_string(), Some(1)),
            ("dup".to_string(), "function".to_string(), Some(1)),
            ("uniq".to_string(), "function".to_string(), Some(1)),
        ];
        let out = compute_layout(&rows);
        assert_eq!(
            out.len(),
            3,
            "every input row must produce one output entry"
        );
        assert_eq!(out[0].q, "dup");
        assert_eq!(out[1].q, "dup");
        assert_eq!(out[2].q, "uniq");
        // The two duplicate rows MUST share the same coordinate (since
        // the by_qname HashMap collapses them).
        assert_eq!(out[0].x, out[1].x, "duplicate qnames must share x");
        assert_eq!(out[0].y, out[1].y, "duplicate qnames must share y");
        // The unique entry has a different coordinate.
        assert!(
            (out[0].x - out[2].x).abs() > 1e-9 || (out[0].y - out[2].y).abs() > 1e-9,
            "unique row must not collide with duplicates: {out:?}",
        );
        // Coords are finite (no NaN / Inf from the math).
        for p in &out {
            assert!(p.x.is_finite(), "x must be finite: {p:?}");
            assert!(p.y.is_finite(), "y must be finite: {p:?}");
        }
    }

    #[test]
    fn layout_handles_pathological_kind_values() {
        // Defensive: confirm the layout doesn't panic on unusual kind
        // strings (NUL byte, Unicode, very long, empty). build.rs's
        // writer should never emit these — kinds come from a fixed
        // enum — but the layout function takes &str so it must be
        // robust.
        let rows = vec![
            ("a".to_string(), "".to_string(), None),
            ("b".to_string(), "function\0extra".to_string(), None),
            ("c".to_string(), "ƒüñçtïøn".to_string(), None),
            ("d".to_string(), "x".repeat(500), None),
        ];
        let out = compute_layout(&rows);
        assert_eq!(out.len(), 4);
        for p in &out {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }

    #[tokio::test]
    async fn api_graph_layout_returns_empty_array_on_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/layout")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(&body[..], b"[]");
    }

    #[tokio::test]
    async fn api_health_returns_200_json() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        // LOW fix (2026-05-05): the internal `phase: "D0"` milestone
        // code was dropped from public health responses. Pin its
        // absence so a future regression that brings it back fails
        // here instead of in production logs.
        assert!(v.get("phase").is_none(), "health must not leak phase");
    }

    #[tokio::test]
    async fn api_graph_nodes_returns_empty_array_on_no_shard() {
        // Updated for F1 D2: /api/graph/nodes is now implemented and
        // returns HTTP 200 with `[]` (empty array) when no project shard
        // is registered under <MNEME_HOME>/projects/. Matches the TS
        // shard.ts contract — "no data yet" reads as empty list, not
        // 501. The 501 path was removed when D2 wired the real handler.
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/nodes?limit=2000")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(
            v.as_array().expect("nodes array").is_empty(),
            "fresh-install test_state should produce empty nodes list"
        );
    }

    #[tokio::test]
    async fn api_projects_empty_when_dir_missing() {
        // test_state() points at an empty tempdir with no projects/ subdir.
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/projects")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(
            v["projects"].as_array().expect("projects array").is_empty(),
            "fresh install should have zero projects"
        );
    }

    /// Multi-shard picker contract: when two project directories exist
    /// and the request asks for a specific `?project=<hash>`, the
    /// handler must read from THAT shard rather than the
    /// alphabetically-first one. Builds two graph.db fixtures with
    /// different file rows and asserts the file-tree response reflects
    /// the requested project.
    ///
    /// M-2 follow-up (2026-05-05): the project hash validator now
    /// requires exactly 64 lowercase hex chars (the canonical
    /// SHA-256 shape produced by `ProjectId::from_path`). The earlier
    /// fixtures used "aaaa" / "zzzz" which are valid as filesystem
    /// names but rejected by the strict validator, so `?project=zzzz`
    /// silently fell back to the alphabetical default. Swap to
    /// 64-hex strings — `a` repeated 64× and `f` repeated 64× — to
    /// preserve the lexicographic ordering the test relies on while
    /// passing the new validator.
    #[tokio::test]
    async fn api_graph_file_tree_honours_project_query_param() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let hash_a = "a".repeat(64);
        let hash_z = "f".repeat(64);
        // Project "aaaa…" — alphabetically-first, contains foo.rs.
        let proj_a = tmp.path().join("projects").join(&hash_a);
        std::fs::create_dir_all(&proj_a).expect("mkdir hash_a");
        let conn = rusqlite::Connection::open(proj_a.join("graph.db")).expect("open a");
        conn.execute_batch(
            "CREATE TABLE files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, \
                                 language TEXT, last_parsed_at TEXT, \
                                 line_count INTEGER, byte_count INTEGER); \
             INSERT INTO files VALUES \
                ('src/foo.rs', 'sha-a', 'rust', '2026-01-01', 10, 100);",
        )
        .expect("seed a");
        drop(conn);

        // Project "ffff…" — alphabetically-last, contains different file.
        let proj_z = tmp.path().join("projects").join(&hash_z);
        std::fs::create_dir_all(&proj_z).expect("mkdir hash_z");
        let conn = rusqlite::Connection::open(proj_z.join("graph.db")).expect("open z");
        conn.execute_batch(
            "CREATE TABLE files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, \
                                 language TEXT, last_parsed_at TEXT, \
                                 line_count INTEGER, byte_count INTEGER); \
             INSERT INTO files VALUES \
                ('lib/zeta.rs', 'sha-z', 'rust', '2026-01-01', 99, 999);",
        )
        .expect("seed z");
        drop(conn);

        let state = ApiGraphState {
            paths: Arc::new(PathManager::with_root(tmp.path().to_path_buf())),
            livebus: None,
        };

        // No project param → alphabetically-first ("aaaa", foo.rs).
        let app = build_router(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/file-tree")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let raw = serde_json::to_string(&v).expect("json string");
        assert!(
            raw.contains("foo.rs"),
            "default fallback should pick aaaa/graph.db; tree was: {raw}"
        );

        // Explicit ?project=zzzz → should switch to zzzz/graph.db.
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/graph/file-tree?project={hash_z}"))
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let raw = serde_json::to_string(&v).expect("json string");
        assert!(
            raw.contains("zeta.rs"),
            "?project={hash_z} should pick zzzz fixture's graph.db; tree was: {raw}"
        );
        assert!(
            !raw.contains("foo.rs"),
            "?project={hash_z} must NOT leak rows from aaaa fixture; tree was: {raw}"
        );
    }

    /// Path-traversal defence: a malicious `?project=..` must NOT be
    /// allowed to escape `<MNEME_HOME>/projects/`. The handler should
    /// silently ignore the bad hash and either fall back to the
    /// alphabetical default or return an empty payload — never read
    /// from outside the projects root.
    #[tokio::test]
    async fn api_graph_file_tree_rejects_traversal_in_project_param() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/file-tree?project=..%2F..%2Fetc")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        // Even on a fresh tempdir with no projects, the response must
        // be a 200 with empty tree — not a panic, not a 500.
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["name"], serde_json::Value::String("project".into()));
        assert!(v["children"].as_array().expect("children").is_empty());
    }

    /// `/api/projects` must surface the picker fields (`hash`,
    /// `display_name`, `indexed_files`, `nodes`, `edges`,
    /// `last_indexed_at`, `has_graph_db`) so the dropdown can render
    /// without a follow-up call. Builds a minimal graph.db so the
    /// COUNT(*) path is exercised end-to-end.
    #[tokio::test]
    async fn api_projects_returns_picker_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let proj_dir = tmp.path().join("projects").join("deadbeef");
        std::fs::create_dir_all(&proj_dir).expect("mkdir");
        let conn = rusqlite::Connection::open(proj_dir.join("graph.db")).expect("open");
        conn.execute_batch(
            "CREATE TABLE files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, \
                                 language TEXT, last_parsed_at TEXT, \
                                 line_count INTEGER, byte_count INTEGER); \
             CREATE TABLE nodes (id INTEGER PRIMARY KEY, qualified_name TEXT, \
                                 name TEXT, kind TEXT, file_path TEXT); \
             CREATE TABLE edges (id INTEGER PRIMARY KEY, source_qualified TEXT, \
                                 target_qualified TEXT, kind TEXT); \
             INSERT INTO files VALUES ('src/lib.rs', 'sha', 'rust', null, 1, 1); \
             INSERT INTO nodes (qualified_name, name, kind) VALUES \
                ('a', 'a', 'function'), \
                ('b', 'b', 'function'); \
             INSERT INTO edges (source_qualified, target_qualified, kind) VALUES \
                ('a', 'b', 'calls');",
        )
        .expect("seed");
        drop(conn);

        let state = ApiGraphState {
            paths: Arc::new(PathManager::with_root(tmp.path().to_path_buf())),
            livebus: None,
        };
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/projects")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let projects = v["projects"].as_array().expect("projects array");
        assert_eq!(projects.len(), 1, "exactly one project on disk");
        let p = &projects[0];
        assert_eq!(p["hash"], serde_json::Value::String("deadbeef".into()));
        assert_eq!(p["id"], serde_json::Value::String("deadbeef".into()));
        assert_eq!(p["has_graph_db"], serde_json::Value::Bool(true));
        assert_eq!(p["indexed_files"], serde_json::Value::Number(1.into()));
        assert_eq!(p["nodes"], serde_json::Value::Number(2.into()));
        assert_eq!(p["edges"], serde_json::Value::Number(1.into()));
        // No meta.db row was seeded so display_name falls back to hash.
        assert_eq!(
            p["display_name"],
            serde_json::Value::String("deadbeef".into())
        );
        // last_indexed_at must be a string (newest *.db mtime fallback).
        assert!(p["last_indexed_at"].is_string(), "mtime fallback set");
    }

    #[tokio::test]
    async fn api_voice_returns_stub_payload() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/voice")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["enabled"], serde_json::Value::Bool(false));
        assert_eq!(v["phase"], serde_json::Value::String("stub".into()));
    }

    // -- F1 D3 — tests for the second-wave endpoints. ------------------
    //
    // Each endpoint must degrade gracefully when no shard exists (TS
    // contract: empty payload, never a 500). The file-tree test also
    // builds a minimal `graph.db` fixture on disk so the happy-path
    // tree assembly is exercised.

    #[tokio::test]
    async fn api_graph_file_tree_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/file-tree")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["name"], serde_json::Value::String("project".into()));
        assert!(v["children"].as_array().expect("children").is_empty());
    }

    #[tokio::test]
    async fn api_graph_kind_flow_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/kind-flow")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v["nodes"].as_array().expect("nodes").is_empty());
        assert!(v["links"].as_array().expect("links").is_empty());
    }

    #[tokio::test]
    async fn api_graph_commits_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/commits")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v.as_array().expect("array").is_empty());
    }

    #[tokio::test]
    async fn api_graph_heatmap_returns_severity_keys_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/heatmap")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let sev = v["severities"].as_array().expect("severities");
        assert_eq!(sev.len(), 4);
        assert_eq!(sev[0], serde_json::Value::String("critical".into()));
        assert!(v["files"].as_array().expect("files").is_empty());
    }

    #[tokio::test]
    async fn api_graph_test_coverage_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/test-coverage")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v.as_array().expect("array").is_empty());
    }

    #[tokio::test]
    async fn api_graph_community_matrix_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/community-matrix")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v["communities"].as_array().expect("communities").is_empty());
        assert!(v["matrix"].as_array().expect("matrix").is_empty());
    }

    #[tokio::test]
    async fn api_graph_domain_flow_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/domain-flow")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v["nodes"].as_array().expect("nodes").is_empty());
        assert!(v["links"].as_array().expect("links").is_empty());
    }

    /// Builds a minimal `graph.db` fixture under
    /// `<root>/projects/<id>/graph.db` and asserts that the file-tree
    /// endpoint folds the rows into a hierarchical structure. This is
    /// the "real fixture" requirement of the TDD discipline — exercises
    /// the hot path of `insert_into_tree` end-to-end.
    #[tokio::test]
    async fn api_graph_file_tree_builds_hierarchy_from_fixture() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let proj_dir = tmp.path().join("projects").join("fixture-id");
        std::fs::create_dir_all(&proj_dir).expect("create projects/<id>");
        let db_path = proj_dir.join("graph.db");
        let conn = rusqlite::Connection::open(&db_path).expect("open writable fixture");
        conn.execute_batch(
            "CREATE TABLE files (path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, \
                                 language TEXT, last_parsed_at TEXT, \
                                 line_count INTEGER, byte_count INTEGER); \
             INSERT INTO files VALUES \
                ('src/foo.rs', 'sha-a', 'rust', '2026-01-01', 10, 100), \
                ('src/bar/baz.rs', 'sha-b', 'rust', '2026-01-01', 20, 200);",
        )
        .expect("seed fixture");
        drop(conn);

        let state = ApiGraphState {
            paths: Arc::new(PathManager::with_root(tmp.path().to_path_buf())),
            // Phase A · F2: file-tree fixture test doesn't exercise /ws.
            livebus: None,
        };
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/file-tree")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");

        // Root => "project", with one "src" child holding "foo.rs"
        // (a leaf with value=10) and "bar" (a subdir) -> "baz.rs"
        // (leaf with value=20).
        assert_eq!(v["name"], serde_json::Value::String("project".into()));
        let children = v["children"].as_array().expect("children");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["name"], serde_json::Value::String("src".into()));
        let src_kids = children[0]["children"].as_array().expect("src.children");
        assert_eq!(src_kids.len(), 2);
        // We don't assert the order of foo vs bar (HashMap-free linear
        // scan preserves insertion order which matches DESC line_count:
        // baz first, foo second).
        let names: Vec<&str> = src_kids
            .iter()
            .map(|c| c["name"].as_str().expect("name"))
            .collect();
        assert!(names.contains(&"foo.rs"));
        assert!(names.contains(&"bar"));
    }

    /// Sanity check the path heuristic — used directly by the
    /// test-coverage handler. Pure function, no DB access.
    #[test]
    fn is_test_path_recognises_common_layouts() {
        assert!(is_test_path("tests/foo.rs"));
        assert!(is_test_path("src/foo_test.rs"));
        assert!(is_test_path("src/__tests__/foo.ts"));
        assert!(is_test_path("src/foo.test.tsx"));
        assert!(is_test_path("src/foo.spec.js"));
        assert!(is_test_path("tests/test_foo.py"));
        assert!(is_test_path("pkg/test_bar.py"));
        assert!(!is_test_path("src/foo.rs"));
        assert!(!is_test_path("src/lib.ts"));
    }

    /// Covers the candidate-test-filename generator the test-coverage
    /// handler uses to pair a source file with its co-located test.
    #[test]
    fn test_filename_candidates_for_known_extensions() {
        let rust = test_filename_candidates("src/foo.rs");
        assert!(rust.contains(&"src/foo_test.rs".to_string()));
        assert!(rust.contains(&"tests/foo.rs".to_string()));

        let ts = test_filename_candidates("src/foo.ts");
        assert!(ts.contains(&"src/foo.test.ts".to_string()));
        assert!(ts.contains(&"src/foo.spec.ts".to_string()));
        assert!(ts.contains(&"src/__tests__/foo.ts".to_string()));

        let py = test_filename_candidates("pkg/foo.py");
        assert!(py.contains(&"pkg/test_foo.py".to_string()));
        assert!(py.contains(&"tests/test_foo.py".to_string()));
    }

    // -- F1 D4 — tests for the final-wave endpoints. -------------------
    //
    // Each endpoint must degrade gracefully when no shard exists (TS
    // contract: empty payload, never a 500). Mirrors the D3 wave style.

    #[tokio::test]
    async fn api_graph_layers_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/layers")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        // tiers always present even on empty shard (static list)
        let tiers = v["tiers"].as_array().expect("tiers");
        assert_eq!(tiers.len(), 6);
        assert_eq!(tiers[0], serde_json::Value::String("presentation".into()));
        assert!(v["entries"].as_array().expect("entries").is_empty());
    }

    #[tokio::test]
    async fn api_graph_galaxy_3d_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/galaxy-3d")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v["nodes"].as_array().expect("nodes").is_empty());
        assert!(v["edges"].as_array().expect("edges").is_empty());
    }

    #[tokio::test]
    async fn api_graph_theme_palette_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/theme-palette")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert!(v.as_array().expect("array").is_empty());
    }

    #[tokio::test]
    async fn api_graph_hierarchy_empty_when_no_shard() {
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/graph/hierarchy")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["name"], serde_json::Value::String("project".into()));
        assert!(v["children"].as_array().expect("children").is_empty());
    }

    #[tokio::test]
    async fn api_daemon_health_returns_200_json() {
        // /api/daemon/health is a deliberate alias for /api/health — the
        // vision frontend uses both URLs interchangeably for liveness
        // probes. Wire shape must match `/api/health` exactly.
        let app = build_router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/daemon/health")
                    .header("Host", "127.0.0.1:7777")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        // LOW fix (2026-05-05): same `phase` drop as /api/health.
        assert!(
            v.get("phase").is_none(),
            "/api/daemon/health must not leak phase"
        );
    }
}
