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
    // BUG-A2-024: heading extraction is now gated on the `kind` being
    // an explicit markdown flavour. The previous heuristic
    // (`text.contains("\n#")`) produced false-positive headings on
    // Rust attribute syntax (`#[derive(...)]`). The corpus below
    // mixes a markdown heading with a Rust function declaration —
    // we pass `kind: "markdown"` to keep the heading-extraction
    // behaviour for the documentation tooling, where it belongs.
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
            kind: "markdown",
            text,
        })
        .unwrap();

    assert!(!concepts.is_empty(), "expected at least one concept");

    // Heading should be present.
    let has_heading = concepts
        .iter()
        .any(|c| c.source == ConceptSource::Heading && c.term.contains("loader"));
    assert!(
        has_heading,
        "expected the heading to be extracted: {concepts:?}"
    );

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

// ---------------------------------------------------------------------------
// Call-edge resolver — Phase A bench Q3 fix
// ---------------------------------------------------------------------------
//
// `parsers/src/extractor.rs::collect_calls` emits call edges with a
// placeholder `target_qualified` like `call::a.rs::helper` — the
// brain-side resolver translates those into the matching Function
// `n_<hash>` id from the indexed graph so call_graph / find_references
// in the *callers* direction return real edges.

mod call_resolver_tests {
    use crate::call_resolver::{
        build_function_index, extract_callee_name, parse_call_placeholder, resolve_callee,
        IndexedFunction,
    };
    use std::collections::HashMap;

    #[test]
    fn parse_placeholder_basic() {
        let p = parse_call_placeholder("call::src/lib.rs::helper").expect("parse");
        assert_eq!(p.file_path, "src/lib.rs");
        assert_eq!(p.callee_text, "helper");
    }

    #[test]
    fn parse_placeholder_keeps_path_in_callee() {
        // `crate::foo::bar` callee text contains its own `::`. Must
        // survive splitn(2) so the path-suffix logic in
        // `extract_callee_name` can pick the rightmost segment.
        let p = parse_call_placeholder("call::lib.rs::crate::foo::bar").expect("parse");
        assert_eq!(p.file_path, "lib.rs");
        assert_eq!(p.callee_text, "crate::foo::bar");
    }

    #[test]
    fn parse_placeholder_rejects_non_call() {
        assert!(parse_call_placeholder("n_abc1234567890def").is_none());
        assert!(parse_call_placeholder("import::x::y").is_none());
        assert!(parse_call_placeholder("call::nofile").is_none());
        assert!(parse_call_placeholder("call::").is_none());
        assert!(parse_call_placeholder("").is_none());
    }

    #[test]
    fn extract_callee_name_handles_method_and_path() {
        assert_eq!(extract_callee_name("helper"), "helper");
        assert_eq!(extract_callee_name("b.put"), "put");
        assert_eq!(extract_callee_name("self.method"), "method");
        assert_eq!(extract_callee_name("crate::foo::bar"), "bar");
        assert_eq!(extract_callee_name("Foo::new"), "new");
        // Built-in macro / bare ident
        assert_eq!(extract_callee_name("vec"), "vec");
        assert_eq!(extract_callee_name("println"), "println");
        // Whitespace tolerance
        assert_eq!(extract_callee_name("  trimmed  "), "trimmed");
    }

    #[test]
    fn resolve_callee_finds_unique_function() {
        // Synthetic input mirrors what the parser emits + what the
        // ingest layer indexes:
        //   parser-emitted edge:
        //     {from: "n_caller", to: "call::a.rs::helper", kind: "calls"}
        //   nodes table has:
        //     {qualified_name: "n_helper", name: "helper", file: "a.rs", kind: "function"}
        // After resolve, the edge's target should be "n_helper".
        let by_name = build_function_index([
            ("helper", "n_helper", "a.rs"),
            ("caller", "n_caller", "a.rs"),
        ]);
        let got = resolve_callee("helper", Some("a.rs"), &by_name);
        assert_eq!(got.as_deref(), Some("n_helper"));
    }

    #[test]
    fn resolve_callee_prefers_same_file_on_collision() {
        // Two functions named `put` in different files → same-file
        // hint MUST win so the resolver doesn't randomly cross-link.
        let by_name = build_function_index([
            ("put", "n_put_bag_rs", "bag.rs"),
            ("put", "n_put_box_rs", "box.rs"),
        ]);
        let got = resolve_callee("b.put", Some("box.rs"), &by_name);
        assert_eq!(got.as_deref(), Some("n_put_box_rs"));
        let got2 = resolve_callee("b.put", Some("bag.rs"), &by_name);
        assert_eq!(got2.as_deref(), Some("n_put_bag_rs"));
    }

