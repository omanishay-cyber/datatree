//! Periodic Leiden runner.
//!
//! Per design §21.2.2 #4 — after Leiden converges any community holding
//! more than `oversize_ratio` of all nodes is recursively split by re-running
//! Leiden on the induced subgraph with a higher resolution. The recursion
//! depth is bounded so pathological inputs cannot wedge the runner.

use std::collections::HashMap;

use petgraph::graph::UnGraph;
use tracing::{debug, info};

use crate::error::BrainResult;
use crate::leiden::{Community, LeidenConfig, LeidenSolver};
use crate::NodeId;

/// Tunables for the runner. Defaults match design §21.2.
#[derive(Debug, Clone, Copy)]
pub struct ClusterRunnerConfig {
    /// Base Leiden config.
    pub leiden: LeidenConfig,
    /// Communities holding more than this fraction of total nodes get split.
    pub oversize_ratio: f32,
    /// Max recursive split depth.
    pub max_split_depth: usize,
    /// Resolution multiplier applied per recursion level.
    pub split_resolution_step: f64,
}

impl Default for ClusterRunnerConfig {
    fn default() -> Self {
        Self {
            leiden: LeidenConfig::default(),
            oversize_ratio: 0.25,
            max_split_depth: 3,
            split_resolution_step: 1.5,
        }
    }
}

/// Runner state.
#[derive(Debug, Clone, Default)]
pub struct ClusterRunner {
    cfg: ClusterRunnerConfig,
}

impl ClusterRunner {
    pub fn new(cfg: ClusterRunnerConfig) -> Self {
        Self { cfg }
    }

    /// Run Leiden over a list of weighted edges. Self-loops and zero/negative
    /// weights are silently dropped.
    pub fn run(&self, edges: &[(NodeId, NodeId, f32)]) -> BrainResult<Vec<Community>> {
        if edges.is_empty() {
            return Ok(Vec::new());
        }

        let graph = build_graph(edges);
        let total_nodes = graph.node_count();
        info!(nodes = total_nodes, edges = edges.len(), "leiden start");

        let solver = LeidenSolver::new(self.cfg.leiden);
        let initial = solver.run(&graph)?;
        let final_communities = self.split_oversized(edges, total_nodes, initial, 0)?;
        info!(
            communities = final_communities.len(),
            "leiden done"
        );
        Ok(final_communities)
    }

    fn split_oversized(
        &self,
        edges: &[(NodeId, NodeId, f32)],
        total_nodes: usize,
        communities: Vec<Community>,
        depth: usize,
    ) -> BrainResult<Vec<Community>> {
        if depth >= self.cfg.max_split_depth || total_nodes == 0 {
            return Ok(reindex(communities));
        }
        let threshold = ((total_nodes as f32) * self.cfg.oversize_ratio).ceil() as usize;
        if threshold < 4 {
            return Ok(reindex(communities));
        }

        let mut out: Vec<Community> = Vec::with_capacity(communities.len());
        for comm in communities {
            if comm.members.len() <= threshold {
                out.push(comm);
                continue;
            }

            // Build subgraph induced by `comm.members`.
            let member_set: HashMap<NodeId, ()> =
                comm.members.iter().map(|n| (*n, ())).collect();
            let sub_edges: Vec<(NodeId, NodeId, f32)> = edges
                .iter()
                .filter(|(a, b, _)| member_set.contains_key(a) && member_set.contains_key(b))
                .copied()
                .collect();

            if sub_edges.is_empty() {
                out.push(comm);
                continue;
            }

            // Higher resolution → smaller pieces.
            let mut sub_cfg = self.cfg;
            sub_cfg.leiden.resolution *= self.cfg.split_resolution_step;
            sub_cfg.leiden.seed = sub_cfg.leiden.seed.wrapping_add(depth as u64 + 1);
            let sub_runner = ClusterRunner::new(sub_cfg);

            let sub_graph = build_graph(&sub_edges);
            let solver = LeidenSolver::new(sub_cfg.leiden);
            let sub_initial = solver.run(&sub_graph)?;

            // Only accept the split if it actually broke the community apart.
            if sub_initial.len() <= 1 {
                out.push(comm);
                continue;
            }

            let split = sub_runner.split_oversized(
                &sub_edges,
                comm.members.len(),
                sub_initial,
                depth + 1,
            )?;
            debug!(
                parent_size = comm.members.len(),
                pieces = split.len(),
                depth,
                "split oversized community"
            );
            out.extend(split);
        }
        Ok(reindex(out))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_graph(edges: &[(NodeId, NodeId, f32)]) -> UnGraph<NodeId, f32> {
    let mut g: UnGraph<NodeId, f32> = UnGraph::new_undirected();
    let mut seen: HashMap<NodeId, petgraph::graph::NodeIndex> = HashMap::new();
    for (a, b, w) in edges {
        if !w.is_finite() || *w <= 0.0 {
            continue;
        }
        let ai = *seen.entry(*a).or_insert_with(|| g.add_node(*a));
        let bi = *seen.entry(*b).or_insert_with(|| g.add_node(*b));
        if ai == bi {
            continue;
        }
        g.add_edge(ai, bi, *w);
    }
    g
}

fn reindex(mut communities: Vec<Community>) -> Vec<Community> {
    // Stable order: largest first, then by minimum NodeId for determinism.
    communities.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then_with(|| a.members.iter().min().cmp(&b.members.iter().min()))
    });
    for (i, c) in communities.iter_mut().enumerate() {
        c.id = i as u32;
    }
    communities
}
