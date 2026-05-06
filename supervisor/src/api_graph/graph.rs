//! Graph data handlers extracted from `api_graph/mod.rs` (HIGH-45 split).
//!
//! Owns all 16 `/api/graph/*` data endpoints (nodes, edges, status, files,
//! findings, file-tree, kind-flow, domain-flow, community-matrix, commits,
//! heatmap, layers, galaxy-3d, test-coverage, theme-palette, hierarchy),
//! their DTOs, and the private helper functions that only these handlers use.
//!
//! Shared infrastructure (`with_layer_db_sync`, `find_active_layer_db`,
//! `is_valid_layer_name`, `is_valid_project_hash`) remains in
//! `api_graph/mod.rs` because `layout.rs` also imports them from `super`.

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use super::{with_layer_db_sync, ApiGraphState, ProjectQuery, MAX_GRAPH_LIMIT};

// ---------------------------------------------------------------------------
// F1 D2 — Real `/api/graph/{nodes,edges,status,files,findings}` endpoints
// ---------------------------------------------------------------------------

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
pub(super) async fn api_graph_nodes(
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
pub(super) async fn api_graph_edges(
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

/// `GET /api/graph/status` — shard health + counts for the status bar.
pub(super) async fn api_graph_status(
    State(state): State<ApiGraphState>,
    Query(q): Query<ProjectQuery>,
) -> impl IntoResponse {
    use super::find_active_layer_db;

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

pub(super) async fn api_graph_files(
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

pub(super) async fn api_graph_findings(
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
pub(super) async fn api_graph_file_tree(
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
pub(super) async fn api_graph_kind_flow(
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
pub(super) async fn api_graph_domain_flow(
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
pub(super) async fn api_graph_community_matrix(
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
pub(super) async fn api_graph_commits(
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
pub(super) async fn api_graph_heatmap(
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
pub(super) async fn api_graph_test_coverage(
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
pub(super) async fn api_graph_layers(
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
pub(super) async fn api_graph_galaxy_3d(
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
pub(super) async fn api_graph_theme_palette(
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
pub(super) async fn api_graph_hierarchy(
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

// ---------------------------------------------------------------------------
// Tests for pure helper functions (no DB / router needed)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{is_test_path, test_filename_candidates};

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
}
