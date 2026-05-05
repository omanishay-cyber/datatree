//! Smart question generation for the mneme codebase explorer.
//!
//! # Algorithm
//!
//! Given a flat list of graph nodes + edges (loaded by the caller from
//! `graph.db`), this module produces a ranked list of questions an AI
//! should ask about the codebase. Questions are ordered by an aggregate
//! score that combines three orthogonal signals:
//!
//! ```text
//! score = centrality_z * 0.4 + complexity_z * 0.3 + anomaly_score * 0.3
//! ```
//!
//! ## Centrality
//!
//! We use **in-degree** (how many other nodes depend on this node) as the
//! primary centrality measure. Betweenness would be more accurate but requires
//! O(VE) Brandes — too slow for a synchronous MCP tool call that must return
//! within ~100 ms on a graph with 10k+ nodes. In-degree is O(E) and highly
//! correlated with betweenness in practice (high-degree hubs are almost always
//! betweenness bottlenecks).
//!
//! ## Complexity
//!
//! Proxy = `(line_end - line_start).max(0)`. This is a loose estimate of
//! cyclomatic complexity — longer functions tend to have more branches. Nodes
//! with no line information receive the corpus median so they don't dominate
//! the ranking unfairly.
//!
//! ## Anomaly classes
//!
//! | Class      | Definition                                   | Score |
//! |------------|----------------------------------------------|-------|
//! | god node   | in-degree > 95th percentile across all nodes | 1.0   |
//! | cycle node | member of a directed cycle                   | 0.7   |
//! | orphan     | both in-degree = 0 **and** out-degree = 0    | 0.5   |
//!
//! A node can match multiple classes; scores are summed (capped at 1.0).
//!
//! ## Question kinds
//!
//! | `kind` arg   | Which nodes are included       |
//! |--------------|-------------------------------|
//! | `starter`    | top-centrality, non-anomalous  |
//! | `deep-dive`  | high-complexity nodes          |
//! | `anomaly`    | god nodes + cycle members + orphans |
//! | (all)        | all nodes, mixed               |

use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// A single node in the graph as supplied by the caller.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Stable identifier (qualified_name from the DB).
    pub qualified_name: String,
    /// Human-readable name (bare function / class name).
    pub name: String,
    /// Kind: "function", "class", "file", "module", …
    pub kind: String,
    /// Absolute path to the source file. `None` for synthetic nodes.
    pub file_path: Option<String>,
    /// First source line (1-based). `None` when unavailable.
    pub line_start: Option<i64>,
    /// Last source line (1-based). `None` when unavailable.
    pub line_end: Option<i64>,
}

/// A directed edge between two nodes.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Qualified name of the source node (caller / importer).
    pub source: String,
    /// Qualified name of the target node (callee / imported).
    pub target: String,
    /// Edge type: "calls", "imports", "inherits", …
    pub kind: String,
}

/// Which flavour of questions to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind {
    /// Broad orientation questions about high-traffic nodes.
    Starter,
    /// Detailed questions about complex / large nodes.
    DeepDive,
    /// Structural health questions: god nodes, cycles, dead code.
    Anomaly,
    /// All questions (no filter by kind).
    All,
}

impl QuestionKind {
    /// Parse from the string values accepted by the MCP tool.
    pub fn from_str(s: &str) -> Self {
        match s {
            "starter" => Self::Starter,
            "deep-dive" => Self::DeepDive,
            "anomaly" => Self::Anomaly,
            _ => Self::All,
        }
    }
}

