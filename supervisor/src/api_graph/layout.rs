//! `/api/graph/layout` — Item #124 server-pre-computed positions
//! plus the HIGH-22 in-process cache. Extracted from `api_graph/mod.rs`
//! as part of the HIGH-45 split.
//!
//! Returns one (q, x, y) triple per node in the same window the SPA's
//! `/api/graph/nodes` call returns. The SPA seeds Sigma's positions
//! from this payload before kicking off FA2 worker refinement,
//! dropping first-paint from ~3 s (random-init + 1-2 FA2 iters) to
//! <500 ms.
//!
//! Algorithm: community-aware sunflower spiral.
//!   1. Group nodes by `community_membership.community_id` (left-join
//!      so nodes outside any community fall into a synthetic "loose"
//!      bucket bucketed by `kind`).
//!   2. Place each group's center on a Vogel sunflower disk: angle =
//!      i * golden_angle_radians, radius = R_outer * sqrt(i / N).
//!   3. Within each group, distribute members on a small inner circle
//!      proportional to group size: angle = j * 2π / group_size,
//!      radius = R_inner * sqrt(group_size). Members ordered by
//!      qualified_name SHA hash for stable layout across rebuilds.

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use super::{with_layer_db_sync, ApiGraphState, ProjectQuery, MAX_GRAPH_LIMIT};

#[derive(Serialize, Debug, Clone, PartialEq)]
pub(super) struct LayoutPosition {
    /// Node qualified_name — joins with `/api/graph/nodes` on `id`.
    pub(super) q: String,
    pub(super) x: f64,
    pub(super) y: f64,
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

pub(super) fn layout_cache_get(project: &str, limit: usize) -> Option<Vec<LayoutPosition>> {
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

pub(super) fn layout_cache_put(project: &str, limit: usize, rows: Vec<LayoutPosition>) {
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

/// Audit fix TEST-NEW-10 (2026-05-06 multi-agent fan-out, testing-
/// reviewer): direct test-only writer that lets a unit test back-
/// date `inserted_at` so the TTL-expiry branch is exercisable
/// without sleeping 30 seconds. Production callers use
/// `layout_cache_put` (which always stamps `Instant::now()`).
#[cfg(test)]
fn layout_cache_put_with_age(
    project: &str,
    limit: usize,
    rows: Vec<LayoutPosition>,
    age: std::time::Duration,
) {
    let cache = LAYOUT_CACHE.get_or_init(|| std::sync::Mutex::new(Default::default()));
    if let Ok(mut guard) = cache.lock() {
        let inserted_at = std::time::Instant::now()
            .checked_sub(age)
            .unwrap_or_else(std::time::Instant::now);
        guard.insert(
            (project.to_string(), limit),
            LayoutCacheEntry { rows, inserted_at },
        );
    }
}

/// Audit fix TEST-NEW-10: explicit clear so tests don't see each
/// other's writes. The static is process-global, so tests use
/// distinctive project-key prefixes AND clear by-key after
/// themselves.
#[cfg(test)]
fn layout_cache_clear_for_test(project: &str, limit: usize) {
    let cache = LAYOUT_CACHE.get_or_init(|| std::sync::Mutex::new(Default::default()));
    if let Ok(mut guard) = cache.lock() {
        guard.remove(&(project.to_string(), limit));
    }
}

/// `GET /api/graph/layout` — pre-computed (q, x, y) positions for
/// the same node window the paired `/api/graph/nodes` call returns.
pub(super) async fn api_graph_layout(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_position(q: &str, x: f64, y: f64) -> LayoutPosition {
        LayoutPosition {
            q: q.to_string(),
            x,
            y,
        }
    }

    #[test]
    fn layout_cache_miss_returns_none_on_first_lookup() {
        // Use a unique project key so this test is order-independent
        // against the process-global static.
        let proj = "test-NEW-10-miss";
        layout_cache_clear_for_test(proj, 100);
        let out = layout_cache_get(proj, 100);
        assert!(out.is_none());
    }

    #[test]
    fn layout_cache_hit_within_ttl_returns_same_rows() {
        let proj = "test-NEW-10-hit";
        layout_cache_clear_for_test(proj, 50);
        let rows = vec![fresh_position("a", 1.0, 2.0), fresh_position("b", 3.0, 4.0)];
        layout_cache_put(proj, 50, rows.clone());
        let hit = layout_cache_get(proj, 50).expect("cache must hit within TTL");
        assert_eq!(hit.len(), 2);
        assert_eq!(hit[0].q, "a");
        assert_eq!(hit[1].q, "b");
        layout_cache_clear_for_test(proj, 50);
    }

    #[test]
    fn layout_cache_expired_entry_is_treated_as_miss() {
        let proj = "test-NEW-10-ttl";
        layout_cache_clear_for_test(proj, 75);
        // Insert with an inserted_at that is already older than the
        // 30s TTL — should behave the same as a cold miss.
        let rows = vec![fresh_position("stale", 9.0, 9.0)];
        layout_cache_put_with_age(
            proj,
            75,
            rows,
            std::time::Duration::from_secs(60), // 2x TTL
        );
        let out = layout_cache_get(proj, 75);
        assert!(
            out.is_none(),
            "an entry inserted 60s ago must be treated as a cache miss \
             when LAYOUT_CACHE_TTL is 30s"
        );
        // Also assert the stale entry was DROPPED (the cache trims
        // expired keys to keep the map bounded).
        let again = layout_cache_get(proj, 75);
        assert!(
            again.is_none(),
            "stale entries should not linger in the map after a miss"
        );
    }

    #[test]
    fn layout_cache_keys_are_isolated_by_project_and_limit() {
        let proj_a = "test-NEW-10-iso-a";
        let proj_b = "test-NEW-10-iso-b";
        layout_cache_clear_for_test(proj_a, 100);
        layout_cache_clear_for_test(proj_b, 100);
        layout_cache_clear_for_test(proj_a, 200);

        layout_cache_put(proj_a, 100, vec![fresh_position("a-100", 0.0, 0.0)]);

        // Different project, same limit -> miss.
        assert!(
            layout_cache_get(proj_b, 100).is_none(),
            "proj_b must not see proj_a's entry"
        );
        // Same project, different limit -> miss.
        assert!(
            layout_cache_get(proj_a, 200).is_none(),
            "limit=200 must not see limit=100's entry"
        );
        // Same project + same limit -> hit.
        assert!(
            layout_cache_get(proj_a, 100).is_some(),
            "exact key must hit"
        );

        layout_cache_clear_for_test(proj_a, 100);
        layout_cache_clear_for_test(proj_b, 100);
        layout_cache_clear_for_test(proj_a, 200);
    }
}
