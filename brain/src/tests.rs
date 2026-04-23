//! Crate-level tests.
//!
//! These tests must pass on a machine with **no models installed** — they
//! exercise the degraded-mode paths and the pure-Rust algorithms (Leiden,
//! deterministic concept extraction, signature-based summaries).

use crate::cluster_runner::{ClusterRunner, ClusterRunnerConfig};
use crate::concept::{ConceptExtractor, ConceptSource, ExtractInput};
use crate::embed_store::EmbedStore;
use crate::embeddings::{Embedder, EMBEDDING_DIM};
use crate::leiden::{LeidenConfig, LeidenSolver};
use crate::summarize::Summarizer;
use crate::NodeId;

use petgraph::graph::UnGraph;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Embedder
// ---------------------------------------------------------------------------

#[test]
fn embed_determinism_same_text_same_vector() {
    // Use a path guaranteed to NOT exist so the embedder enters degraded mode.
    let bogus_model = PathBuf::from("/nonexistent/mneme/model.onnx");
    let bogus_tok = PathBuf::from("/nonexistent/mneme/tokenizer.json");
    let e = Embedder::new(&bogus_model, &bogus_tok).expect("embedder build");
    assert!(!e.is_ready(), "expected degraded mode");

    let v1 = e.embed("hello world").unwrap();
    let v2 = e.embed("hello world").unwrap();
    assert_eq!(v1.len(), EMBEDDING_DIM);
    assert_eq!(v1, v2, "same text must produce identical vector");

    // Batch path: same text twice ⇒ identical rows.
    let batch = e.embed_batch(&["hello world", "hello world"]).unwrap();
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0], batch[1]);
    assert_eq!(batch[0], v1);
}

// ---------------------------------------------------------------------------
// Embed store
// ---------------------------------------------------------------------------

#[test]
fn embed_store_round_trip_and_nearest() {
    let dir = tempfile::tempdir().unwrap();
    let store = EmbedStore::open(dir.path()).unwrap();

    // Three orthogonal-ish vectors.
    let mut a = vec![0f32; EMBEDDING_DIM];
    a[0] = 1.0;
    let mut b = vec![0f32; EMBEDDING_DIM];
    b[1] = 1.0;
    let mut c = vec![0f32; EMBEDDING_DIM];
    c[0] = 0.7;
    c[1] = 0.7;

    let na = NodeId::new(1);
    let nb = NodeId::new(2);
    let nc = NodeId::new(3);

    store.upsert(na, &a).unwrap();
    store.upsert(nb, &b).unwrap();
    store.upsert(nc, &c).unwrap();
    assert_eq!(store.len(), 3);

    // Query close to `a` ⇒ a should be top result.
    let mut q = vec![0f32; EMBEDDING_DIM];
    q[0] = 1.0;
    let hits = store.nearest(&q, 2);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].node, na);

    // Persist + reopen.
    store.flush().unwrap();
    drop(store);
    let reopened = EmbedStore::open(dir.path()).unwrap();
    assert_eq!(reopened.len(), 3);
}

// ---------------------------------------------------------------------------
// Leiden
// ---------------------------------------------------------------------------

#[test]
fn leiden_finds_at_least_one_community_on_two_cliques() {
    // Two 4-cliques joined by a single weak bridge.
    let mut g: UnGraph<NodeId, f32> = UnGraph::new_undirected();
    let nodes: Vec<_> = (0..8).map(|i| g.add_node(NodeId::new(i as u128))).collect();
    // Clique A: 0-3
    for i in 0..4 {
        for j in (i + 1)..4 {
            g.add_edge(nodes[i], nodes[j], 1.0);
        }
    }
    // Clique B: 4-7
    for i in 4..8 {
        for j in (i + 1)..8 {
            g.add_edge(nodes[i], nodes[j], 1.0);
        }
    }
    // Bridge.
    g.add_edge(nodes[3], nodes[4], 0.05);

    let solver = LeidenSolver::new(LeidenConfig::default());
    let comms = solver.run(&g).unwrap();
    assert!(!comms.is_empty(), "must produce at least one community");
    // We expect the algorithm to discover 2 well-separated groups, but the
    // hard contract is just ≥1.
    let total_members: usize = comms.iter().map(|c| c.members.len()).sum();
    assert_eq!(total_members, 8, "every node must belong to one community");
    for c in &comms {
        assert!(c.cohesion >= 0.0 && c.cohesion <= 1.0);
    }
}