/// A generated question with its score and metadata.
#[derive(Debug, Clone)]
pub struct SmartQuestion {
    /// The question text.
    pub question: String,
    /// Composite score in [0.0, 1.0] (higher = more important to ask).
    pub score: f64,
    /// Why this question was generated.
    pub justification: String,
    /// Qualified names of the nodes this question relates to.
    pub related_nodes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Generate the top-`limit` smart questions for a corpus graph.
///
/// # Arguments
/// * `nodes`  — all graph nodes for the project
/// * `edges`  — all graph edges for the project
/// * `limit`  — maximum number of questions to return
/// * `kind`   — filter to a specific question flavour
///
/// Returns a list sorted by `score` descending.
pub fn generate_questions(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    limit: usize,
    kind: QuestionKind,
) -> Vec<SmartQuestion> {
    if nodes.is_empty() {
        return Vec::new();
    }

    // --- Step 1: compute centrality metrics ---------------------------------

    // in_degree[qname] = count of edges where target = qname
    let mut in_degree: HashMap<String, usize> = HashMap::with_capacity(nodes.len());
    // out_degree[qname] = count of edges where source = qname
    let mut out_degree: HashMap<String, usize> = HashMap::with_capacity(nodes.len());

    // Initialise every node to zero so orphans appear in the maps.
    for n in nodes {
        in_degree.entry(n.qualified_name.clone()).or_insert(0);
        out_degree.entry(n.qualified_name.clone()).or_insert(0);
    }
    for e in edges {
        *in_degree.entry(e.target.clone()).or_insert(0) += 1;
        *out_degree.entry(e.source.clone()).or_insert(0) += 1;
    }

    // --- Step 2: detect anomaly classes ------------------------------------

    // Detect cycles via iterative Tarjan SCC.
    let cycle_members = tarjan_cycle_members(nodes, edges);

    // God node threshold: 95th percentile of in-degree.
    let god_threshold = percentile_95_in_degree(&in_degree);

    // --- Step 3: compute complexity proxy for each node --------------------

    // line span (line_end - line_start). Missing → use median later.
    let mut line_spans: Vec<(String, i64)> = Vec::with_capacity(nodes.len());
    let mut valid_spans: Vec<i64> = Vec::new();
    for n in nodes {
        let span = match (n.line_start, n.line_end) {
            (Some(s), Some(e)) if e >= s => {
                let v = e - s;
                valid_spans.push(v);
                v
            }
            _ => -1, // sentinel for "missing"
        };
        line_spans.push((n.qualified_name.clone(), span));
    }
    // Median of valid spans (used as fill for missing).
    let median_span = if valid_spans.is_empty() {
        0i64
    } else {
        valid_spans.sort_unstable();
        valid_spans[valid_spans.len() / 2]
    };

    let complexity_map: HashMap<String, i64> = line_spans
        .into_iter()
        .map(|(qn, span)| (qn, if span < 0 { median_span } else { span }))
        .collect();

    // --- Step 4: z-score normalise centrality + complexity ------------------

    let (centrality_z_map, complexity_z_map) = {
        let centrality_vals: Vec<f64> = nodes
            .iter()
            .map(|n| *in_degree.get(&n.qualified_name).unwrap_or(&0) as f64)
            .collect();
        let complexity_vals: Vec<f64> = nodes
            .iter()
            .map(|n| *complexity_map.get(&n.qualified_name).unwrap_or(&0) as f64)
            .collect();

        let cen_z = z_score_map(nodes, &centrality_vals);
        let cpx_z = z_score_map(nodes, &complexity_vals);
        (cen_z, cpx_z)
    };

    // --- Step 5: generate candidate questions for each node ----------------

    let mut candidates: Vec<SmartQuestion> = Vec::new();

    for n in nodes {
        let qname = &n.qualified_name;
        let in_d = *in_degree.get(qname).unwrap_or(&0);
        let out_d = *out_degree.get(qname).unwrap_or(&0);

        let centrality_z = *centrality_z_map.get(qname).unwrap_or(&0.0);
        let complexity_z = *complexity_z_map.get(qname).unwrap_or(&0.0);

        // Anomaly score: sum of class contributions, capped at 1.0.
        let is_god = in_d >= god_threshold;
        let is_cycle = cycle_members.contains(qname);
        let is_orphan = in_d == 0 && out_d == 0;

        let anomaly_score: f64 = (if is_god { 1.0_f64 } else { 0.0 }
            + if is_cycle { 0.7 } else { 0.0 }
            + if is_orphan { 0.5 } else { 0.0 })
        .min(1.0);

        let composite =
            (centrality_z.max(0.0) * 0.4) + (complexity_z.max(0.0) * 0.3) + (anomaly_score * 0.3);

        // Filter by requested kind.
        let passes_filter = match kind {
            QuestionKind::Starter => !is_god && !is_cycle && !is_orphan && centrality_z > 0.0,
            QuestionKind::DeepDive => complexity_z > 0.5,
            QuestionKind::Anomaly => is_god || is_cycle || is_orphan,
            QuestionKind::All => true,
        };

        if !passes_filter {
            continue;
        }

        // Skip nodes with effectively zero score (noise below threshold).
        if composite < 0.01 && kind == QuestionKind::All {
            continue;
        }

        let (question, justification) =
            build_question(n, in_d, out_d, is_god, is_cycle, is_orphan, complexity_z);

        candidates.push(SmartQuestion {
            question,
            score: composite.clamp(0.0, 1.0),
            justification,
            related_nodes: vec![qname.clone()],
        });
    }

    // Add one cycle-level question per unique cycle (multi-node anomaly).
    if matches!(kind, QuestionKind::Anomaly | QuestionKind::All) {
        let cycle_questions = cycle_level_questions(nodes, edges, &cycle_members);
        candidates.extend(cycle_questions);
    }

    // Sort by composite score descending, then by qualified_name for stability.
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.related_nodes
                    .first()
                    .unwrap_or(&String::new())
                    .cmp(b.related_nodes.first().unwrap_or(&String::new()))
            })
    });

    candidates.truncate(limit);
    candidates
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compute the 95th percentile of in-degree values.
///
/// Returns 0 if the map is empty (no god-node threshold can be meaningful).
fn percentile_95_in_degree(in_degree: &HashMap<String, usize>) -> usize {
    if in_degree.is_empty() {
        return 0;
    }
    let mut vals: Vec<usize> = in_degree.values().copied().collect();
    vals.sort_unstable();
    let idx = ((vals.len() as f64) * 0.95) as usize;
    let idx = idx.min(vals.len().saturating_sub(1));
    // Only meaningful if the threshold is > 0 (otherwise everything is a
    // "god node" in a graph with all-zero in-degrees, which is unhelpful).
    vals[idx].max(1)
}

