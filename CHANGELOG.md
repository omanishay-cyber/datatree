# Changelog

All notable changes to mneme will be recorded here.

Format loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned for v0.3
- Wire remaining 44 MCP tools to real data (currently only 3: `blast_radius`, `recall_concept`, `health`)
- Supervisor-mediated parse dispatch (currently `mneme build` runs in-CLI-process)
- Real BGE-small ONNX embeddings wired end-to-end (`ort` dep unblocked in v0.2.0, code path still hashing-trick)
- Register 9 Tier-2 Tree-sitter grammars (Swift, Scala, Vue, Julia, Haskell, Kotlin, Svelte, Solidity, Zig) — ABI v15 already supported by tree-sitter 0.25
- Verify C# grammar works post-ABI-bump
- Vision app launched against live graph.db (12 views scaffolded, not yet wired)
- Auto-restart re-enabled (tokio::process::Child Send-recursion unblocked)
- Multimodal Python sidecar end-to-end wired to Rust bridge; PDF ingestion first
- Benchmark CSV published (`just bench-all` ready; numbers not yet committed)
- 60-second demo video

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

### Known v0.2 constraints
- Only 3 of 47 MCP tools are wired to real data (same 3 as v0.1: `blast_radius`, `recall_concept`, `health`). The wired ratio dropped from 9% → 6% because tool *files* grew faster than wiring.
- Supervisor still doesn't dispatch to workers — `mneme build` runs inline in CLI.
- Auto-restart still deferred (tokio::process::Child Send issue).
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

[Unreleased]: https://github.com/omanishay-cyber/mneme/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/omanishay-cyber/mneme/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/omanishay-cyber/mneme/releases/tag/v0.1.0
