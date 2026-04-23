//! Unit tests covering the contract spelled out in §21.3 / §25.10:
//! - Function extraction works in TS, Python, and Rust.
//! - Incremental re-parse reuses the cached tree.
//! - ERROR / MISSING are captured *and* the graph is still built.
//! - The extractor degrades to `Confidence::Ambiguous` on syntax issues.

use crate::{
    extractor::Extractor,
    incremental::IncrementalParser,
    job::{Confidence, NodeKind},
    language::Language,
    parser_pool::ParserPool,
    query_cache::{self, QueryKind},
};
use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pool() -> Arc<ParserPool> {
    Arc::new(ParserPool::new(2).expect("parser pool"))
}

async fn parse_once(
    inc: &IncrementalParser,
    path: &str,
    lang: Language,
    src: &str,
) -> tree_sitter::Tree {
    let bytes = Arc::new(src.as_bytes().to_vec());
    inc.parse_file(&PathBuf::from(path), lang, bytes)
        .await
        .expect("parse")
        .tree
}

// ---------------------------------------------------------------------------
// Language → grammar wiring
// ---------------------------------------------------------------------------

#[test]
fn from_extension_known_cases() {
    assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
    assert_eq!(Language::from_extension(".py"), Some(Language::Python));
    assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    assert_eq!(Language::from_extension("zzz"), None);
}

#[test]
fn from_filename_special_cases() {
    assert_eq!(
        Language::from_filename(&PathBuf::from("Cargo.toml")),
        Some(Language::Toml)
    );
    assert_eq!(
        Language::from_filename(&PathBuf::from("/tmp/Dockerfile")),
        Some(Language::Bash)
    );
    assert_eq!(
        Language::from_filename(&PathBuf::from("foo.rs")),
        Some(Language::Rust)
    );
}

#[test]
fn tier1_languages_all_enabled() {
    for lang in Language::ALL {
        if lang.is_tier_one() {
            assert!(
                lang.is_enabled(),
                "{lang} is Tier 1 but not enabled in this build"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Query cache
// ---------------------------------------------------------------------------

#[test]
fn query_cache_warms_for_every_enabled_language() {
    query_cache::warm_up().expect("warm");
    // Hot-path lookup is cheap & infallible after warm-up.
    for lang in Language::ALL {
        if !lang.is_enabled() {
            continue;
        }
        let _ = query_cache::get_query(*lang, QueryKind::Errors).expect("errors query");
    }
}

#[test]
fn errors_query_compiles_for_rust() {
    let q = query_cache::get_query(Language::Rust, QueryKind::Errors).unwrap();
    // The query has at least one capture (either ERROR or MISSING).
    assert!(q.capture_names().len() >= 1);
}

// ---------------------------------------------------------------------------
// Function extraction — TS / Python / Rust (the core "did it work?" test)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extracts_typescript_functions() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    let src = r#"
        export function alpha(x: number): number { return x + 1; }
        export const beta = (y: number) => y * 2;
        class Foo { method bar() { return 1; } }
    "#;
    let tree = parse_once(&inc, "alpha.ts", Language::TypeScript, src).await;
    let extractor = Extractor::new(Language::TypeScript);
    let g = extractor
        .extract(&tree, src.as_bytes(), &PathBuf::from("alpha.ts"))
        .expect("extract");

    let fns: Vec<_> = g
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        fns.len() >= 2,
        "expected at least 2 functions, got {} ({:?})",
        fns.len(),
        fns
    );
    assert!(fns.iter().any(|n| n.name == "alpha"));
}

#[tokio::test]
async fn extracts_python_functions_and_classes() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    let src = "
class Animal:
    def speak(self):
        return 'noise'

class Dog(Animal):
    def bark(self):
        return 'woof'

def top_level():
    return 42
";
    let tree = parse_once(&inc, "zoo.py", Language::Python, src).await;
    let extractor = Extractor::new(Language::Python);
    let g = extractor
        .extract(&tree, src.as_bytes(), &PathBuf::from("zoo.py"))
        .expect("extract");

    let fns: Vec<_> = g
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    let classes: Vec<_> = g
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();

    assert!(fns.iter().any(|n| n.name == "top_level"));
    assert!(fns.iter().any(|n| n.name == "speak"));
    assert!(classes.iter().any(|n| n.name == "Animal"));
    assert!(classes.iter().any(|n| n.name == "Dog"));

    // Inheritance edge present.
    assert!(g
        .edges
        .iter()
        .any(|e| matches!(e.kind, crate::job::EdgeKind::Inherits)));
}

#[tokio::test]
async fn extracts_rust_functions() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    let src = r#"
        pub fn add(a: i32, b: i32) -> i32 { a + b }
        struct Counter { n: i32 }
        impl Counter {
            pub fn bump(&mut self) -> i32 { self.n += 1; self.n }
        }
    "#;
    let tree = parse_once(&inc, "lib.rs", Language::Rust, src).await;
    let extractor = Extractor::new(Language::Rust);
    let g = extractor
        .extract(&tree, src.as_bytes(), &PathBuf::from("lib.rs"))
        .expect("extract");

    let fns: Vec<_> = g
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(fns.iter().any(|n| n.name == "add"));
    assert!(fns.iter().any(|n| n.name == "bump"));

    // No syntax issues → high confidence everywhere.
    assert!(!g.has_syntax_issues());
    assert!(g.nodes.iter().all(|n| !matches!(
        n.confidence,
        Confidence::Ambiguous
    ) || n.kind == NodeKind::File));
}