/// Compute z-scores for a slice of values, returning a map keyed by
/// the node's qualified_name at the same index.
///
/// Uses population std-dev (not sample) for speed; the slight bias is
/// irrelevant for ranking purposes.
fn z_score_map(nodes: &[GraphNode], vals: &[f64]) -> HashMap<String, f64> {
    debug_assert_eq!(nodes.len(), vals.len());
    if nodes.is_empty() {
        return HashMap::new();
    }

    let n = vals.len() as f64;
    let mean = vals.iter().copied().sum::<f64>() / n;
    let variance = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    nodes
        .iter()
        .zip(vals.iter())
        .map(|(node, &val)| {
            let z = if std_dev < f64::EPSILON {
                0.0
            } else {
                (val - mean) / std_dev
            };
            (node.qualified_name.clone(), z)
        })
        .collect()
}

/// Iterative Tarjan SCC; returns the set of qualified_names that appear
/// in any cycle (SCC with ≥ 2 members).
fn tarjan_cycle_members(nodes: &[GraphNode], edges: &[GraphEdge]) -> HashSet<String> {
    // Build adjacency map.
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let all_names: HashSet<&str> = nodes.iter().map(|n| n.qualified_name.as_str()).collect();
    for e in edges {
        if all_names.contains(e.source.as_str()) && all_names.contains(e.target.as_str()) {
            adj.entry(e.source.as_str())
                .or_default()
                .push(e.target.as_str());
        }
    }

    let mut index_counter: usize = 0;
    let mut indices: HashMap<&str, usize> = HashMap::new();
    let mut lowlinks: HashMap<&str, usize> = HashMap::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut cycle_members: HashSet<String> = HashSet::new();

    for start in nodes.iter().map(|n| n.qualified_name.as_str()) {
        if indices.contains_key(start) {
            continue;
        }

        // Iterative DFS.
        // Work stack entries: (node, child_index_into_successors).
        let mut work: Vec<(&str, usize)> = vec![(start, 0)];
        indices.insert(start, index_counter);
        lowlinks.insert(start, index_counter);
        index_counter += 1;
        stack.push(start);
        on_stack.insert(start);

        while let Some(frame) = work.last_mut() {
            let v = frame.0;
            let child_idx = frame.1;
            let successors = adj.get(v).map(Vec::as_slice).unwrap_or(&[]);
            if child_idx < successors.len() {
                frame.1 += 1;
                let w = successors[child_idx];
                if !indices.contains_key(w) {
                    indices.insert(w, index_counter);
                    lowlinks.insert(w, index_counter);
                    index_counter += 1;
                    stack.push(w);
                    on_stack.insert(w);
                    work.push((w, 0));
                } else if on_stack.contains(w) {
                    let w_idx = indices[w];
                    let ll = lowlinks.entry(v).or_insert(usize::MAX);
                    *ll = (*ll).min(w_idx);
                }
            } else {
                // Pop frame — propagate lowlink and emit SCC if root.
                work.pop();
                let v_ll = lowlinks[v];
                let v_idx = indices[v];

                if let Some(parent_frame) = work.last() {
                    let p = parent_frame.0;
                    let p_ll = *lowlinks.get(p).unwrap_or(&usize::MAX);
                    lowlinks.insert(p, p_ll.min(v_ll));
                }

                if v_ll == v_idx {
                    // SCC root — pop until we reach v.
                    let mut scc: Vec<&str> = Vec::new();
                    while let Some(top) = stack.last().copied() {
                        stack.pop();
                        on_stack.remove(top);
                        scc.push(top);
                        if top == v {
                            break;
                        }
                    }
                    if scc.len() >= 2 {
                        for member in &scc {
                            cycle_members.insert((*member).to_owned());
                        }
                    }
                }
            }
        }
    }

    cycle_members
}

