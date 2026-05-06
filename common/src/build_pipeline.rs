//! `BuildPipeline` trait — thin abstraction over the 20+ build passes run by
//! `mneme build`.
//!
//! # Why this exists
//!
//! `run_inline` in `cli/src/commands/build.rs` grew to contain all pass
//! invocations in a flat sequence.  Extracting each pass into a named method
//! on a trait lets:
//!
//! * **Test code** supply a lightweight mock that records *which* passes ran
//!   and *in what order* without needing a real SQLite shard or embedding
//!   model on disk.
//! * **Future dispatch paths** substitute an alternative `BuildPipeline` impl
//!   that routes passes to the supervisor worker-pool, a remote node, or a
//!   replay fixture — without touching `run_inline`.
//!
//! # Design constraints
//!
//! * This module lives in `mneme-common` so both `mneme-cli` and test crates
//!   can depend on it without creating a circular dependency.
//! * It must NOT import `store`, `brain`, `parsers`, or `multimodal` — those
//!   heavy crates depend on `mneme-common`, not the other way around.
//! * Consequently `BuildContext` carries only the small, universally-available
//!   inputs (project id, project path, path-manager, plus the parse-loop
//!   scalars that the perf / errors passes need).  Pass-specific state that
//!   requires CLI-only types (`Store`, `Heartbeat`, `BuildChildRegistry`,
//!   `DefaultLearner`) is stored inside the concrete `DefaultBuildPipeline`
//!   struct in `mneme-cli`, not here.
//!
//! # Async trait stability
//!
//! Rust 1.75 (RPITIT) stabilised `async fn` in traits.  The workspace
//! `rust-version` is 1.78 so no `async-trait` proc-macro is needed.  Note
//! that these methods are NOT object-safe — use generic bounds
//! (`where P: BuildPipeline`) rather than `dyn BuildPipeline`.

use std::path::PathBuf;
use std::time::Instant;

use crate::ids::ProjectId;
use crate::paths::PathManager;

// ── BuildContext ──────────────────────────────────────────────────────────────

/// Inputs shared by all build passes.
///
/// `BuildContext` is constructed once in `run_inline` just before the first
/// pass call and passed by shared reference to every pass.  It is intentionally
/// cheap to construct — no allocations beyond the two paths and the error vec.
///
/// Pass-specific state that is mutable across passes (e.g. the accumulated
/// `IntentStats` or the `DefaultLearner` observation table) lives inside the
/// `DefaultBuildPipeline` implementation in `mneme-cli`.
#[derive(Debug)]
pub struct BuildContext {
    /// Stable identifier for the project shard (hash of the canonical path).
    pub project_id: ProjectId,

    /// Absolute path to the project root on disk.  This is the same path the
    /// user passed to `mneme build`.
    pub project: PathBuf,

    /// Root-level path manager.  Controls where shards, models, and runtime
    /// files live (`~/.mneme/` by default, overrideable via `MNEME_HOME`).
    pub paths: PathManager,

    /// Wall-clock instant at which the inline pipeline started.  Used by the
    /// perf-baselines pass to compute `build.duration_ms`.
    pub started_at: Instant,

    /// Number of source files successfully indexed in the parse loop.  Passed
    /// through to the perf-baselines pass.
    pub indexed: usize,

    /// Total graph nodes written in the parse loop.  Perf-baselines input.
    pub node_total: u64,

    /// Total graph edges written in the parse loop.  Perf-baselines input.
    pub edge_total: u64,

    /// `(message, file_path)` pairs for parse/extract failures collected
    /// during the parse loop.  Consumed by the errors pass to persist
    /// deduplicated rows into `errors.db`.
    pub build_errors: Vec<(String, String)>,

    /// Set to `true` when `mneme build --inline` was passed, or when the
    /// caller has already confirmed that no supervisor connection should be
    /// used.  The audit pass uses this to pick the direct-subprocess path
    /// instead of dialling the supervisor.
    pub inline_mode: bool,
}

impl BuildContext {
    /// Construct a `BuildContext` with parse-loop scalars defaulted to zero.
    ///
    /// Callers should mutate `indexed`, `node_total`, `edge_total`, and
    /// `build_errors` after the parse loop completes and before the first
    /// pass is invoked.
    pub fn new(
        project_id: ProjectId,
        project: PathBuf,
        paths: PathManager,
        inline_mode: bool,
    ) -> Self {
        Self {
            project_id,
            project,
            paths,
            started_at: Instant::now(),
            indexed: 0,
            node_total: 0,
            edge_total: 0,
            build_errors: Vec::new(),
            inline_mode,
        }
    }
}