// ---------------------------------------------------------------------------
// Incremental re-parse (bytes unchanged → reuse; bytes changed → reuse old tree)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn incremental_reuses_tree_on_unchanged_bytes() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    let path = PathBuf::from("inc.rs");
    let src = "fn x() -> i32 { 1 }";
    let bytes = Arc::new(src.as_bytes().to_vec());

    let first = inc
        .parse_file(&path, Language::Rust, bytes.clone())
        .await
        .unwrap();
    assert!(!first.unchanged, "first parse must not be marked unchanged");

    let second = inc
        .parse_file(&path, Language::Rust, bytes.clone())
        .await
        .unwrap();
    assert!(
        second.unchanged,
        "byte-identical second parse must hit the short-circuit"
    );
    assert!(second.incremental);
}

#[tokio::test]
async fn incremental_reparses_on_byte_change() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    let path = PathBuf::from("ed.rs");

    let v1 = Arc::new(b"fn a() -> i32 { 1 }".to_vec());
    let v2 = Arc::new(b"fn a() -> i32 { 1 + 2 }".to_vec());

    let _ = inc.parse_file(&path, Language::Rust, v1).await.unwrap();
    let r2 = inc.parse_file(&path, Language::Rust, v2).await.unwrap();
    assert!(!r2.unchanged);
    assert!(r2.incremental, "second parse must be on the incremental path");
}

#[tokio::test]
async fn incremental_cache_lru_capacity_evicts() {
    let pool = pool();
    let inc = IncrementalParser::with_capacity(pool, 2);
    for i in 0..5 {
        let path = PathBuf::from(format!("f{i}.rs"));
        let src = format!("fn f{i}() {{ {i} }}");
        let bytes = Arc::new(src.into_bytes());
        let _ = inc
            .parse_file(&path, Language::Rust, bytes)
            .await
            .unwrap();
    }
    assert_eq!(inc.cached_count(), 2, "LRU should cap at capacity");
}

// ---------------------------------------------------------------------------
// Error recovery — design §25.10
// ---------------------------------------------------------------------------

#[tokio::test]
async fn syntax_errors_captured_but_graph_built() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    // Deliberately broken: missing brace + dangling token.
    let src = "fn broken( { let x = ; }";
    let tree = parse_once(&inc, "bad.rs", Language::Rust, src).await;
    let extractor = Extractor::new(Language::Rust);
    let g = extractor
        .extract(&tree, src.as_bytes(), &PathBuf::from("bad.rs"))
        .expect("extract should not fail on malformed input");

    // Errors recorded.
    assert!(
        g.has_syntax_issues(),
        "expected ERROR/MISSING in {:?}",
        g.issues
    );

    // Graph still emitted (file node at minimum).
    assert!(
        g.nodes.iter().any(|n| n.kind == NodeKind::File),
        "file node always present"
    );

    // Confidence demoted to AMBIGUOUS on non-file nodes that were extracted.
    for n in g.nodes.iter().filter(|n| n.kind != NodeKind::File) {
        assert_eq!(
            n.confidence,
            Confidence::Ambiguous,
            "extracted nodes should be AMBIGUOUS when ERRORs present"
        );
    }
}

#[tokio::test]
async fn python_decorators_captured() {
    let pool = pool();
    let inc = IncrementalParser::new(pool);
    let src = "
@decorator
def func():
    pass
";
    let tree = parse_once(&inc, "deco.py", Language::Python, src).await;
    let extractor = Extractor::new(Language::Python);
    let g = extractor
        .extract(&tree, src.as_bytes(), &PathBuf::from("deco.py"))
        .expect("extract");
    assert!(
        g.nodes.iter().any(|n| n.kind == NodeKind::Decorator),
        "decorator should be captured"
    );
}

// ---------------------------------------------------------------------------
// ParserPool — concurrent leases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn parser_pool_serves_two_leases_in_parallel() {
    let pool = ParserPool::new(2).unwrap();
    let l1 = pool.acquire(Language::Rust).await.unwrap();
    let l2 = pool.acquire(Language::Rust).await.unwrap();
    assert_ne!(l1.slot(), l2.slot(), "should hand out distinct slots");
    drop(l1);
    drop(l2);
}

#[tokio::test]
async fn parser_pool_rejects_disabled_language_cleanly() {
    // We test the negative path by building a pool then querying for a
    // language not in `Language::ALL` is impossible — instead query for a
    // disabled-by-feature one. If the build has Tier-2 features off, Vue
    // should be missing; otherwise this test is a no-op assertion.
    let pool = ParserPool::new(1).unwrap();
    if !Language::Vue.is_enabled() {
        let err = pool.acquire(Language::Vue).await.unwrap_err();
        assert!(
            matches!(err, crate::ParserError::NoParserForLanguage(_)),
            "expected NoParserForLanguage, got {err:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// JSON contract — ParseJob round-trips
// ---------------------------------------------------------------------------

#[test]
fn confidence_serializes_as_kebab_case() {
    let j = serde_json::to_string(&Confidence::Extracted).unwrap();
    assert_eq!(j, "\"extracted\"");
    let j = serde_json::to_string(&Confidence::Ambiguous).unwrap();
    assert_eq!(j, "\"ambiguous\"");
}