/// Generate per-cycle questions (one question per unique cycle that involves
/// at least 2 named nodes).
fn cycle_level_questions(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    cycle_members: &HashSet<String>,
) -> Vec<SmartQuestion> {
    if cycle_members.is_empty() {
        return Vec::new();
    }

    let node_names: HashSet<&str> = nodes.iter().map(|n| n.qualified_name.as_str()).collect();

    // Build adjacency restricted to cycle members.
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges {
        if cycle_members.contains(&e.source) && cycle_members.contains(&e.target) {
            if node_names.contains(e.source.as_str()) && node_names.contains(e.target.as_str()) {
                adj.entry(e.source.as_str())
                    .or_default()
                    .push(e.target.as_str());
            }
        }
    }

    // Simple BFS paths: find shortest cycle for each member as starting point.
    // We only keep the first 3 distinct cycles to avoid flooding the output.
    let mut seen_cycles: HashSet<Vec<String>> = HashSet::new();
    let mut questions: Vec<SmartQuestion> = Vec::new();

    for start in cycle_members.iter().take(10) {
        if let Some(path) = bfs_shortest_cycle(start.as_str(), &adj) {
            let mut canonical = path.clone();
            canonical.sort();
            if seen_cycles.contains(&canonical) || seen_cycles.len() >= 3 {
                continue;
            }
            seen_cycles.insert(canonical);

            let chain: Vec<String> = path.iter().map(|n| short_name(n)).collect::<Vec<_>>();
            let chain_str = chain.join(" → ");
            questions.push(SmartQuestion {
                question: format!(
                    "Should the circular dependency {chain_str} → {} be broken? If so, which edge should be removed or inverted?",
                    chain.first().map(String::as_str).unwrap_or("?")
                ),
                score: 0.85,
                justification: format!(
                    "Detected a directed cycle of length {}. Cycles in a dependency graph prevent clean layering and make reasoning about initialization order, testing, and refactoring significantly harder.",
                    path.len()
                ),
                related_nodes: path,
            });
        }
    }

    questions
}