// ── BuildPipeline trait ───────────────────────────────────────────────────────

/// Sequenced build passes for `mneme build --inline`.
///
/// Each method corresponds to one named pass in the original `run_inline`
/// function.  All methods take a shared `&BuildContext` plus `&mut self` so
/// implementations can accumulate per-pass statistics in their own fields.
///
/// ## Ordering contract
///
/// The orchestrator (`run_inline`) MUST call the methods in the order they
/// appear below.  Some passes produce data that later passes consume:
///
/// * `resolve_imports_pass` and `resolve_calls_pass` must run **before**
///   `leiden_pass` so community detection clusters by resolved connectivity.
/// * `leiden_pass` must run **before** `embedding_pass` (community ids anchor
///   embedding slots) and `wiki_pass` (wiki pages are built per community).
/// * All analytics passes (leiden → embedding → betweenness → intent →
///   architecture → conventions → wiki → federated) must run **after**
///   the parse loop has committed graph data.
/// * `perf_pass`, `errors_pass`, `livestate_pass`, `agents_pass`, and
///   `seed_populate_pass` are the final bookkeeping passes; their order among
///   themselves is not load-bearing but should stay stable.
///
/// ## Mock usage
///
/// ```rust
/// use common::build_pipeline::{BuildContext, BuildPipeline};
/// use common::ids::ProjectId;
/// use common::paths::PathManager;
/// use std::path::PathBuf;
///
/// struct RecordingPipeline {
///     pub calls: Vec<&'static str>,
/// }
///
/// impl BuildPipeline for RecordingPipeline {
///     async fn multimodal_pass(&mut self, _ctx: &BuildContext) {
///         self.calls.push("multimodal");
///     }
///     async fn resolve_imports_pass(&mut self, _ctx: &BuildContext) {
///         self.calls.push("resolve_imports");
///     }
///     // … (all remaining methods push their own label) …
/// #   async fn resolve_calls_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn leiden_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn embedding_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn audit_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn tests_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn git_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn deps_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn betweenness_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn intent_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn architecture_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn conventions_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn wiki_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn federated_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn perf_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn errors_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn livestate_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn agents_pass(&mut self, _ctx: &BuildContext) {}
/// #   async fn seed_populate_pass(&mut self, _ctx: &BuildContext) {}
/// }
/// ```
// The `async fn` in this trait are only ever called through monomorphised
// generics (never `dyn BuildPipeline`), so the missing `Send` bound on the
// returned futures is intentional and harmless.
#[allow(async_fn_in_trait)]
pub trait BuildPipeline {
    // ── 1. Multimodal (PDF / image / audio / video extraction) ───────────────

    /// Walks the project for non-code files (PDFs, images, audio, video) and
    /// persists extracted content to `multimodal.db::media` plus one node per
    /// page in `graph.db::nodes` (kind = `pdf_page`).
    async fn multimodal_pass(&mut self, ctx: &BuildContext);

    // ── 2. Import-edge resolution ─────────────────────────────────────────────

    /// Resolves `import::*` pseudo-ids in `graph.db::edges.target_qualified`
    /// to real file-qualified-names.  Must run before Leiden.
    async fn resolve_imports_pass(&mut self, ctx: &BuildContext);

    // ── 3. Call-edge resolution ───────────────────────────────────────────────

    /// Resolves `call::*` pseudo-ids in `graph.db::edges.target_qualified` to
    /// the matching `Function` node id.  Must run before Leiden.
    async fn resolve_calls_pass(&mut self, ctx: &BuildContext);

    // ── 4. Leiden community detection ─────────────────────────────────────────

    /// Runs the Rust Leiden solver over `graph.db::edges` and writes
    /// communities + membership into `semantic.db`.  Must run before
    /// embedding and wiki passes.
    async fn leiden_pass(&mut self, ctx: &BuildContext);

    // ── 5. Embedding ──────────────────────────────────────────────────────────

    /// Runs `brain::Embedder::embed_batch` over every substantive node and
    /// persists 384-dim f32 vectors into `semantic.db::embeddings`.
    async fn embedding_pass(&mut self, ctx: &BuildContext);

    // ── 6. Audit (scanner findings) ───────────────────────────────────────────