    #[test]
    fn resolve_callee_falls_back_to_first_when_no_same_file() {
        let by_name = build_function_index([
            ("put", "n_put_bag_rs", "bag.rs"),
            ("put", "n_put_box_rs", "box.rs"),
        ]);
        let got = resolve_callee("b.put", Some("unrelated.rs"), &by_name).expect("resolve");
        // First-insert order wins. Both are valid pointers — caller
        // logged a deterministic best-effort link.
        assert!(got == "n_put_bag_rs" || got == "n_put_box_rs");
    }

    #[test]
    fn resolve_callee_returns_none_for_external() {
        // `vec!`, `println!`, external crate functions — none in the
        // index → resolver drops them, caller leaves edge alone.
        let by_name = build_function_index([("helper", "n_helper", "a.rs")]);
        assert!(resolve_callee("vec", None, &by_name).is_none());
        assert!(resolve_callee("HashMap::new", None, &by_name).is_none());
        assert!(resolve_callee("std::process::exit", None, &by_name).is_none());
    }

    #[test]
    fn resolve_callee_returns_none_for_empty_callee() {
        let by_name: HashMap<String, Vec<IndexedFunction>> = HashMap::new();
        assert!(resolve_callee("", None, &by_name).is_none());
        assert!(resolve_callee("   ", None, &by_name).is_none());
    }

    #[test]
    fn build_function_index_skips_anonymous() {
        // Arrow fn / closure with no binding → empty `name`. Indexing
        // it would let the resolver match every empty callee text to
        // an arbitrary closure — useless and dangerous.
        let by_name =
            build_function_index([("", "n_anon_closure", "a.rs"), ("named", "n_named", "a.rs")]);
        assert!(by_name.get("").is_none());
        assert_eq!(by_name.get("named").map(|v| v.len()), Some(1));
    }

    #[test]
    fn end_to_end_resolve_via_placeholder() {
        // Walks the full path the orchestration layer takes:
        //   1. Parse the `call::*` placeholder
        //   2. Reduce callee_text → bare identifier
        //   3. Resolve against the function index
        //   4. Result is the n_<hash> the UPDATE writes back
        let by_name = build_function_index([
            ("build_or_migrate", "n_bom_builder", "store/src/builder.rs"),
            ("helper", "n_helper", "lib.rs"),
        ]);

        let placeholder = "call::lib.rs::build_or_migrate";
        let p = parse_call_placeholder(placeholder).expect("parse");
        let resolved = resolve_callee(p.callee_text, Some(p.file_path), &by_name);
        assert_eq!(resolved.as_deref(), Some("n_bom_builder"));

        // Method-call form: `caller_struct.helper()` → resolves to
        // `helper` even though the placeholder text is `c.helper`.
        let p2 = parse_call_placeholder("call::lib.rs::c.helper").expect("parse");
        let resolved2 = resolve_callee(p2.callee_text, Some(p2.file_path), &by_name);
        assert_eq!(resolved2.as_deref(), Some("n_helper"));
    }
}

// ---------------------------------------------------------------------------
// ConceptStore -- persistence tests (Wave 3.3)
// ---------------------------------------------------------------------------

mod concept_store_tests {
    use crate::concept_store::{stable_id, ConceptStore, StoredConcept};
    use std::sync::Arc;
    use std::thread;

    fn temp_store() -> (tempfile::TempDir, ConceptStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ConceptStore::new(dir.path().join("concepts.db").as_path())
            .expect("open concept store");
        (dir, store)
    }

    fn make_concept(project: &str, name: &str) -> StoredConcept {
        StoredConcept::new(project, name, "pattern")
            .with_score(0.7)
            .with_description(format!("test concept: {name}"))
    }

    // Test 1: upsert persists across reopen
    #[test]
    fn concept_store_upsert_persists_across_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("concepts.db");

        {
            let store = ConceptStore::new(&db_path).expect("open store");
            let c = make_concept("proj-abc", "cache invalidation");
            store.upsert(&c).expect("upsert");
        }

