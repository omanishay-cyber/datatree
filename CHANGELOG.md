# Changelog

All notable changes to datatree will be recorded here.

Format loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned for v0.2
- Wire remaining 30 MCP tools to `store.ts` helpers (pattern already shipped for `blast_radius`, `recall_concept`, `health`)
- Supervisor-mediated parse dispatch (currently `datatree build` runs in-CLI-process)
- Real BGE-small embeddings via `candle-transformers` (deps commented in `brain/Cargo.toml`)
- Tier-2 Tree-sitter grammars (Swift, Scala, Vue, Julia, Haskell, Kotlin, Svelte, Solidity, Zig)
- C# grammar ABI bump to v13/14 runtime compatibility
- Vision app launched against live graph.db
- Auto-restart re-enabled (tokio::process::Child Send-recursion unblocked)
- Multimodal Python sidecar end-to-end wired to Rust bridge

## [0.1.0] — 2026-04-23

Initial public release. .

### Added
- Multi-process Rust + Bun + Python architecture (10 crates, supervisor-managed)
- **Compaction-resilient Step Ledger** — numbered, verification-gated plans that survive context compaction
- **27 storage layers** per project (code graph, conversation history, decisions, tool cache, todos, errors, findings, multimodal corpus, telemetry, …)
- **33+ MCP tools** — `blast_radius`, `recall_concept`, `health` wired to real data; 30+ follow the same pattern
- **14 visualization view modes** (source written; WebGL renderer targets 100 000+ nodes)
- **18-platform installer** — auto-detects Claude Code, Codex, Cursor, Windsurf, Zed, Continue, OpenCode, Antigravity, Gemini CLI, Aider, Copilot CLI/VS Code, Factory Droid, Trae, Trae-CN, Kiro, Qoder, OpenClaw, Hermes, Qwen
- **Per-project SQLite graph** built in-process by `datatree build .` via Tree-sitter → extractor → `store::inject` pipeline
- **Pure-Rust hashing-trick embedder** — real similarity-preserving vectors with no native DLL dependency
- **Live SSE/WebSocket push channel** (code + schema complete; vision app subscribes)
- **Knowledge-worker mode** — drinks every `.md`, usable for blogs / research / notes, not only code
- **Plain-English LICENSE** (Apache-2.0) — use yes, sell/host/compete/train no

### Verified end-to-end on 2026-04-23
- 40 of 40 workers running under supervisor
- `curl http://127.0.0.1:7777/health` returns live SLA JSON
- `datatree install` writes real manifest blocks to `~/CLAUDE.md`, `~/AGENTS.md`, `~/.claude.json`, `~/.codex/config.toml`
- `datatree build .` indexed the datatree repo itself: **1 922 nodes + 3 643 edges** across 50 files (1 771 calls, 1 605 contains, 267 imports)
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
- 18 platform templates with marker-based idempotent install (`<!-- datatree-start v1.0 -->`)
- Install scripts (POSIX + PowerShell) for supervisor, models, runtime deps, uninstall
- GitHub Actions CI (build + test + clippy + bun check)

[Unreleased]: https://github.com/omanishay-cyber/datatree/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/omanishay-cyber/datatree/releases/tag/v0.1.0
