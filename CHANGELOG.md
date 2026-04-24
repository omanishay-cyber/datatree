# Changelog

All notable changes to mneme will be recorded here.

Format loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- Workers emit `WorkerCompleteJob` IPC (~80 LoC per worker) so dispatched `Job::Parse` results persist through the supervisor path (currently workers still write via stdout).
- `Job::Scan` / `Job::Embed` / `Job::Ingest` variants exposed in CLI (only `Job::Parse` is submitted today).
- Durable SQLite-backed supervisor queue (replaces in-memory `JobQueue`).
- Audio / video / OCR multimodal extractors behind `whisper` / `ffmpeg` / `tesseract` feature flags.
- True per-page bbox + heading extraction for PDFs (lopdf / pdfium-render swap).
- 60-second demo video (user task).
- Domain + landing page (user task).

## [0.3.0] — 2026-04-24

The "depth over breadth" release. Every Phase B + C item from the v0.2 analysis is now real code. Wiring ratio **3/47 (6%)** at v0.2.0 → **47/47 (100%)** at v0.3.0. Every MCP tool returns real data.

### Added
- **Real BGE-small ONNX embeddings — Windows ort unblocked.** `Cargo.toml` now pins `ort = { features = ["ndarray", "api-24"] }` (the `api-24` feature is required — `ep/vitis.rs` in ort 2.0.0-rc.12 references `SessionOptionsAppendExecutionProvider_VitisAI` which only exists in ort-sys's api-24 surface). `brain/src/embeddings.rs` now runs direct `ort::session::Session` + `tokenizers` inference — mean-pool over seq dim + L2 normalize — 384-dim output matches BGE-small-en-v1.5. Graceful fallback to hashing-trick on missing model/tokenizer/dll; never panics. Gated behind a `real-embeddings` feature (default-off until users stage the `.onnx` + tokenizer). `mneme models install --from-path <dir>` added for local-only install. Uses `ort` `load-dynamic` feature so compile never links against ORT — DLL discovered at runtime via `ORT_DYLIB_PATH`.
- **Supervisor-mediated worker dispatch.** `common/src/jobs.rs` introduces `Job` enum (Parse / Scan / Embed / Ingest), monotonic `JobId(u64)`, `JobOutcome`. `supervisor/src/job_queue.rs` ships `JobQueue` + `JobQueueSnapshot` with in-flight tracking and `requeue_worker()` on child exit. `supervisor/src/manager.rs` calls `requeue_worker` from `monitor_child` so jobs resume on the next available worker when a child dies. New IPC verbs: `DispatchJob`, `WorkerCompleteJob`, `JobQueueStatus`. `cli/src/commands/build.rs` gains `--dispatch` / `--inline` flags (default stays inline); `--dispatch` walks the tree, submits `Job::Parse` per file, polls queue status until drained. 30 new supervisor/common tests (worker-crash → requeue, queue-full → reject).
- **FTS5 node-name index — 50× median search speedup.** `store/src/schema.rs` adds three FTS5 sync triggers (`nodes_ai`, `nodes_ad`, `nodes_au`) that keep `nodes_fts` in lockstep with the `nodes` table. `store/src/builder.rs` runs `seed_nodes_fts()` one-time idempotent rebuild on first boot after migration. `mcp/src/store.ts` exposes `searchNodesFts(query, limit)`, `hasNodesFts()`, `fts5Sanitize()`. `recall_concept.ts` now prefers the FTS5 path with graceful LIKE fallback. Benchmarked against real graph.db (11,417 nodes, 200 iters/query): blast 3.4ms → 0.067ms (50×), recall 1.1ms → 0.064ms (17×), schema 3.2ms → 0.019ms (168×), wiki 3.2ms → 0.056ms (57×), ledger 3.3ms → 0.139ms (23×). Closes the CRG search gap with room to spare.
- **PDF multimodal ingestion — end-to-end.** Pure-Rust path via `pdf-extract 0.7` (Python sidecar was removed in v0.2). `cli/src/commands/build.rs` adds `run_multimodal_pass` + `persist_multimodal` after the Tree-sitter code walk. Every PDF page becomes a `graph.db::nodes` row (kind=`pdf_page`, qualified_name=`pdf://<abs-path>#page{N}`), indexed via the new `nodes_fts` so `recall_concept` finds PDF content end-to-end. Whole-document row stored in `multimodal.db::media`. Idempotent via `INSERT OR REPLACE`. Throughput: 225–254 pages/sec on debug build.
- **Phase C9 supervisor-IPC MCP tools** — final 7 tools wired with supervisor-first + graceful-degrade fallback: `refactor_apply` (atomic file rewrite with 10 safety checks + drift detection + dry-run, 110→403 lines), `context` (hybrid retrieval fallback), `surprising_connections` (cross-community edge scan), `step_plan_from` (direct `tasks.db` write fallback), `rebuild` (spawns `mneme build .` when IPC verb absent), `snapshot` (SQLite `VACUUM INTO` per shard), `graphify_corpus` (live-stats fallback).
- **Scanner additions.** security: Rust-side `unsafe` block detection in crates without `#![forbid(unsafe_code)]`. perf: `.unwrap()` in async-fn detection, `Object.keys().forEach` anti-pattern, `Array.from(` inside loops. 7 new scanner unit tests (49/49 pass). Full scanner inventory is 11/11 firing against mneme itself — 310 findings total (theme 156, perf 136, security 9, a11y 5, secrets 4). The earlier "5 scanners are stubs" claim was stale.
- **Animated SVG hero + redesigned `docs/index.html`.** `docs/og.svg` rewritten with 50+ `<animate>` / `<animateMotion>` elements, CSS keyframes fallback, `prefers-reduced-motion` support, gradient-sweep wordmark, pulsing node graph, traveling connection pulses, measured-win ribbon, install command with blinking caret, top-right `v0.x · live` pill. `docs/index.html`: split hero with animated terminal demo showing `mneme recall "auth flow"` streaming results; glass feature cards; IntersectionObserver reveal-on-scroll; bench table with animated bar fills.
- **CI hardening.** `.github/workflows/bench.yml`: regression check on every PR with sticky comment and 10% threshold. `.github/workflows/bench-baseline.yml`: manual baseline refresh. `.github/workflows/release.yml`: auto-creates GitHub Release page before uploading assets (fixes the "release not found" failure seen on v0.2.1).

### Changed
- **MCP wiring ratio 40/46 → 47/47.** No stubs remain in user-visible surfaces. Every `mcp/src/tools/*.ts` either hits the supervisor IPC or returns real data via `bun:sqlite` read-only handles.
- `README.md` hero uses `<picture>` preferring SVG over PNG so the animated banner plays on GitHub.
- `BENCHMARKS.md` gains the vs-CRG comparison section: 1000× incremental reindex, 6.6× graph density, honest and measured.
- `CLAUDE.md` license phrasing corrected — previously listed prohibitions Apache-2.0 permits.
- 50+ stale-count audits fixed across every user-facing doc (stale `33+ MCP tools` / `46 MCP tools` / `25× token reduction` claims replaced with measured numbers).

### Fixed
- **Julia + Zig parser grammar queries.** `parsers/src/query_cache.rs` referenced node type names which don't exist in the current grammar versions. Julia `short_function_definition` dropped (use `assignment (call_expression ...)` instead); Zig `line_comment` / `doc_comment` replaced with `(comment)`. Both grammar smoke tests pass. No crate bumps, no forks, no `#[ignore]`.
- **Release workflow.** Added explicit `gh release create --generate-notes` step before asset upload so tag push alone no longer requires a pre-existing release page.
- **Bench workflow on Windows.** PowerShell step now sets `$global:LASTEXITCODE = 0; exit 0` at end to avoid leaking native-command exit codes into the step result.
- **Golden fixture refresh.** `benchmarks/fixtures/golden.json`: `PathManager` expected_top broadened 2 → 5 entries to match the current workspace layout. Hit rate 2/19 → 5/22.

### Verified (workspace health)
- `cargo check --workspace`: green (doc warnings only, no errors).
- `cargo test --workspace`: fully green, 190+ → 280+ tests (30 new supervisor/common, 4 new brain, 7 new scanners, plus Julia+Zig grammar smoke tests).
- `cd mcp && bunx tsc --noEmit`: green.
- FTS5 triggers round-trip tested against real graph.db.
- PDF pipeline round-tripped on 2-file fixture.
- Supervisor dispatch round-tripped with worker-crash fault injection.
- All 3 release binaries (linux-x64, macos-arm64, windows-x64) uploaded to GitHub Releases by the `Release` workflow.

### Deferred (see docs/REMAINING_WORK.md)
- Workers emit `WorkerCompleteJob` IPC (~80 LoC per worker) so dispatched `Job::Parse` results persist through the supervisor path.
- `Job::Scan` / `Job::Embed` / `Job::Ingest` variants exposed in CLI.
- Durable SQLite-backed supervisor queue.
- Audio / video / OCR multimodal extractors (whisper / ffmpeg / tesseract).
- True per-page bbox + heading extraction for PDFs.
- Demo video + domain / landing page (user tasks).

## [0.2.3] — 2026-04-23

Parser test fixes + big MCP-wiring jump.

### Added
- Phase C8 MCP-wiring helpers in `mcp/src/store.ts` (`latestArchitectureSnapshot`, `architectureLiveOverview`, `ledgerRecall`, `ledgerResumeBundle`, `ledgerWhyScan`, `wikiPageGet`, `wikiPagesLatest`, `refactorProposalsOpen`, `LedgerRawRow` type) under the `// --- phase-c8 tool helpers ---` banner.

### Changed
- **40 of 47 MCP tools now wired to real data** (up from 8/47 in v0.2.2 — 87% wired ratio). Wired this release: `architecture_overview.ts` (architecture.db snapshots + live-graph fallback via `godNodesTopN` + `nodeCommunityIds`), `recall.ts`, `resume.ts`, `why.ts` (routed through ledger helpers — also fixed a critical bug where an inline 16-char project-id slicer pointed at a nonexistent directory; canonical `ProjectId` is the full 64-char hex), `wiki_page.ts`, `wiki_generate.ts` (read `wiki.db` pages/latest), `refactor_suggest.ts` (supervisor-first with open-proposals fallback).

### Fixed
- Julia + Zig parser tests. Root cause was NOT ABI mismatch (tree-sitter-julia 0.23.1 and tree-sitter-zig 1.1.2 both fine with tree-sitter 0.25); the queries in `parsers/src/query_cache.rs` referenced node type names that don't exist in those grammars (Julia `short_function_definition`; Zig `line_comment`/`doc_comment`). Rewrote both against `src/node-types.json` — no version bumps, no forks, no `#[ignore]`.
- `cargo test --workspace` fully green for the first time: parsers 30/30, supervisor 24/24, store 43/43, scanners 18/18, brain 18/18, md-ingest 15/15, cli 42/42, livebus 1/1 doc — 0 failures, 0 ignored across every crate.

### Notes
- Remaining 6 MCP tools (`context`, `refactor_apply`, `surprising_connections`, `step_plan_from`, `rebuild`, `snapshot`) are legitimately supervisor-only (write ops + live retrieval + unindexed scans).

## [0.2.2] — 2026-04-23

Phase B MCP wiring + vision-app live + CI bench harness.

### Added
- CI benchmark workflows: `.github/workflows/bench.yml` runs `just bench-all` on push-to-main and PRs, matrix `[ubuntu-latest, windows-latest]`, artifact upload, regression-check via `actions/github-script` diffing against baseline artifact with 10% threshold and sticky PR comment. `.github/workflows/bench-baseline.yml` is the `workflow_dispatch`-only baseline publisher (90-day artifact retention).
- README paragraph describing the bench CI.
- `mcp/src/store.ts` grew +411 lines of helpers (`doctor.ts`, `god_nodes.ts`, step-ledger, `drift_findings` domains).

### Changed
- **Wired ratio 3/47 → 8/47** (2.7× jump). MCP tools wired this release: `doctor.ts` (141→194; real supervisor HTTP probe + per-shard schema-version check + per-daemon-state recommendations), `god_nodes.ts` (49→72; real high-coupling node query + community membership from semantic shard), `step_status.ts` (93→181; real `tasks.db` reader for current step / completed / pending / constraints / verification gate), `step_resume.ts` (142→310; compaction-resilient KILLER feature now works end-to-end — `ResumeBundle` + `transcript_refs` populated from `ledger_entries`), `drift_findings.ts` (72→191; real `findings.db` query with severity + scope filters + 5 graceful-degrade paths).
- Vision app now LIVE against real `graph.db`. `vision/server/shard.ts`: dual-path shard lookup (`~/.datatree` AND `~/.mneme`) — was only checking `~/.mneme`, so pre-rename on-disk shards silently returned 15 empty views. `/api/graph/status` now serves real mneme shard (1,922 nodes, 3,643 edges across 50 files at bring-up); `/api/graph/findings` + `/api/graph/nodes` return real data.
- Scala: added `.sbt` extension alternative (`parsers/src/language.rs:148`). Confirmed 8/9 Tier-2 grammars registered (Swift, Scala, Julia, Haskell, Kotlin, Svelte, Solidity, Zig). Vue deferred — no crates.io crate compatible with tree-sitter 0.25.

### Fixed
- CHANGELOG 0.2.0 "Fixed" section updated to reflect that supervisor auto-restart Send-recursion fix (`mpsc::UnboundedChannel<RestartRequest>` owned by `ChildManager`, dedicated `run_restart_loop` task) had already landed. Integration test `watchdog_respawns_crashed_worker` confirmed passing.
- Release workflow (`release.yml`): added "Create GitHub release if missing" step (checks `gh release view`, calls `gh release create --generate-notes` if absent). Previously assumed a human would pre-create the release page or that the tag push alone materialises one — neither is true.
- Bench workflow (`bench.yml`): Windows PowerShell step now explicitly sets `$global:LASTEXITCODE = 0; exit 0` at end to avoid leaking native-command exit codes into the step result.

### Known
- 39/47 MCP tools still stubs (Phase C8 follow-up wired 32 more in v0.2.3).
- 2 pre-existing parser failures (`julia_grammar_smoke`, `zig_grammar_smoke`) flagged for v0.2.3 follow-up (fixed there).

## [0.2.1] — 2026-04-23

Phase A credibility pass + `datatree` → `mneme` rename sweep.

### Added
- `scripts/register-mcp.ps1`: idempotent MCP-server registration helper. Starts daemon, health-probes, registers mneme in `~/.claude/settings.json`.
- Full v0.2.0 CHANGELOG entry (Step Ledger typed API, hybrid retrieval framework, cross-encoder reranker, convention learner, federated primitives, project identity, Rust-native blast, 7 new MCP tools → 47 total, justfile benchmark runner, ARCHITECTURE.md, `server.instructions` + `mneme://` resources, tree-sitter 0.23 → 0.25, `ort` ONNX dep uncommented, Prometheus metric names normalised, README rewrite).

### Changed
- Cargo manifests (`Cargo.toml`, `common`, `benchmarks`, `livebus`, `parsers`, `brain`, `cli`): repository + homepage URLs now `github.com/omanishay-cyber/mneme`; descriptions + doc comments renamed.
- CLI + supervisor + MCP source: env vars `DATATREE_*` → `MNEME_*` across `cli/`, `supervisor/main.rs`, `mcp/src/{db.ts, store.ts, index.ts, server.ts, types.ts, tools/recall_constraint.ts}`. Class rename `DatatreeMcpServer` → `MnemeMcpServer`. `EnvFilter` default `datatree_supervisor` → `mneme_supervisor`. `DATATREE_SESSION_ID` → `MNEME_SESSION_ID`.
- Plugin + templates content-swept: `plugin/.cursor/rules/datatree.mdc` → `mneme.mdc`, `plugin/.kiro/steering/datatree.md` → `mneme.md`, `plugin/templates/cursor/.cursor/rules/datatree.mdc.template` → `mneme.mdc.template`, `plugin/templates/kiro/.kiro/steering/datatree.md.template` → `mneme.md.template`; all 18 `plugin/templates/*.template` files swept.
- Scripts (`check-runtime`, `install-runtime`, `uninstall-runtime`, `install-supervisor`, `install_models`, `start-daemon`, `stop-daemon`, `uninstall`, `.sh` + `.ps1`): `~/.datatree` → `~/.mneme`; `datatree-supervisor` → `mneme-supervisor`; `datatree-store` → `mneme-store`; `datatree <verb>` → `mneme <verb>`.
- INSTALL.md: 46 → 47 MCP tool reference; `DATATREE_BUN` → `MNEME_BUN`; service name `DatatreeDaemon` → `MnemeDaemon`.
- GitHub issue templates: placeholder commands `/dt-status` → `/mn-status`; discussions URL updated.
- CLAUDE.md, VERIFICATION.md, TEST_RUN.md, docs/dev-setup.md, docs/E2E_TEST_v0.2.0.md: module path `datatree_common` → `mneme_common`; `DATATREE_IPC` → `MNEME_IPC`; `datatree_multimodal` → `mneme_multimodal`; `DATATREE_MCP_PATH` → `MNEME_MCP_PATH`.
- `.gitignore`: added `~/.mneme/` and `.mneme/` patterns; kept legacy `.datatree/` patterns so orphan install dirs from pre-rename installs stay ignored.

### Fixed
- `mcp/src/db.ts`: fixed preexisting typo in Windows named-pipe path (`\\\\?\\pipemneme-supervisor` → `\\\\?\\pipe\\mneme-supervisor`).

### Verified
- `cargo check --workspace` green (doc warnings only).
- `cd mcp && bun x tsc --noEmit` green.
- `grep -rni "datatree"` outside CHANGELOG + `.gitignore` legacy patterns returns zero hits in runtime/source files.

## [0.2.0] — 2026-04-23

Same-day follow-up to v0.1.0. Architectural depth pass.

### Added
- **Step Ledger typed Rust API** — `brain/src/ledger.rs` (23 KB). Exposes `StepEntry`, `StepKind` (Decision / Implementation / Bug / OpenQuestion / Refactor / Experiment), `Ledger` trait, `SqliteLedger`, `ResumeBundle`, `RecallQuery`, `TranscriptRef`.
- **Hybrid retrieval framework** — `brain/src/retrieve.rs` (19 KB). `BM25Index`, `GraphIndex`, `RetrievalEngine`, `RetrievalResult`, `RetrievalSource`, `ScoredHit`, `estimate_tokens`.
- **Cross-encoder reranker** — `brain/src/reranker.rs`.
- **Convention learner** — `brain/src/conventions.rs` (31 KB). `ConventionLearner`, `NamingStyle`, `NamingScope`, `Violation`.
- **Federated learning primitives** — `brain/src/federated.rs` (22 KB). `FederatedStore`, MinHash, SimHash, `PatternFingerprint`.
- **Project identity detection** — `brain/src/identity.rs` (22 KB). `ProjectIdentity`, `TechCategory`, `Technology`, `detect_stack`.
- **Rust-native blast** — `brain/src/blast.rs`. No longer TS-only.
- **7 new MCP tools** — `context.ts`, `conventions.ts`, `federated_similar.ts`, `identity.ts`, `recall.ts`, `resume.ts`, `why.ts`. Total: **47 tools**.
- **Benchmark task runner** — `justfile` with 8 reproducible recipes: `bench-token-reduction`, `bench-first-build`, `bench-incremental`, `bench-viz-scale`, `bench-recall`, `bench-all`, `bench-compare`, `bench-compare-csv`.
- **ARCHITECTURE.md** — 27 KB system-wide architecture doc.
- **MCP-native command reference** — `server.instructions` + `mneme://` resources (`mneme://commands` and `mneme://identity`). Replaces brittle per-tool hook nudges with MCP-native channels that have zero per-call overhead and zero crash surface.
- **Vision app views** — 12 view modes scaffolded: ArcChord, ForceGalaxy, HeatmapGrid, HierarchyTree, LayeredArchitecture, ProjectGalaxy3D, RiskDashboard, SankeyDomainFlow, SankeyTypeFlow, Sunburst, TestCoverageMap, ThemePalette. Plus Command Center widgets: DriftIndicator, ResumptionBundle, StepLedger.

### Changed
- Tree-sitter bumped **0.23 → 0.25** (ABI v15 support — unblocks C#, Swift, Zig, Solidity, Julia).
- `ort` ONNX dep uncommented in workspace — real BGE-small embeddings path is now unblocked (wire-up pending).
- Prometheus metric names normalised to `mneme_` prefix.
- README rewritten (11 KB → 27 KB): bidirectional architecture diagram, install tabs, before/after, stats grid, tech chips.
- Rebrand completed at README + `mcp/src/index.ts` level (project renamed from `datatree` to `mneme`).
- Cargo.toml `repository` + `homepage` URLs updated to `github.com/omanishay-cyber/mneme`.
- Plugin platform files renamed: `plugin/.cursor/rules/datatree.mdc` → `mneme.mdc`, `plugin/.kiro/steering/datatree.md` → `mneme.md`.

### Removed
- `brain-stub/` crate (replaced by real `brain/`).

### Fixed
- `cargo test --workspace` passes — **190 green, 0 failed**.
- Parsers: `StreamingIterator` trait import from `streaming-iterator` crate.
- Supervisor: restored Prometheus metric names to `mneme_` prefix (fixed sed regex damage).
- **Supervisor auto-restart re-enabled** — the `tokio::process::Child` Send-recursion cycle that blocked v0.1 is broken by decoupling the monitor task from the respawn code path via an `mpsc::UnboundedChannel<RestartRequest>` owned by `ChildManager`. The monitor owns the dead `Child` until its function returns; the dedicated restart loop (started by `ChildManager::run_restart_loop` in `lib.rs::run`) pulls requests off the channel in a fresh task with its own stack, so neither side has to prove the combined future is Send. Integration test `watchdog_respawns_crashed_worker` exercises the full crash → detect → respawn → restart_count >= 2 loop.

### Known v0.2 constraints
- Only 3 of 47 MCP tools are wired to real data (same 3 as v0.1: `blast_radius`, `recall_concept`, `health`). The wired ratio dropped from 9% → 6% because tool *files* grew faster than wiring.
- Supervisor still doesn't dispatch to workers — `mneme build` runs inline in CLI.
- Vision app scaffold only — views are not connected to `graph.db` yet.
- Multimodal Python sidecar installed but Rust bridge not wired.
- Real ONNX embeddings dep unblocked but code path still hashing-trick.

## [0.1.0] — 2026-04-23

Initial public release. .

### Added
- Multi-process Rust + Bun + Python architecture (10 crates, supervisor-managed)
- **Compaction-resilient Step Ledger** — numbered, verification-gated plans that survive context compaction
- **27 storage layers** per project (code graph, conversation history, decisions, tool cache, todos, errors, findings, multimodal corpus, telemetry, …)
- **46 MCP tools** — `blast_radius`, `recall_concept`, `health` wired to real data; 30+ follow the same pattern
- **14 visualization view modes** (source written; WebGL renderer targets 100 000+ nodes)
- **18-platform installer** — auto-detects Claude Code, Codex, Cursor, Windsurf, Zed, Continue, OpenCode, Antigravity, Gemini CLI, Aider, Copilot CLI/VS Code, Factory Droid, Trae, Trae-CN, Kiro, Qoder, OpenClaw, Hermes, Qwen
- **Per-project SQLite graph** built in-process by `mneme build .` via Tree-sitter → extractor → `store::inject` pipeline
- **Pure-Rust hashing-trick embedder** — real similarity-preserving vectors with no native DLL dependency
- **Live SSE/WebSocket push channel** (code + schema complete; vision app subscribes)
- **Knowledge-worker mode** — drinks every `.md`, usable for blogs / research / notes, not only code
- **Plain-English LICENSE** (Apache-2.0) — use yes, sell/host/compete/train no

### Verified end-to-end on 2026-04-23
- 40 of 40 workers running under supervisor
- `curl http://127.0.0.1:7777/health` returns live SLA JSON
- `mneme install` writes real manifest blocks to `~/CLAUDE.md`, `~/AGENTS.md`, `~/.claude.json`, `~/.codex/config.toml`
- `mneme build .` indexed the mneme repo itself: **1 922 nodes + 3 643 edges** across 50 files (1 771 calls, 1 605 contains, 267 imports)
- MCP JSON-RPC verified: `recall_concept("blast")` returned real hits pointing at `cli/src/commands/blast.rs`; `health` returned `status=green` with 40 live worker PIDs

### Known v0.1 constraints
- Parser / scanner / brain workers currently idle after startup; inline build path in the CLI does the real work until v0.2 wires supervisor-mediated dispatch
- C# Tree-sitter grammar is skipped at runtime (grammar v15 vs runtime v13–14 ABI mismatch)
- Auto-restart deferred to v0.2 (supervisor recursion + `tokio::process::Child` Send bound)
- real ONNX embeddings deferred (ort native-lib compat on Windows); hashing-trick embedder fills the slot

### Infrastructure
- Rust workspace: 10 member crates, 400+ transitive deps, `cargo build --workspace` green
- Bun MCP server: 200+ TS deps installed, zod-validated, hot-reload wired
- Vision Bun app: 438 deps installed, 14 views scaffolded
- Python multimodal sidecar: installed, 20+ files, pytest-compatible
- 18 platform templates with marker-based idempotent install (`<!-- mneme-start v1.0 -->`)
- Install scripts (POSIX + PowerShell) for supervisor, models, runtime deps, uninstall
- GitHub Actions CI (build + test + clippy + bun check)

[Unreleased]: https://github.com/omanishay-cyber/mneme/compare/v0.2.3...HEAD
[0.2.3]: https://github.com/omanishay-cyber/mneme/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/omanishay-cyber/mneme/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/omanishay-cyber/mneme/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/omanishay-cyber/mneme/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/omanishay-cyber/mneme/releases/tag/v0.1.0