/// BFS from `start` within `adj`; returns the shortest cycle as a sequence
/// of qualified names (including `start` at index 0, not repeated at the end).
fn bfs_shortest_cycle<'a>(
    start: &'a str,
    adj: &HashMap<&'a str, Vec<&'a str>>,
) -> Option<Vec<String>> {
    use std::collections::VecDeque;
    let mut visited: HashMap<&str, &str> = HashMap::new(); // node → parent
    let mut queue: VecDeque<&str> = VecDeque::new();
    queue.push_back(start);
    visited.insert(start, start);

    while let Some(node) = queue.pop_front() {
        for &next in adj.get(node).map(Vec::as_slice).unwrap_or(&[]) {
            if next == start {
                // Reconstruct path.
                let mut path = vec![start.to_owned()];
                let mut cur = node;
                while cur != start {
                    path.push(cur.to_owned());
                    cur = visited[cur];
                }
                path.reverse();
                return Some(path);
            }
            if !visited.contains_key(next) {
                visited.insert(next, node);
                queue.push_back(next);
            }
        }
    }
    None
}

/// Build a single node-level question + justification.
fn build_question(
    node: &GraphNode,
    in_d: usize,
    out_d: usize,
    is_god: bool,
    is_cycle: bool,
    is_orphan: bool,
    complexity_z: f64,
) -> (String, String) {
    let label = short_name(&node.qualified_name);
    let kind = node.kind.as_str();
    let file_hint = node
        .file_path
        .as_deref()
        .and_then(|p| p.rsplit(['/', '\\']).next())
        .unwrap_or("");

    if is_orphan {
        let file_part = if file_hint.is_empty() {
            String::new()
        } else {
            format!(" in `{file_hint}`")
        };
        return (
            format!(
                "Is `{label}`{file_part} dead code? It has no callers and imports nothing — was it intentionally left or accidentally orphaned?"
            ),
            format!(
                "`{label}` ({kind}) has in-degree=0 and out-degree=0, meaning nothing references it and it references nothing in the indexed graph. Likely dead code or a recently-added stub not yet wired up."
            ),
        );
    }

    if is_god {
        return (
            format!(
                "What does `{label}` do, and why is everything calling it? With {in_d} dependants it is the highest-traffic node in this codebase — what would break if its signature changed?"
            ),
            format!(
                "`{label}` ({kind}) has in-degree={in_d} which exceeds the 95th percentile, making it a god node. Changes here have the widest blast radius in the project."
            ),
        );
    }

    if is_cycle {
        return (
            format!(
                "Why does `{label}` participate in a circular dependency? What is the intended ownership boundary, and can the cycle be broken by introducing an abstraction?"
            ),
            format!(
                "`{label}` ({kind}) is part of a directed cycle in the dependency graph (in={in_d}, out={out_d}). Cycles impede testability and layered architecture."
            ),
        );
    }

    if complexity_z > 1.5 {
        let line_hint = match (node.line_start, node.line_end) {
            (Some(s), Some(e)) => format!(" (~{} lines)", e - s),
            _ => String::new(),
        };
        return (
            format!(
                "What does `{label}`{line_hint} do, and should it be split? It is significantly larger than the codebase median — does it have a single clear responsibility?"
            ),
            format!(
                "`{label}` ({kind}) has a line-span that is {complexity_z:.1}σ above the corpus median, making it a complexity outlier likely to hide bugs and resist change."
            ),
        );
    }

    // Default: high-centrality node, non-anomalous.
    (
        format!(
            "What is the role of `{label}` in this codebase? It has {in_d} upstream dependants — what contract does it expose and who should be notified if it changes?"
        ),
        format!(
            "`{label}` ({kind}) has in-degree={in_d} and out-degree={out_d}, placing it in the top tier by centrality. Understanding it is essential before any large refactor."
        ),
    )
}

/// Return the short (rightmost `::`- or `.`-separated) segment of a
/// qualified name, falling back to the full string.
fn short_name(qname: &str) -> String {
    qname
        .rsplit("::")
        .next()
        .or_else(|| qname.rsplit('.').next())
        .unwrap_or(qname)
        .trim()
        .to_owned()
}

// ---------------------------------------------------------------------------
// Unit tests (in tests.rs via `mod smart_questions_tests`)
// ---------------------------------------------------------------------------
//
// See brain/src/tests.rs — the three required tests live there so they are
// compiled with `#[cfg(test)]` and `cargo test --package mneme-brain` picks
// them up alongside the rest of the crate suite.