#[test]
fn leiden_is_deterministic_with_default_seed() {
    let mut g: UnGraph<NodeId, f32> = UnGraph::new_undirected();
    let nodes: Vec<_> = (0..6).map(|i| g.add_node(NodeId::new(i as u128))).collect();
    for i in 0..3 {
        for j in (i + 1)..3 {
            g.add_edge(nodes[i], nodes[j], 1.0);
        }
    }
    for i in 3..6 {
        for j in (i + 1)..6 {
            g.add_edge(nodes[i], nodes[j], 1.0);
        }
    }
    g.add_edge(nodes[2], nodes[3], 0.1);

    let solver = LeidenSolver::new(LeidenConfig::default());
    let a = solver.run(&g).unwrap();
    let b = solver.run(&g).unwrap();
    assert_eq!(a.len(), b.len());
    for (ca, cb) in a.iter().zip(b.iter()) {
        assert_eq!(ca.id, cb.id);
        assert_eq!(ca.members, cb.members);
    }
}

// ---------------------------------------------------------------------------
// Cluster runner (split policy)
// ---------------------------------------------------------------------------

#[test]
fn cluster_runner_handles_empty_input() {
    let runner = ClusterRunner::new(ClusterRunnerConfig::default());
    let out = runner.run(&[]).unwrap();
    assert!(out.is_empty());
}

#[test]
fn cluster_runner_runs_on_two_cliques() {
    let mut edges: Vec<(NodeId, NodeId, f32)> = Vec::new();
    for i in 0..4u128 {
        for j in (i + 1)..4 {
            edges.push((NodeId::new(i), NodeId::new(j), 1.0));
        }
    }
    for i in 4..8u128 {
        for j in (i + 1)..8 {
            edges.push((NodeId::new(i), NodeId::new(j), 1.0));
        }
    }
    edges.push((NodeId::new(3), NodeId::new(4), 0.05));

    let runner = ClusterRunner::new(ClusterRunnerConfig::default());
    let comms = runner.run(&edges).unwrap();
    assert!(!comms.is_empty());
    let total: usize = comms.iter().map(|c| c.members.len()).sum();
    assert_eq!(total, 8);
}

// ---------------------------------------------------------------------------
// Concept extractor
// ---------------------------------------------------------------------------

#[test]
fn concept_extraction_picks_up_function_names_and_headings() {
    let text = r#"
# Loader Module
Loads embeddings from disk.

/// Loads the BGE small model.
fn load_model_from_disk(path: &Path) -> Result<()> {
    Ok(())
}
"#;
    let extractor = ConceptExtractor::new();
    let concepts = extractor
        .extract(ExtractInput {
            kind: "code",
            text,
        })
        .unwrap();

    assert!(!concepts.is_empty(), "expected at least one concept");

    // Heading should be present.
    let has_heading = concepts
        .iter()
        .any(|c| c.source == ConceptSource::Heading && c.term.contains("loader"));
    assert!(has_heading, "expected the heading to be extracted: {concepts:?}");

    // Identifier-derived concept should include "load" or "model".
    let has_ident = concepts.iter().any(|c| {
        c.source == ConceptSource::Identifier
            && (c.term.contains("load") || c.term.contains("model"))
    });
    assert!(has_ident, "expected identifier concepts: {concepts:?}");
}

#[test]
fn concept_extraction_handles_pure_markdown() {
    let text = r#"
# Mneme Design
## Embedding Pipeline
The pipeline produces 384-dim vectors.
"#;
    let extractor = ConceptExtractor::new();
    let concepts = extractor
        .extract(ExtractInput {
            kind: "markdown",
            text,
        })
        .unwrap();
    assert!(concepts.iter().any(|c| c.source == ConceptSource::Heading));
}

// ---------------------------------------------------------------------------
// Summarizer
// ---------------------------------------------------------------------------

#[test]
fn summarizer_uses_doc_comment_when_present() {
    let s = Summarizer::new();
    let body = "/// Compute the SHA-256 of a slice.\nfn sha(x: &[u8]) -> [u8; 32] { todo!() }";
    let out = s
        .summarize_function("fn sha(x: &[u8]) -> [u8; 32]", body)
        .unwrap();
    assert!(out.to_ascii_lowercase().contains("sha"), "got: {out}");
}

#[test]
fn summarizer_falls_back_to_signature() {
    let s = Summarizer::new();
    let out = s
        .summarize_function("fn foo(a: i32, b: i32, c: i32)", "{ a + b + c }")
        .unwrap();
    assert!(out.contains("foo"));
    assert!(out.contains("3"));
}
