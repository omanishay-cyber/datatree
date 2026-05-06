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

// HIGH-45 split (2026-05-06 audit): Query + State moved with their only
// consumers (the graph data handlers) into api_graph/graph.rs.
// Json moved with the health/project handlers into health.rs / projects.rs.
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use common::PathManager;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

// HIGH-45 split (2026-05-06 audit): health-probe handlers — api_health,
// api_daemon_health, voice_stub, stub_handler — extracted into a
// dedicated submodule to start shrinking the 3,977-LOC god file.
// build_router() pulls them in via this `use`, so existing callers see
// no behaviour or wire-shape change. Future commits in v0.4.1 extract
// projects/, layout/, and graph/ submodules the same way.
mod graph;
mod health;
mod layout;
mod projects;
use graph::{
    api_graph_commits, api_graph_community_matrix, api_graph_domain_flow, api_graph_edges,
    api_graph_file_tree, api_graph_files, api_graph_findings, api_graph_galaxy_3d,
    api_graph_heatmap, api_graph_hierarchy, api_graph_kind_flow, api_graph_layers, api_graph_nodes,
    api_graph_status, api_graph_test_coverage, api_graph_theme_palette,
};
use health::{api_daemon_health, api_health, stub_handler, voice_stub};
use layout::api_graph_layout;
use projects::api_projects;

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
        .layer(axum::middleware::from_fn(
            crate::ws::enforce_origin_and_host,
        ))
        // HIGH-42: stamp every response with the API version header so
        // older / cached SPA bundles can detect wire-shape drift.
        .layer(axum::middleware::from_fn(inject_api_version_header))
        .with_state(state)
}

// HIGH-45 (2026-05-06 audit): api_health moved to api_graph/health.rs.
// HIGH-45 (2026-05-06 audit): DiscoveredProject + ProjectsResponse +
// MetaProjectRow + load_meta_projects + newest_db_mtime_iso +
// count_table + api_projects all moved to api_graph/projects.rs.
// HIGH-45 (2026-05-06 audit): voice_stub + stub_handler moved to api_graph/health.rs.
// HIGH-45 (2026-05-06 audit): all 16 graph data handlers + DTOs + private helpers
// (size_for_kind, color_for_kind, domain_of, tier_of, is_test_path,
// test_filename_candidates, insert_into_tree, insert_into_hierarchy,
// extract_color_tokens, MapTakeOrDefault) moved to api_graph/graph.rs.

/// `<layer>.db` shard locator.
///
/// When `requested` is `Some(hash)` and the directory
/// `<MNEME_HOME>/projects/<hash>/<layer>.db` exists, return that path
/// directly — supports the multi-project picker in the vision SPA.
/// Otherwise (no hash, missing hash, or missing layer file) fall back
/// to the legacy "first project under projects/ whose `<layer>.db`
/// exists, alphabetically" lookup so single-project installs keep
/// working without any param.
pub(super) fn find_active_layer_db(
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
pub(super) async fn with_layer_db_sync<F, T>(
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

// HIGH-45 (2026-05-06 audit): all 16 graph data handlers, their DTOs
// (GraphNodeOut, GraphNodeMeta, GraphEdgeOut, GraphEdgeMeta, GraphStatusOut, etc.)
// and private helpers (size_for_kind, color_for_kind, domain_of, tier_of,
// is_test_path, test_filename_candidates, insert_into_tree, insert_into_hierarchy,
// extract_color_tokens, MapTakeOrDefault) moved to api_graph/graph.rs.

#[cfg(test)]
mod tests {
    use super::*;
    // HIGH-45 split: compute_layout now lives in the sibling layout
    // submodule. Re-import it here so the layout pure-function tests keep
    // working unchanged. LayoutPosition is used only in test comments so
    // it is not imported (avoids unused-import warning).
    use super::layout::compute_layout;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
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

    // HIGH-45 split: is_test_path + test_filename_candidates are private
    // to api_graph/graph.rs. Their pure-function tests moved there too —
    // see api_graph/graph.rs::tests::{is_test_path_recognises_common_layouts,
    // test_filename_candidates_for_known_extensions}.

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