    /// Spawns `mneme-scanners` over the project and persists findings to
    /// `findings.db`.  Non-fatal: a scanner failure leaves findings.db empty
    /// rather than failing the build.
    async fn audit_pass(&mut self, ctx: &BuildContext);

    // ── 7. Tests shard ────────────────────────────────────────────────────────

    /// Materialises `graph.db::nodes` where `is_test = 1` into
    /// `tests.db::test_files`.
    async fn tests_pass(&mut self, ctx: &BuildContext);

    // ── 8. Git shard ──────────────────────────────────────────────────────────

    /// Mines `git log` + `git blame` (sampled) into `git.db`.  Non-fatal:
    /// missing git binary or non-git directory leaves git.db empty.
    async fn git_pass(&mut self, ctx: &BuildContext);

    // ── 9. Deps shard ─────────────────────────────────────────────────────────

    /// Parses `package.json`, `Cargo.toml`, and `requirements.txt` into
    /// `deps.db::dependencies`.
    async fn deps_pass(&mut self, ctx: &BuildContext);

    // ── 10. Betweenness centrality ────────────────────────────────────────────

    /// Runs the sampled Brandes algorithm over `graph.db::edges` and persists
    /// per-node BC scores to `graph.db::node_centrality`.
    async fn betweenness_pass(&mut self, ctx: &BuildContext);

    // ── 11. Intent (J1 / J2 / J4 / J6) ──────────────────────────────────────

    /// Runs all four intent sub-passes in order:
    ///
    /// * J1 — `@mneme-intent:` magic comment scanner.
    /// * J2 — git-history heuristics (frozen / deferred files).
    /// * J4 — `intent.config.json` glob rules.
    /// * J6 — per-directory `INTENT.md` annotations.
    ///
    /// All four write to `memory.db::file_intent`; later sub-passes skip
    /// files already written by earlier ones.
    async fn intent_pass(&mut self, ctx: &BuildContext);

    // ── 12. Architecture snapshot ─────────────────────────────────────────────

    /// Runs `ArchitectureScanner` over the built graph and appends one row
    /// to `architecture.db::architecture_snapshots`.
    async fn architecture_pass(&mut self, ctx: &BuildContext);

    // ── 13. Conventions ───────────────────────────────────────────────────────

    /// Materialises `DefaultLearner` observations into
    /// `conventions.db::conventions`.
    async fn conventions_pass(&mut self, ctx: &BuildContext);

    // ── 14. Wiki ──────────────────────────────────────────────────────────────

    /// Builds one markdown wiki page per Leiden community and persists rows
    /// into `wiki.db::wiki_pages` (append-only; bumps `version` column).
    async fn wiki_pass(&mut self, ctx: &BuildContext);

    // ── 15. Federated fingerprints ────────────────────────────────────────────

    /// Triggers the federated scan so `federated.db` is populated as a
    /// side-effect of `mneme build` without requiring a separate command.
    async fn federated_pass(&mut self, ctx: &BuildContext);

    // ── 16. Perf baselines ────────────────────────────────────────────────────

    /// Captures build-time throughput numbers into `perf.db::baselines`.
    /// Uses `ctx.started_at`, `ctx.indexed`, `ctx.node_total`,
    /// `ctx.edge_total`.
    async fn perf_pass(&mut self, ctx: &BuildContext);

    // ── 17. Errors ────────────────────────────────────────────────────────────

    /// Persists `ctx.build_errors` into `errors.db::errors` (deduplicated by
    /// blake3 hash of `message + file_path`).
    async fn errors_pass(&mut self, ctx: &BuildContext);

    // ── 18. Live-state ────────────────────────────────────────────────────────

    /// Stamps a `build_completed` event into `livestate.db::file_events`.
    async fn livestate_pass(&mut self, ctx: &BuildContext);

    // ── 19. Agents ────────────────────────────────────────────────────────────

    /// Records a synthetic "build" run in `agents.db::subagent_runs` so the
    /// shard is never 0 rows on a fresh project.
    async fn agents_pass(&mut self, ctx: &BuildContext);

    // ── 20. Seed populate ────────────────────────────────────────────────────

    /// Seeds `history.db::turns` from git commits, `tasks.db::ledger_entries`
    /// from TODO / FIXME / HACK / XXX comments, and `wiki.db::wiki_pages` from
    /// README / CHANGELOG / docs/*.md — so recall returns results on a freshly
    /// built project before any agent turn has run.
    async fn seed_populate_pass(&mut self, ctx: &BuildContext);
}