        {
            let store = ConceptStore::new(&db_path).expect("reopen store");
            let got = store
                .get("proj-abc", "cache invalidation")
                .expect("get")
                .expect("should be present after reopen");

            assert_eq!(got.name, "cache invalidation");
            assert_eq!(got.kind, "pattern");
            assert!(
                (got.score - 0.7).abs() < 1e-9,
                "score should be 0.7 but was {}",
                got.score
            );
            assert_eq!(
                got.description.as_deref(),
                Some("test concept: cache invalidation")
            );
        }
    }

    // Test 2: list_for_project returns all concepts for that project
    #[test]
    fn concept_store_list_for_project_returns_all() {
        let (_dir, store) = temp_store();

        let names = ["ownership", "borrowing", "lifetimes", "traits", "closures"];
        for name in &names {
            store
                .upsert(&make_concept("proj-rust", name))
                .expect("upsert");
        }
        store
            .upsert(&make_concept("proj-other", "unrelated"))
            .expect("upsert other project");

        let listed = store
            .list_for_project("proj-rust", 100)
            .expect("list_for_project");

        assert_eq!(
            listed.len(),
            names.len(),
            "should list exactly the 5 concepts for proj-rust, got: {listed:?}"
        );
        for c in &listed {
            assert_eq!(c.project_id, "proj-rust");
        }
    }

    // Test 3: boost increments use_count
    #[test]
    fn concept_store_boost_increments_use_count() {
        let (_dir, store) = temp_store();
        let c = make_concept("proj-boost", "zero-copy");
        store.upsert(&c).expect("upsert");

        let before = store
            .get("proj-boost", "zero-copy")
            .expect("get before boost")
            .expect("present");

        store.boost("proj-boost", "zero-copy").expect("boost");

        let after = store
            .get("proj-boost", "zero-copy")
            .expect("get after boost")
            .expect("present");

        assert_eq!(
            after.use_count,
            before.use_count + 1,
            "boost must increment use_count by exactly 1"
        );
        assert!(
            after.last_used >= before.last_used,
            "last_used must be >= previous value after boost"
        );
    }

    // Test 4: decay_stale lowers scores for old concepts
    #[test]
    fn concept_store_decay_stale_lowers_old_scores() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("concepts.db");
        let store = ConceptStore::new(&db_path).expect("open store");

        let forty_days_ago = crate::concept_store::unix_now() - 40 * 86_400;
        let id = stable_id("proj-decay", "ancient pattern");
        store
            .conn
            .lock()
            .expect("lock")
            .execute(
                "INSERT INTO concepts
                     (id, project_id, name, kind, score, first_seen, last_used, use_count)
                 VALUES (?1, 'proj-decay', 'ancient pattern', 'pattern', 0.8, ?2, ?2, 1)",
                rusqlite::params![id, forty_days_ago],
            )
            .expect("direct insert for test setup");

        let fresh = make_concept("proj-decay", "fresh pattern");
        store.upsert(&fresh).expect("upsert fresh");

        let changed = store.decay_stale(30).expect("decay_stale");
        assert_eq!(changed, 1, "exactly one concept should be decayed");

        let ancient = store
            .get("proj-decay", "ancient pattern")
            .expect("get")
            .expect("present");
        assert!(
            ancient.score < 0.8,
            "score must decrease after decay: got {}",
            ancient.score
        );
        assert!(
            (ancient.score - 0.8 * 0.95).abs() < 1e-9,
            "score must be 0.8 * 0.95 = 0.76, got {}",
            ancient.score
        );

        let fresh_after = store
            .get("proj-decay", "fresh pattern")
            .expect("get")
            .expect("present");
        assert!(
            (fresh_after.score - 0.7).abs() < 1e-9,
            "fresh concept score must be unchanged: {}",
            fresh_after.score
        );
    }

    // Test 5: concurrent writes via WAL do not corrupt data
    #[test]
    fn concept_store_concurrent_writes_via_wal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("concepts.db");

        let store = Arc::new(ConceptStore::new(&db_path).expect("open store"));
        let thread_count = 4usize;
        let concepts_per_thread = 25usize;

        let handles: Vec<_> = (0..thread_count)
            .map(|t| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    for i in 0..concepts_per_thread {
                        let name = format!("concept-t{t}-{i}");
                        let c = StoredConcept::new("proj-concurrent", &name, "convention")
                            .with_score(0.6);
                        store.upsert(&c).expect("concurrent upsert must not fail");
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread must not panic");
        }

        let total = store
            .list_for_project("proj-concurrent", 1_000)
            .expect("list after concurrent writes");

        assert_eq!(
            total.len(),
            thread_count * concepts_per_thread,
            "all {} concepts must be persisted after concurrent writes, got {}",
            thread_count * concepts_per_thread,
            total.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Smart questions — Wave 3.2
// ---------------------------------------------------------------------------

mod smart_questions_tests {
    use crate::smart_questions::{generate_questions, GraphEdge, GraphNode, QuestionKind};

    fn make_node(name: &str, kind: &str, ls: Option<i64>, le: Option<i64>) -> GraphNode {
        GraphNode {
            qualified_name: name.to_owned(),
            name: name.split("::").last().unwrap_or(name).to_owned(),
            kind: kind.to_owned(),
            file_path: Some(format!("src/{name}.rs")),
            line_start: ls,
            line_end: le,
        }
    }

    fn make_edge(src: &str, tgt: &str) -> GraphEdge {
        GraphEdge {
            source: src.to_owned(),
            target: tgt.to_owned(),
            kind: "calls".to_owned(),
        }
    }

    #[test]
    fn smart_questions_returns_top_n_by_centrality() {
        // Star topology: `hub` is called by 5 peripherals.
        // Hub must appear first; only `limit` questions returned.
        let nodes = vec![
            make_node("hub", "function", Some(1), Some(50)),
            make_node("a", "function", Some(1), Some(5)),
            make_node("b", "function", Some(1), Some(5)),
            make_node("c", "function", Some(1), Some(5)),
            make_node("d", "function", Some(1), Some(5)),
            make_node("e", "function", Some(1), Some(5)),
        ];
        let edges = vec![
            make_edge("a", "hub"),
            make_edge("b", "hub"),
            make_edge("c", "hub"),
            make_edge("d", "hub"),
            make_edge("e", "hub"),
        ];

        let results = generate_questions(&nodes, &edges, 3, QuestionKind::All);

        assert!(
            results.len() <= 3,
            "must not exceed limit={}, got {}",
            3,
            results.len()
        );
        assert!(
            !results.is_empty(),
            "non-trivial graph must produce at least one question"
        );

        let top = &results[0];
        assert!(
            top.related_nodes.iter().any(|n| n == "hub"),
            "top question must reference hub; got: {:?}",
            top.related_nodes
        );

        for i in 1..results.len() {
            assert!(
                results[i - 1].score >= results[i].score,
                "scores must be non-increasing: [{}]={} > [{}]={}",
                i - 1,
                results[i - 1].score,
                i,
                results[i].score
            );
        }
    }

    #[test]
    fn smart_questions_includes_anomaly_questions_for_god_nodes() {
        // Star with 20 callers: `god` exceeds the 95th-percentile threshold.
        let mut nodes = vec![make_node("god", "function", Some(1), Some(30))];
        let mut edges = Vec::new();
        for i in 0..20usize {
            let name = format!("caller_{i}");
            nodes.push(make_node(&name, "function", Some(1), Some(5)));
            edges.push(make_edge(&name, "god"));
        }
        edges.push(make_edge("caller_0", "caller_1"));

        let results = generate_questions(&nodes, &edges, 5, QuestionKind::Anomaly);

        assert!(
            !results.is_empty(),
            "anomaly pass must surface at least one question"
        );
        let mentions_god = results
            .iter()
            .any(|q| q.related_nodes.iter().any(|n| n == "god"));
        assert!(
            mentions_god,
            "expected a question about the god node; got: {results:#?}"
        );

        let god_q = results
            .iter()
            .find(|q| q.related_nodes.iter().any(|n| n == "god"))
            .unwrap();
        assert!(
            god_q.score > 0.25,
            "god node question score must be > 0.25; got {}",
            god_q.score
        );
    }

    #[test]
    fn smart_questions_handles_empty_graph() {
        let results = generate_questions(&[], &[], 10, QuestionKind::All);
        assert!(
            results.is_empty(),
            "empty graph must return empty vec; got: {results:?}"
        );

        for kind in [
            QuestionKind::Starter,
            QuestionKind::DeepDive,
            QuestionKind::Anomaly,
        ] {
            let r = generate_questions(&[], &[], 10, kind);
            assert!(
                r.is_empty(),
                "empty graph must return empty vec for {kind:?}"
            );
        }
    }
}