// ── run_all ───────────────────────────────────────────────────────────────────

/// Run all twenty passes in the canonical order.
///
/// The orchestrator in `run_inline` calls this after the parse loop so the
/// ordering contract is expressed once here rather than repeated in every
/// call site.  Heartbeat phase changes and other per-pass side-effects that
/// belong to the orchestrator are handled by the caller — this function is
/// pure pass sequencing.
pub async fn run_all<P: BuildPipeline>(pipeline: &mut P, ctx: &BuildContext) {
    pipeline.multimodal_pass(ctx).await;
    pipeline.resolve_imports_pass(ctx).await;
    pipeline.resolve_calls_pass(ctx).await;
    pipeline.leiden_pass(ctx).await;
    pipeline.embedding_pass(ctx).await;
    pipeline.audit_pass(ctx).await;
    pipeline.tests_pass(ctx).await;
    pipeline.git_pass(ctx).await;
    pipeline.deps_pass(ctx).await;
    pipeline.betweenness_pass(ctx).await;
    pipeline.intent_pass(ctx).await;
    pipeline.architecture_pass(ctx).await;
    pipeline.conventions_pass(ctx).await;
    pipeline.wiki_pass(ctx).await;
    pipeline.federated_pass(ctx).await;
    pipeline.perf_pass(ctx).await;
    pipeline.errors_pass(ctx).await;
    pipeline.livestate_pass(ctx).await;
    pipeline.agents_pass(ctx).await;
    pipeline.seed_populate_pass(ctx).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock implementation ───────────────────────────────────────────────────

    /// Test double for `BuildPipeline` that records the name of every pass
    /// invoked and the order in which they ran.  The name strings match the
    /// method names defined in the trait so test assertions are
    /// self-documenting.
    struct RecordingPipeline {
        pub calls: Vec<&'static str>,
    }

    impl RecordingPipeline {
        fn new() -> Self {
            Self { calls: Vec::new() }
        }
    }

    impl BuildPipeline for RecordingPipeline {
        async fn multimodal_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("multimodal_pass");
        }
        async fn resolve_imports_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("resolve_imports_pass");
        }
        async fn resolve_calls_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("resolve_calls_pass");
        }
        async fn leiden_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("leiden_pass");
        }
        async fn embedding_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("embedding_pass");
        }
        async fn audit_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("audit_pass");
        }
        async fn tests_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("tests_pass");
        }
        async fn git_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("git_pass");
        }
        async fn deps_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("deps_pass");
        }
        async fn betweenness_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("betweenness_pass");
        }
        async fn intent_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("intent_pass");
        }
        async fn architecture_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("architecture_pass");
        }
        async fn conventions_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("conventions_pass");
        }
        async fn wiki_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("wiki_pass");
        }
        async fn federated_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("federated_pass");
        }
        async fn perf_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("perf_pass");
        }
        async fn errors_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("errors_pass");
        }
        async fn livestate_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("livestate_pass");
        }
        async fn agents_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("agents_pass");
        }
        async fn seed_populate_pass(&mut self, _ctx: &BuildContext) {
            self.calls.push("seed_populate_pass");
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn dummy_ctx() -> BuildContext {
        // `ProjectId::from_path` is fallible and needs a real path; use a
        // temp dir so the test is hermetic.
        let tmp = std::env::temp_dir().join("mneme-build-pipeline-test");
        std::fs::create_dir_all(&tmp).ok();
        let project_id = crate::ids::ProjectId::from_path(&tmp)
            .expect("ProjectId::from_path should succeed on a real directory");
        let paths = crate::paths::PathManager::default_root();
        BuildContext::new(project_id, tmp, paths, false)
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn recording_pipeline_records_all_twenty_passes() {
        let ctx = dummy_ctx();
        let mut pipeline = RecordingPipeline::new();
        run_all(&mut pipeline, &ctx).await;
        assert_eq!(
            pipeline.calls.len(),
            20,
            "run_all must invoke exactly 20 passes; got {:?}",
            pipeline.calls,
        );
    }

    #[tokio::test]
    async fn recording_pipeline_records_passes_in_canonical_order() {
        let ctx = dummy_ctx();
        let mut pipeline = RecordingPipeline::new();
        run_all(&mut pipeline, &ctx).await;

        let expected = [
            "multimodal_pass",
            "resolve_imports_pass",
            "resolve_calls_pass",
            "leiden_pass",
            "embedding_pass",
            "audit_pass",
            "tests_pass",
            "git_pass",
            "deps_pass",
            "betweenness_pass",
            "intent_pass",
            "architecture_pass",
            "conventions_pass",
            "wiki_pass",
            "federated_pass",
            "perf_pass",
            "errors_pass",
            "livestate_pass",
            "agents_pass",
            "seed_populate_pass",
        ];

        for (i, (&recorded, &expected_name)) in
            pipeline.calls.iter().zip(expected.iter()).enumerate()
        {
            assert_eq!(
                recorded, expected_name,
                "pass at position {i} should be '{expected_name}', got '{recorded}'"
            );
        }
    }

    #[tokio::test]
    async fn recording_pipeline_resolve_passes_run_before_leiden() {
        let ctx = dummy_ctx();
        let mut pipeline = RecordingPipeline::new();
        run_all(&mut pipeline, &ctx).await;

        let pos = |name: &str| {
            pipeline
                .calls
                .iter()
                .position(|&c| c == name)
                .unwrap_or(usize::MAX)
        };

        let resolve_imports = pos("resolve_imports_pass");
        let resolve_calls = pos("resolve_calls_pass");
        let leiden = pos("leiden_pass");

        assert!(
            resolve_imports < leiden,
            "resolve_imports_pass ({resolve_imports}) must precede leiden_pass ({leiden})"
        );
        assert!(
            resolve_calls < leiden,
            "resolve_calls_pass ({resolve_calls}) must precede leiden_pass ({leiden})"
        );
    }

    #[tokio::test]
    async fn recording_pipeline_leiden_runs_before_embedding_and_wiki() {
        let ctx = dummy_ctx();
        let mut pipeline = RecordingPipeline::new();
        run_all(&mut pipeline, &ctx).await;

        let pos = |name: &str| {
            pipeline
                .calls
                .iter()
                .position(|&c| c == name)
                .unwrap_or(usize::MAX)
        };

        let leiden = pos("leiden_pass");
        let embedding = pos("embedding_pass");
        let wiki = pos("wiki_pass");

        assert!(
            leiden < embedding,
            "leiden_pass ({leiden}) must precede embedding_pass ({embedding})"
        );
        assert!(
            leiden < wiki,
            "leiden_pass ({leiden}) must precede wiki_pass ({wiki})"
        );
    }

    #[tokio::test]
    async fn recording_pipeline_bookkeeping_passes_run_last() {
        let ctx = dummy_ctx();
        let mut pipeline = RecordingPipeline::new();
        run_all(&mut pipeline, &ctx).await;

        let pos = |name: &str| {
            pipeline
                .calls
                .iter()
                .position(|&c| c == name)
                .unwrap_or(usize::MAX)
        };

        let perf = pos("perf_pass");
        let errors = pos("errors_pass");
        let livestate = pos("livestate_pass");
        let agents = pos("agents_pass");
        let seed = pos("seed_populate_pass");

        // All five bookkeeping passes must come after the last analytics pass
        // (federated_pass).
        let federated = pos("federated_pass");
        for (label, book_pos) in [
            ("perf_pass", perf),
            ("errors_pass", errors),
            ("livestate_pass", livestate),
            ("agents_pass", agents),
            ("seed_populate_pass", seed),
        ] {
            assert!(
                federated < book_pos,
                "federated_pass ({federated}) must precede bookkeeping pass '{label}' ({book_pos})"
            );
        }
    }

    #[test]
    fn build_context_new_has_zero_scalar_defaults() {
        let ctx = dummy_ctx();
        assert_eq!(ctx.indexed, 0);
        assert_eq!(ctx.node_total, 0);
        assert_eq!(ctx.edge_total, 0);
        assert!(ctx.build_errors.is_empty());
        assert!(!ctx.inline_mode);
    }

    #[test]
    fn build_context_inline_mode_flag_propagates() {
        let tmp = std::env::temp_dir().join("mneme-build-pipeline-test-inline");
        std::fs::create_dir_all(&tmp).ok();
        let project_id =
            crate::ids::ProjectId::from_path(&tmp).expect("ProjectId::from_path should succeed");
        let paths = crate::paths::PathManager::default_root();
        let ctx = BuildContext::new(project_id, tmp, paths, true);
        assert!(ctx.inline_mode);
    }
}
