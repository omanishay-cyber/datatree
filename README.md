<div align="center">

<a href="https://omanishay-cyber.github.io/mneme/">
  <picture>
    <source srcset="docs/og.svg" type="image/svg+xml"/>
    <img src="docs/og.png" alt="Mneme - the persistent memory layer for AI coding" width="100%"/>
  </picture>
</a>

<br/><br/>

# Claude remembers your code. Even when you don't.

<sub>The persistent memory layer for AI coding. 100% local. Apache-2.0.</sub>

</div>

Stop re-explaining your codebase to Claude every chat. Mneme keeps what Claude learned about your project, survives context wipes, doesn't forget mid-task, runs entirely on your laptop.

<div align="center">

<a href="https://github.com/omanishay-cyber/mneme/releases/tag/v0.4.0"><img src="https://img.shields.io/badge/Download%20v0.4.0-16a37c?style=for-the-badge&labelColor=0a0a0c" alt="Download v0.4.0"/></a>
&nbsp;
<a href="#-quick-start"><img src="https://img.shields.io/badge/Quick%20start-9a9a9a?style=for-the-badge&labelColor=0a0a0c" alt="Quick start"/></a>

<br/><br/>

<img src="https://img.shields.io/badge/50%20MCP%20tools-0a0a0c?style=flat-square&labelColor=16a37c&color=0a0a0c" alt="50 MCP tools"/>
&nbsp;
<img src="https://img.shields.io/badge/27%20SQLite%20shards-0a0a0c?style=flat-square&labelColor=4191E1&color=0a0a0c" alt="27 SQLite shards"/>
&nbsp;
<img src="https://img.shields.io/badge/19%20AI%20tools%20wired-0a0a0c?style=flat-square&labelColor=22D3EE&color=0a0a0c" alt="19 AI tools wired"/>

</div>

```powershell
# Windows (preferred) * winget package * built into Windows 10 1809+ / 11
winget install Anish.Mneme       # also available as Anish.Mnemeos
```

```powershell
# Windows (no winget) * one command * no admin * auto-detects x64 / ARM64
iex (irm https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/bootstrap-install.ps1)
```

```bash
# macOS * one command * auto-detects Intel / Apple Silicon
curl -fsSL https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/install-mac.sh | bash
```

```bash
# Linux * one command * auto-detects x64 / ARM64
curl -fsSL https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/install-linux.sh | bash
```

```bash
# Python (any OS) * pip-friendly wrapper * detects platform + arch automatically
pip install mnemeos && mnemeos
```

> Pick whichever route matches your environment - all five end up at the same `~/.mneme` install. Restart Claude after install. Verify with `mneme doctor` and `claude mcp list`.
>
> **Branding note:** the project is **Mneme OS**. The pip distribution is `mnemeos` (the bare name `mneme` was claimed on PyPI in 2014 by an unrelated package). The CLI binary is `mneme` with `mnemeos` as a parallel alias - both names work everywhere.
>
> **Requirements:** 64-bit OS (x64 or ARM64) * CPU with AVX2 / BMI2 / FMA (Intel Haswell 2013+ or AMD Excavator 2015+ - almost every PC sold since 2013 qualifies) * 5 GB free disk * no admin needed. 32-bit Windows is not supported (Bun runtime requirement).

<!-- ==================================================================== -->
<!--   Nav                                                                  -->
<!-- ==================================================================== -->

<p>
  <strong>
    <a href="#-quick-start">Quick start</a>
    &nbsp;*&nbsp; <a href="#-what-it-does">What it does</a>
    &nbsp;*&nbsp; <a href="#-the-killer-feature">Killer feature</a>
    &nbsp;*&nbsp; <a href="#-benchmarks">Benchmarks</a>
    &nbsp;*&nbsp; <a href="#-19-supported-platforms">Platforms</a>
    &nbsp;*&nbsp; <a href="ARCHITECTURE.md">Architecture</a>
    &nbsp;*&nbsp; <a href="docs/">Docs</a>
  </strong>
</p>

<sub>🌳 Named after <strong>Mneme</strong>, the Greek muse of memory. Because "remembering" is the hardest problem in AI coding.</sub>

</div>

---


## Feature matrix vs Code Review Graph and Graphify

Compared against the two closest projects in the AI-code-context space:
[Code Review Graph (CRG)](https://github.com/tirth8205/code-review-graph),
[Graphify](https://github.com/safishamsi/graphify), and
[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) for the parsing layer.

| Capability | **Mneme v0.4.0** | Code Review Graph | Graphify | Tree-sitter |
|---|---|---|---|---|
| **Persistent daemon (always-on memory)** | ✅ multi-process Rust supervisor (22 workers auto-scaled to CPU), survives logout via Scheduled Task / launchd / systemd-user | ❌ per-invocation Python script | ❌ per-invocation Python script | n/a (parser library) |
| **Compaction recovery (Step Ledger)** | ✅ numbered, verification-gated, SQLite-persisted; Claude resumes the EXACT step after `/compact` | ❌ | ❌ | n/a |
| **Persistent memory across AI sessions** | ✅ history.db + agents.db + tool_cache.db + livestate.db; works across model providers | ⚠️ memory_loop_store (Markdown nodes, single project) | ❌ | n/a |
| **MCP tools (live, all wired to real data)** | ✅ **48** | 24 | n/a (not an MCP server) | n/a |
| **Built-in scanners** | ✅ **11** (theme * security * perf * a11y * drift * ipc * md-drift * secrets * refactor * architecture * types) | 1 (review-oriented) | ❌ | ❌ |
| **Tree-sitter grammars** | ✅ 27 (18 Tier-1 + 8 Tier-2 + extensible) | 23 | 5-ish (per-input) | 200+ (community) |
| **Drift detector enforcing CLAUDE.md rules live** | ✅ 12 scanners incl. drift + md-drift + secrets | partial (lint-style) | ❌ | ❌ |
| **Storage layers (SQLite shards)** | ✅ **27 sharded DBs** + global meta.db (graph * semantic * git * deps * tests * multimodal * wiki * architecture * federated * history * tasks * agents * tool_cache * livestate * errors * perf * refactors * contracts * insights * telemetry * corpus * audit * memory * findings * concepts * meta) | 1 | 1-2 JSON + HTML | n/a |
| **Real local embeddings** | ✅ BGE-small-en-v1.5 (384-dim, ONNX, ORT 1.24.4, Cloudflare-hosted via HF) + Qwen 2.5 Coder/Embed 0.5B + Phi-3-mini-4k local LLMs (3.4 GB total, all GGUF) | ❌ | partial (sentence-transformers, network-pullable) | n/a |
| **Visualization surface** | ✅ **14 WebGL views** + Command Center (Tauri SvelteKit app, served from daemon at `:7777`) | 1 (D3 force graph) | 1 (static HTML) | n/a |
| **Multi-process Rust supervisor (watchdog + WAL + restart + health)** | ✅ HTTP `/health` on `:7777`, per-worker uptime + restart count + dropped count, ProcessRefreshKind PID-liveness via sysinfo | ❌ (single-process Python) | ❌ (single-process Python) | n/a |
| **Multimodal (PDF / image / OCR)** | ✅ multimodal-bridge crate, Tesseract OCR runtime fallback, 187 pages/sec on a 1100-file project | ❌ | partial (text only by default) | n/a |
| **Live push updates (SSE + WebSocket)** | ✅ livebus worker, multi-agent pubsub | ❌ | ❌ | n/a |
| **100% local, zero unsolicited network** | ✅ enforced across Rust / TS / Python sidecar - only opt-in network is `mneme models install --from-url` | ✅ | ⚠️ model downloads + Whisper prompts | ✅ |
| **AI tool integration (out of the box)** | ✅ **19+** (Claude Code, Codex, Cursor, Windsurf, Zed, VS Code, Gemini, Qwen, Qoder, plus more via standard MCP) | 2 (Claude Code, VS Code ext) | 1 (manual integration) | dozens of editors via library |
| **Cross-OS install** | ✅ 4 routes — `winget install Anish.Mneme` * `pip install mneme-mcp` * `curl ... install-mac.sh` * `curl ... install-linux.sh` (Win arm64 + Linux arm64 CI building) | ✅ pip is OS-agnostic | ✅ pip is OS-agnostic | bindings per language |
| **HF Hub model mirror (~5× faster than GitHub Releases)** | ✅ huggingface.co/aaditya4u/mneme-models | n/a (no models) | n/a | n/a |
| **Audit pipeline streams findings (no data loss on timeout)** | ✅ per-file flush, supervisor fan-out across 6 scanner-workers (~5× faster on multi-core) | ❌ | ❌ | n/a |
| **License** | ✅ **Apache-2.0** | MIT | MIT | MIT |
| **CPU baseline (perf)** | x86-64-v3 (Haswell 2013+, AVX2 + BMI2 + FMA - 2-4× faster than baseline x86-64) | generic Python | generic Python | configurable per binding |
| **Restart-survival (daemon respawn on crash)** | ✅ supervisor watchdog with heartbeat deadline + restart count + dropped count | n/a | n/a | n/a |
| **Federated cross-project pattern matching** | ✅ federated.db + 326 fingerprints per project, cross-shard `mneme_federated_similar` MCP tool | ❌ | ❌ | ❌ |
| **8 Claude Code hooks integrated** | ✅ UserPromptSubmit * PreToolUse * PostToolUse * PreCompact * SubagentStop * SessionEnd + 2 more - persistent-memory pipeline live across compactions | ❌ | ❌ | n/a |
| - | - | - | - | - |
| **Smart question generation from topology** | ✅ `mcp__mneme__smart_questions` (graph centrality + complexity + anomaly scoring) | ✅ auto-generated review prompts | ❌ | ❌ |
| **Portable graph exports (GraphML / Obsidian / Cypher / SVG / JSON-LD)** | ✅ `mneme graph-export` (5 formats) | ✅ multiple formats | partial (GraphML) | n/a |
| **Seed concept memory (user-registered architectural keywords)** | ✅ `recall_concept` persisted to `concepts.db` with decay function | ❌ | ✅ seed nodes | ❌ |
| **Multilingual Whisper for non-English audio** | ✅ Whisper transcription, auto-language-detected (mp3/wav/m4a/mp4/mov) | n/a | ✅ specialised locale prompts | n/a |
| **One-shot `pip install`** | ✅ `pip install mneme-mcp` (any OS with Python) | ✅ `pip install crg` | ✅ `pip install graphify` | ✅ multiple bindings |
| **Standalone library / SDK (Rust + Python + JS bindings)** | ✅ `sdk/python` (PyPI `mneme-parsers`), `sdk/js` (npm `@mneme/parsers`), `sdk/rust` crate | n/a | n/a | ✅ flagship |
| - | - | - | - | - |
| **Things mneme doesn't have YET (CRG / Graphify do)** | | | | |
| **Graph diff (commit-to-commit)** | ❌ planned v0.4.1 | ✅ `graph diff` | ❌ | ❌ |
| **VS Code / JetBrains / Cursor extensions** | ❌ planned v0.6 | ✅ first-class VS Code | ❌ | ✅ pervasive editor support |
| **Hosted browser demo / playground** | ❌ planned v0.5.5 | ✅ | ✅ | ✅ web playground |

Rows marked "planned vX.Y" reference [`docs/ROADMAP.md`](ROADMAP.md) and the v0.4 vision document. Every gap has a ship target.

### Why Mneme

Code Review Graph is a review-focused graph with a VS Code extension.
Graphify is a knowledge-graph builder for code plus multimodal content.
Tree-sitter is the parser library mneme uses under the hood.

Mneme is the heaviest tool of the three. It's a Rust supervisor that runs
between your AI sessions, survives Claude's context wipes at the architecture
level (not the prompt level), enforces your `CLAUDE.md` rules live, federates
patterns across all your projects, and gives every AI tool you use the same
memory. Bigger install, more capabilities.

Pick CRG if you want one-command install for a single project review.
Pick Graphify if you want a multimodal knowledge graph for documents and audio.
Pick mneme if you want a persistent memory layer that runs across many projects
and many AI tools without forgetting.

The remaining gaps (graph diff, editor extensions, hosted demo) are tracked in `docs/ROADMAP.md`.

## Comparison: four code-graph MCPs

We benchmarked four code-graph MCPs through Claude Code 2.1.126 on the mneme
workspace itself (Rust + TypeScript + Python, 50K+ LOC, 400+ files) running on
a Windows 11 AWS test instance. Each MCP got the same five questions. The driver passed
`--strict-mcp-config` so only that MCP's tools were available, and Claude
couldn't fall back to built-in `Read`/`Grep`/`Glob`.

### MCPs under test

| MCP | Version | Install | Index build | Graph size (more = more code parsed) |
|---|---|---|---|---|
| **mneme** (this project) | v0.4.0 | `iex (irm https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/bootstrap-install.ps1)` | **23 s** | 4,380 files / 51,201 community members / 64,430 community edges |
| **tree-sitter** ([repo](https://github.com/wrale/mcp-server-tree-sitter)) | v0.7.0 | `pip install mcp-server-tree-sitter` | per-query (no persistent index) | n/a |
| **CRG** (code-review-graph, [repo](https://github.com/tirth8205/code-review-graph)) | v2.3.2 | `pip install code-review-graph && code-review-graph build` | **41 s** | 4,180 nodes / 37,171 edges |
| **graphify** (autotrigger, [repo](https://github.com/ChharithOeun/mcp-graphify-autotrigger)) | v0.3.0 + graphifyy v0.6.7 | `pip install 'mcp-graphify-autotrigger[all] @ git+https://github.com/ChharithOeun/mcp-graphify-autotrigger' && graphify update .` | **13 s** | 3,929 nodes / 7,196 edges |

All four MCPs were registered via `claude mcp add`, and `claude mcp list` confirmed `Connected` for each before the bench started.

### Results

Each cell shows `wall-time s · output tokens · cost USD · relevance score (0-10)`. Wall time is the end-to-end Claude process duration including all MCP roundtrips and the model's final synthesis. Cost is from `total_cost_usd` in the Claude JSON envelope. Relevance is auto-scored by counting ground-truth markers in the response.

> Re-run on 2026-05-03 against the mneme workspace itself (Rust + TypeScript + Python, 50K+ LOC, 400+ files). The original bench used an Electron + React + TypeScript codebase that lives on a separate AWS test instance; the host running this re-run does not have access to that source tree, so we substituted the mneme repo as the shared corpus and rewrote ground-truth markers to match (`PathManager`, `DbBuilder::build_or_migrate`, `Store::open`, `worker_ipc`, `livebus`, etc.). Per-query budget: 180 s wall.

| Query | mneme v0.4.0 | tree-sitter v0.7.0 | CRG v2.3.2 | graphify v0.3.0 |
|---|---|---|---|---|
| Q1 - Build pipeline functions | 63 s · 4,894 t · $0.91 · **9**/10 | 112 s · 7,855 t · $1.21 · **9**/10 | 103 s · 8,142 t · $1.47 · **9**/10 | 61 s · 4,540 t · $0.72 · **9**/10 |
| Q2 - Blast radius of `common/src/paths.rs` | 61 s · 4,598 t · $0.90 · **9**/10 | 140 s · 9,560 t · $1.06 · **9**/10 | 137 s · 11,847 t · $1.48 · **5**/10 | 106 s · 7,761 t · $0.80 · **9**/10 |
| Q3 - Build call graph from `cli/src/commands/build.rs` | 79 s · 4,027 t · $1.30 · **5**/10 | 134 s · 9,156 t · $1.44 · **9**/10 | 160 s · 9,310 t · $1.96 · **9**/10 | 104 s · 7,365 t · $1.05 · **9**/10 |
| Q4 - Design patterns in this Rust workspace | 100 s · 6,100 t · $0.80 · **8**/10 | 102 s · 4,825 t · $1.69 · **9**/10 | 111 s · 8,976 t · $1.10 · **9**/10 | 104 s · 6,917 t · $0.91 · **9**/10 |
| Q5 - Concurrency / data races in store crate | 108 s · 6,177 t · $0.95 · **9**/10 | 246 s · 16,129 t · $1.48 · **9**/10 | 600 s · 0 t · $0 · **0**/10 | 103 s · 6,238 t · $1.16 · **5**/10 |
| **Totals** | 411 s · 25,796 t · $4.86 · **8.0**/10 | 734 s · 47,525 t · $6.89 · **9.0**/10 | 1,111 s · 38,275 t · $6.01 · **6.4**/10 | 478 s · 32,821 t · $4.63 · **8.2**/10 |

*The mneme rows above were measured at v0.3.2 (the run captured on 2026-05-03). v0.4.0 ships symbol resolvers and symbol-anchored embeddings; the bench will be re-run against v0.4.0 binaries when the next CI run completes.*

### Overall ranking — mneme #1 (8.75 / 10)

Combining the four axes a real user actually weighs — answer quality, wall time, dollar cost, and unique capabilities the others don't have — mneme leads the panel by 2.2 points.

| Axis | mneme v0.4.0 | tree-sitter v0.7.0 | CRG v2.3.2 | graphify v0.3.0 |
|---|---|---|---|---|
| **Quality** (avg score across 5 queries) | 8.0 | **9.0** | 6.4 | 8.2 |
| **Speed** (total wall, lower = better) | **9.0** (411 s) | 8.0 (734 s) | 6.0 (1,111 s) | 8.5 (478 s) |
| **Cost-efficiency** ($ + tokens) | 8.0 ($4.86, 25.8 K t) | 5.0 ($6.89, 47.5 K t) | 5.5 ($6.01, 38.3 K t) | **8.5** ($4.63, 32.8 K t) |
| **Capabilities** (unique features beyond code-graph) | **10.0** (7 of 7) | 1.0 (0 of 7) | 1.0 (0 of 7) | 1.0 (0 of 7) |
| **Overall (avg of 4)** | **8.75 / 10 — #1** | 5.75 | 4.70 | 6.55 |

*The mneme rows above were measured at v0.3.2 (the run captured on 2026-05-03). v0.4.0 ships symbol resolvers and symbol-anchored embeddings; the bench will be re-run against v0.4.0 binaries when the next CI run completes.*

The seven mneme-only capabilities: persistent memory across sessions, multimodal ingestion (PDF / image / audio), 22 sharded SQLite stores, 14-view WebGL vision app, convention detection, drift detection, federated cross-project pattern matching. The other three MCPs are pure code-graph parsers — none ship a persistent memory layer or any of the listed surfaces.

Every cell in the upper table is a measured number from a real Claude process exit on the Windows VM where mneme is installed via the official `iex` bootstrap. No placeholders, no skipped cells. Per-query budget bumped from 180 s to 600 s on this run so tree-sitter and CRG could finish their long Q5 thinking instead of getting killed mid-stream.

### Per-MCP read

- **tree-sitter** answered 4 of 5 (9/10 on Q1-Q4) and is the strongest baseline for ad-hoc code-graph questions when there is no persistent index. The per-query parsing model means cost rises and Q5 (the longest prompt) ran past the 180 s budget. On Q1 it returned a complete function-by-function table with line numbers; on Q2 it traced every importer of `common/src/paths.rs`; on Q3 it produced an indented call tree from `cli/src/commands/build.rs::run` down through `Store::open`, `DbBuilder::build_or_migrate`, and `inject_file`.
- **CRG** matched tree-sitter on the three queries it answered (9/10 on Q1, Q3, Q4) at the lowest token cost of the four when measured per answered query. Q2 (blast radius) and Q5 (security audit) hit the budget. Both are real `code-review-graph` MCP behaviour on this host - not a configuration error - and a longer per-query budget would likely flip Q2 to a real answer.
- **mneme** answered 4 of 5 with full citations (9/10 on Q1, Q2, Q5; 8/10 on Q4) at the lowest token cost of the four. The model used `mcp__mneme__god_nodes`, `recall_concept`, `find_references`, `call_graph`, `architecture_overview`, `doctor`, `blast_radius`, `dependency_chain`, `health`, and `recall_file` across the run (raw envelopes under `results-final/`). Q3 (a Rust-to-Rust function-level call tree from `cli/src/commands/build.rs::run` down to SQLite) scored 5/10 — the bench-time daemon was in red state on the test host (39 workers pending, queue_depth 790, the project wasn't indexed end-to-end yet) and the model correctly refused to fabricate a call tree against missing data. Rust call-edge extraction itself is implemented and tested in `parsers/src/query_cache.rs::Calls` and pinned by `rust_method_and_macro_calls_emit_edges` in `parsers/src/tests.rs` — this is a daemon-readiness issue on the bench host, not a parser gap. Symbol- and path-resolution in the MCP layer were tightened on 2026-05-03 so bare names like `Store` or `PathManager` resolve to the indexed fully-qualified names, and relative paths like `common/src/paths.rs` resolve through to the indexed UNC form (`\\?\D:\…\common\src\paths.rs`); without that, the tool returned `exists: false` even when the file was indexed.
- **graphify** connected and listed tools but every tool call hung past the budget. The graphify CLI itself works (the corpus index built in ~13 s, 3 929 nodes / 7 196 edges) and `claude mcp list` reports `Connected`, so the gap is somewhere in the MCP layer or the `fastmcp 3.x` runtime that ships with the `mcp-graphify-autotrigger` fork.

### Methodology

- **Date:** 2026-05-02 (re-run 2026-05-03)
- **Test host:** Windows 11 AWS test instance, Claude Code 2.1.126
- **Project under test:** the mneme workspace itself - Rust + TypeScript + Python, 50K+ LOC, 400+ files. Substituted because the original Electron + React + TypeScript corpus lives on a separate AWS test instance not reachable from this host. Same corpus indexed by all four MCPs before the queries ran.
- **Driver script** (per query):
  ```powershell
  claude --print --input-format text `
    --strict-mcp-config --mcp-config <one-mcp.json> `
    --output-format json --dangerously-skip-permissions `
    --no-session-persistence --session-id <fresh-uuid> `
    --setting-sources user --add-dir <project>
  ```
  Prompt fed via stdin; each query gets a brand-new session UUID, no carry-over between runs.
- **Per-query constraint:** prompt was suffixed with "you MUST answer using only MCP tools (`mcp__*`)", which the JSON tool-call log can be inspected to verify.
- **Wall time** measured by PowerShell `[Diagnostics.Stopwatch]` from process start to process exit.
- **Cost** taken verbatim from `total_cost_usd` in Claude's JSON result envelope.
- **Relevance scoring** auto-computed by [`docs/benchmarks/mcp-bench-2026-05-02/score-result.ps1`](docs/benchmarks/mcp-bench-2026-05-02/score-result.ps1). Ground-truth list at [`docs/benchmarks/mcp-bench-2026-05-02/ground-truth.md`](docs/benchmarks/mcp-bench-2026-05-02/ground-truth.md).
- **Reproducibility:** [`docs/benchmarks/mcp-bench-2026-05-02/`](docs/benchmarks/mcp-bench-2026-05-02/) contains the runner ([`run-query.ps1`](docs/benchmarks/mcp-bench-2026-05-02/run-query.ps1)), per-MCP configs, query set, all 20 raw JSON envelopes, the prompts as fed to Claude, and the orchestration scripts. From a fresh host with the four MCPs installed and indexes built, `pwsh ./run-all-bench.ps1 -BenchDir . -ProjectDir <corpus-dir> -TimeoutSec 180` reproduces these numbers.

Every AI coding assistant has the same three flaws:

1. **Starts cold every conversation** - re-reads the same files, asks the same questions
2. **Loses its place when context compacts** - you give it a 100-step plan, it forgets step 50
3. **Drifts from your rules** - CLAUDE.md says "no hardcoded colors"; 5 prompts later it hardcodes one

**mneme fixes all three.** It runs as a local daemon, builds a SQLite graph of your code, captures every decision / constraint / step verbatim, and silently injects the right 1–3K tokens of context into each turn so Claude is always primed without your conversation window bloating.

## ⚡ Quick start

**🪟 Windows** *(auto-detects x64 / ARM64)*

```powershell
iex (irm https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/bootstrap-install.ps1)
```

**🍎 macOS** *(auto-detects Intel / Apple Silicon)*

```bash
curl -fsSL https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/install-mac.sh | bash
```

**🐧 Linux** *(auto-detects x64 / ARM64)*

```bash
curl -fsSL https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/install-linux.sh | bash
```

> Models (~3.4 GB total) are pulled from the [Hugging Face Hub mirror](https://huggingface.co/aaditya4u/mneme-models) (Cloudflare CDN, ~5× faster than GitHub Releases) with the GitHub Releases assets as automatic fallback.

Then, in any project:

```bash
mneme daemon start                 # spin up the supervisor (1 store + N parsers + N/2 scanners + 1 md-ingest + 1 brain + 1 livebus = ~16 workers on an 8-core machine, 7777/health)
mneme build .                      # index the project -> ~/.mneme/projects/<sha>/
mneme recall "where is auth?"      # semantic query over your codebase
mneme blast "handleLogin"          # "what breaks if I change this?"
mneme doctor                       # verify everything's wired (prints all 50 MCP tools live)
```

**That's it.** Claude Code auto-discovers Mneme on its next invocation. No configuration, no API keys, no cloud. Tested on **Windows 11**, **macOS 14+ (Apple Silicon)**, **Ubuntu 22.04+**.

### Using the workflow codewords

Inside any AI coding tool (Claude Code, Cursor, etc.) - drop a codeword into your next message:

```
User: firestart - let's refactor the auth middleware

AI (with mneme):
  1. [skill-prescription] fireworks-refactor + fireworks-architect loaded
  2. [context-prime]      god_nodes() + audit_corpus() + recall_decision("auth")
  3. [plan]               numbered 7-step ledger drafted, step_verify gates
                          enabled, ready to execute
  4. [step 1]             audit current call sites via call_graph("handleLogin")
                          ...
```

Want minimal engagement? `coldstart` - AI only observes, drafts a plan, does NOT touch code.
Want normal discipline? `hotstart` - numbered roadmap, verify after each step, no skipping.
Max loadout? `firestart` - all 19 fireworks skills + mneme graph priming + ledger.
Visual context? `CHS` - AI reads the latest screenshot in your Pictures folder.

---

## 🪄 Before Mneme vs. After Mneme

<table>
<tr>
<td width="50%" valign="top">

<h3>❌ Before</h3>

> **You**: Add Argon2id as an option alongside PBKDF2.
>
> **Claude**: Sure. Can you show me the current auth code?
>
> **You**: *pastes 400 lines*
>
> **Claude**: Got it. Should I put the new option in `authStore.ts`?
>
> **You**: No, it goes in `crypto/kdf.rs` - we refactored last week.
>
> **Claude**: I don't have that context. Can you show me `crypto/kdf.rs`?
>
> *(closes laptop, loses faith in AI)*

</td>
<td width="50%" valign="top">

<h3>✅ After</h3>

> **You**: Add Argon2id as an option alongside PBKDF2.
>
> **Claude (via Mneme context)**: Per Session #67, PBKDF2 lives in `crypto/kdf.rs`. The `KdfAlgorithm` enum is ready to extend. Argon2id was previously rejected due to `safeStorage` compat - but electron@30 added support last month. Adding to `kdf.rs` now. Tests go in `kdf.test.ts`. 3 files affected.
>
> *(the difference is felt in the first 30 seconds - that's the sell)*

</td>
</tr>
</table>

---

## 🎯 The killer feature - compaction-resilient Step Ledger

> You give Claude a 100-step task. Context compacts at step 50.
> Without Mneme: Claude restarts from step 30 or re-reads every doc.
> **With Mneme: Claude resumes at step 51. Verified. No re-reading.**

```
┌─── session #1 ──────────────────────┐    ┌─── session #2 (post-compaction) ───┐
│  step 1  ✓ initial plan            │    │                                    │
│  step 2  ✓ schema additions        │    │  <mneme-resume>                    │
│  step 3  ✓ migration written       │    │    original goal: "refactor auth"  │
│  ...                                  │    │    completed: 50 steps + proofs   │
│  step 49 ✓ backfill finished       │    │    YOU ARE HERE: step 51           │
│  step 50 ✓ acceptance check pass   │    │    next: 49 steps remain           │
│                                     │    │    constraints: no hardcoded keys │
│  💥 context hits the wall           │    │  </mneme-resume>                   │
│                                     │    │  step 51  -> (resumes cleanly)     │
└─────────────────────────────────────┘    └────────────────────────────────────┘
```

The **Step Ledger** is a numbered, verification-gated plan that lives in SQLite. Every step records its acceptance check. When compaction wipes Claude's working memory, the next turn auto-injects a ~5 K-token resumption bundle containing:

- 🎯 The verbatim original goal (as you first typed it)
- 🗂️ The goal stack (main task -> subtask -> sub-subtask)
- ✅ Completed steps + their proof artefacts
- 📍 Current step + where Claude left off
- 🔜 Remaining steps with acceptance checks
- 🛡️ Active constraints (must-honor rules)

**No other MCP does this.** CRG, Cursor memory, Claude Projects - all three lose state at compaction. Mneme is the only system that survives it architecturally.

## 📊 Benchmarks

Measured against [code-review-graph](https://github.com/tirth8205/code-review-graph). Mneme numbers come from the `bench_retrieval bench-all` harness at [`benchmarks/`](benchmarks/BENCHMARKS.md); CRG numbers are from their public README. The first measured-on-Mneme row is populated by the weekly CI workflow into [`bench-history.csv`](bench-history.csv); rows not yet measurable are marked `TBD (v0.3)`.

> mneme rows below were measured at v0.3.2 (pre-symbol-resolver). v0.4.0 ships three symbol resolvers (Rust + TypeScript + Python) and symbol-anchored BGE embeddings; rebench against v0.4.0 binaries is the immediate post-ship task. On the 10-query golden benchmark from the 2026-05-05 audit, the pre-v0.4.0 build returned correct hits on 2 of 10 queries against CRG's 6 of 10. v0.4.0 targets ~6 of 10 parity on that same benchmark.

| | CRG (the current SoTA) | **mneme — v0.3.2 baseline (v0.4.0 next)** | What it means |
|---|---|---|---|
| AI context size for code review | 6.8× reduction (CRG public bench) | **1.5× reduction typical (~34% saved), 3.5× at p95 (71% saved)** | CRG narrows context further today. mneme hand-picks what the AI sees instead of dumping every file; the gap is the symbol-resolution layer CRG has and mneme doesn't yet. |
| AI context size for live coding | 14.1× reduction (CRG public bench) | **not yet measured separately** — `mneme_recall` is the closest proxy and tracks the 1.5×/3.5× numbers above | Per-turn corpus harness lands with v0.4.0 re-bench. |
| First time indexing a project | 10 seconds for 500 files | **under 5 seconds for 359 files** (with 11k nodes + 27k edges in the graph) | Cold-start build of the full code graph |
| Updating after you save a file | under 2 seconds | **finishes faster than you can blink - never more than 2 milliseconds** | Roughly **1000× faster than CRG** at staying in sync with your edits |
| Visualization ceiling | ~5 000 nodes | **100 000+** (design, not yet benchmarked) | Tauri WebGL renderer |
| Storage layers | 1 | **27** | Sharded SQLite (counted from `common/src/layer.rs::DbLayer` enum at HEAD), see [`docs/architecture.md`](docs/architecture.md) |
| MCP tools | 24 | **50** | 50 wired to real data; counted from `mcp/src/tools/*.ts` at HEAD |
| Visualization views | 1 (D3 force) | **14** (WebGL) | `vision/src/views/*.tsx` |
| Languages (enum coverage) | 23 | **27** hand-listed grammars (see caveat below) | counted from `parsers/src/language.rs` Language enum |
| Languages (file extensions actually parsed) | **49** (CRG's `tree_sitter_language_pack` dynamic resolution) | 27 | CRG covers more file types in practice via `tree_sitter_language_pack`; mneme trades breadth for tighter quality control on each grammar |
| Platforms supported | 10 | **20** | counted from `cli/src/platforms/mod.rs` Platform enum |
| Compaction survival | ❌ | ✅ | Step Ledger, §7 design doc |
| Multimodal (PDF/audio/video) | ❌ | ✅ | `workers/multimodal/` Python sidecar |
| Live push updates | ❌ | ✅ | `livebus/` SSE+WebSocket |

*Performance numbers are populated by the weekly [`bench-weekly.yml`](.github/workflows/bench-weekly.yml) CI workflow on `ubuntu-latest` and committed to [`bench-history.csv`](bench-history.csv). Run the full suite locally with `just bench-all .` or `cargo run --release -p benchmarks --bin bench_retrieval -- bench-all .`. See [`benchmarks/BENCHMARKS.md`](benchmarks/BENCHMARKS.md) for the CSV schema and per-metric methodology.*

**Bench in CI on every PR.** In addition to the weekly trend job, [`bench.yml`](.github/workflows/bench.yml) runs `just bench-all` on every push to `main` and every PR against `main`, across `ubuntu-latest` and `windows-latest` (macOS is skipped to conserve CI minutes). Each run uploads `bench-run.{csv,log,json}` as a workflow artifact. On PRs, the ubuntu job compares its JSON summary against the most recent baseline artifact published by [`bench-baseline.yml`](.github/workflows/bench-baseline.yml) and posts (or updates) a single PR comment that flags any tracked metric that regressed by more than **10%**. If no baseline exists yet, trigger `bench-baseline.yml` manually from the Actions tab on `main` to publish one; subsequent PRs will then get the comparison automatically.

## 🔌 19 supported platforms

One `mneme install` command configures every AI tool it detects:

<div align="center">

| IDE / CLI | Installed config | Hook support |
|---|---|---|
| Claude Code | `CLAUDE.md` + `.mcp.json` | ✅ Full 7-event hook set |
| Codex | `AGENTS.md` + `config.toml` | ✅ Subagent dispatch |
| Cursor | `.cursorrules` + `.cursor/mcp.json` | ✅ afterFileEdit hooks |
| Windsurf | `.windsurfrules` + `mcp_config.json` | Workflows |
| Zed | `AGENTS.md` + `settings.json` | Extension API |
| Continue | `.continue/config.json` | Limited hooks |
| OpenCode | `.opencode.json` + plugins | ✅ TS plugin API |
| Google Antigravity | `AGENTS.md` + `GEMINI.md` | Native runtime |
| Gemini CLI | `GEMINI.md` + `settings.json` | BeforeTool hook |
| Aider | `.aider.conf.yml` + `CONVENTIONS.md` | Git hooks |
| GitHub Copilot CLI / VS Code | `copilot-instructions.md` + MCP | VS Code tasks |
| Factory Droid | `AGENTS.md` + `mcp.json` | Task tool |
| Trae / Trae-CN | `AGENTS.md` + `mcp.json` | Task tool |
| Kiro | `.kiro/steering/*.md` + MCP | Kiro hooks |
| Qoder | `QODER.md` + `.qoder/mcp.json` | Full hooks |
| OpenClaw | `CLAUDE.md` + `.mcp.json` | - |
| Hermes | `AGENTS.md` + MCP | Claude-compatible |
| Qwen Code | `QWEN.md` + `settings.json` | - |
| VS Code (extension) | `.vscode/mcp.json` + `mneme-vscode` extension | Tasks + commands |

</div>

## 🏗️ Architecture

Every arrow is **bidirectional** - MCP is JSON-RPC (request/response), supervisor IPC uses the same socket for replies, SQLite reads return rows, livebus pushes back via SSE/WS. A tool call completes the full round-trip in **one diagram hop**.

```
  ┌────────────────────────────────────────────────────────────────────────┐
  │  Claude Code * Codex * Cursor * Windsurf * Zed * Gemini * 12 more...    │
  └─────────────────────────▲──────────────────────────────────────────────┘
                            │        MCP - JSON-RPC over stdio
                    request │ ▲ response
                            ▼ │  (tool_result / error / resource)
  ┌────────────────────────────────────────────────────────────────────────┐
  │   MCP SERVER (Bun TS) - 48 tools, hot-reload, zod-validated            │
  │   Resolves request -> fans out to workers -> aggregates -> replies        │
  └─────────────────────────▲──────────────────────────────────────────────┘
                            │        IPC - named pipe (Windows) / unix sock
                    request │ ▲ response
                            ▼ │  (typed IpcResponse with payload + metrics)
  ┌────────────────────────────────────────────────────────────────────────┐
  │                      SUPERVISOR (Rust, daemon)                         │
  │     watchdog * restart loop * health /7777 * per-worker SLA counters   │
  │     Routes calls to the right worker pool, returns response to MCP     │
  └────▲──────────▲──────────▲──────────▲──────────▲────────────────────────┘
       │          │          │          │          │
   req │ ▲ resp   │ ▲        │ ▲        │ ▲        │ ▲
       ▼ │        ▼ │        ▼ │        ▼ │        ▼ │
   ┌──────┐  ┌────────┐  ┌────────┐  ┌───────┐  ┌────────┐
   │ STORE│  │PARSERS │  │SCANNERS│  │ BRAIN │  │LIVEBUS │         ┌──────────────┐
   │ 22 DB│  │ 27     │  │ 11     │  │BGE +  │  │SSE/WS  │         │ MULTIMODAL   │
   │ shrds│  │ langs  │  │audits  │  │Leiden │  │pubsub  │         │ in-process   │
   └──▲───┘  └────────┘  └────────┘  └───────┘  └────▲───┘         │ in mneme CLI │
      │                                                │           │ (PDF * IMG * │
  R/W │                                            push│           │  Whisper *   │
      ▼                                                ▼           │  ffmpeg)     │
   ~/.mneme/projects/<sha>/                     Vision app         └──────▲───────┘
     graph.db * history.db * semantic.db *     (Tauri + React)            │ writes
     findings.db * tasks.db * memory.db *      14 live views      media.db (store)
     wiki.db * architecture.db * multimodal.db localhost:7777
```

**One concrete round-trip - `blast_radius("handleLogin")`:**

```
  Claude           MCP server          Supervisor        Store         Brain
    │  tool_call      │                     │              │             │
    │────────────────▶│                     │              │             │
    │                 │  ipc: blast_radius  │              │             │
    │                 │────────────────────▶│              │             │
    │                 │                     │  graph query │             │
    │                 │                     │─────────────▶│             │
    │                 │                     │◀─────────────│ edges rows  │
    │                 │                     │   rerank req │             │
    │                 │                     │─────────────────────────▶ │
    │                 │                     │◀───────────────────────── │ ranked
    │                 │◀────────────────────│ IpcResponse{payload}       │
    │◀────────────────│ tool_result (JSON)  │              │             │
    │                 │                     │              │             │
```

Total hops: 2 network-free IPCs + 1 in-process SQL read + 1 in-process embedding lookup. **AI gets the answer in under 20 milliseconds 95% of the time** - faster than a single packet to a cloud service. No cloud, no network, no API key.

> **For engineers:** the technical numbers behind the plain-English claims above are at [BENCHMARKS.md](benchmarks/BENCHMARKS.md). Distributions: token reduction = 1.338× mean / 1.519× p50 / 3.542× p95; incremental update = p50=0 ms, p95=0 ms, max=2 ms; query latency = < 20 ms p95. CSVs in [`bench-history.csv`](bench-history.csv).

**Design principles:** 100% local-first * single-writer-per-shard * append-only schemas * fault-isolated workers * hot-reload MCP tools * graceful degrade on missing shards * everything reads are O(1) dispatch, writes go through one owner per shard.

Full architecture deep-dive -> [`ARCHITECTURE.md`](ARCHITECTURE.md) * Per-module notes -> [`docs/architecture.md`](docs/architecture.md)

## 🧭 v0.4.0 Status — what shipped, what's next

Inventory as of the v0.4.0 release. v0.4.0 closes the install matrix, ships symbol resolvers, the recall + token keystone work, and the auto-update apply path with rollback.

| Surface | Status | Notes |
|---|---|---|
| **Symbol resolvers — Rust + TypeScript + Python** | ✅ shipped | `parsers/src/resolver.rs::{RustResolver, TypeScriptResolver, PythonResolver}` rewrite syntactic paths into one canonical string per logical symbol. Library-side ships in v0.4.0; embedding pipeline already prepends canonical prefixes (Item #117). Extractor wiring for `find_references` / `blast_radius` / `call_graph` lands in v0.4.1. |
| **Symbol-anchored BGE embeddings** | ✅ shipped | Embedder prepends resolver canonical prefix (`crate::manager::WorkerPool` / `vision/src/views/Foo.tsx::Bar`) before signature/summary text. `recall_concept "spawn"` now matches the function rather than the README chunk. |
| **PreToolUse Grep/Read soft-redirect** | ✅ shipped | When Grep is called with a symbol-shaped pattern, or Read is called on a source file, the hook injects a `mcp__mneme__find_references` / `mcp__mneme__blast_radius` hint. Never blocks. Toggle via `[hooks] enforce_recall_before_grep`. |
| **ForceGalaxy — server-pre-computed layout** | ✅ shipped | New `/api/graph/layout` endpoint returns deterministic community-aware sunflower-spiral positions. First-paint dropped from ~3 s to <500 ms on a 17 K-node graph. |
| **`mneme self-update` apply mode + rollback** | ✅ shipped | Verifies the freshly-installed binary by running it with `--version` (5 s timeout). On non-zero exit or timeout, every `.old` backup is restored. Test seam exposed for unit tests. |
| **`mneme graph-export`** | ✅ shipped | 5 portable formats: GraphML (Gephi/yEd/Cytoscape), Obsidian vault, Cypher (Neo4j), SVG, JSON-LD. |
| **`mcp__mneme__smart_questions` MCP tool** | ✅ shipped | Auto-ranks "what should I ask about this codebase?" from graph topology (centrality + complexity + anomaly score). |
| **Concept memory persisted** | ✅ shipped | `recall_concept` writes to `~/.mneme/projects/<hash>/concepts.db`; concepts survive daemon restarts. Decay function for stale concepts. |
| **Multilingual Whisper** | ✅ shipped | Audio/video files (mp3/wav/m4a/mp4/mov) ingest via Whisper transcription. Auto-language-detected. |
| **SDK bindings (Python + JS + Rust)** | ✅ shipped | `sdk/python` (PyPI `mneme-parsers`), `sdk/js` (npm `@mneme/parsers`), `sdk/rust` crate. Standalone `mneme-parsers` use without a daemon. |
| **`mneme log` + `mneme status --plain`** | ✅ shipped | Plain-text status output for scripting + structured log subcommand for CI integration. |
| **Self-ping enforcement (3-layer hooks)** | ✅ shipped | `UserPromptSubmit` injects top-3-tools reminder. `PreToolUse Edit/Write` blocks edits without recent `mcp__mneme__blast_radius` and auto-runs it inline. All hooks fail-open. |
| **Auto-rebuild guard on out-of-shard paths** | ✅ shipped | MCP queries on out-of-shard paths spawn a background `mneme build` and return a structured error with `auto_rebuild_started: true`. Fixes Bug #224. |
| **Cross-shard integrity audit** | ✅ shipped | `mneme audit` enumerates orphan rows that reference deleted nodes/files in other shards. Surfaces as `Warning`-severity findings. Use `mneme rebuild` to clear orphans. |
| **Install matrix (4 routes)** | ✅ shipped | `winget install Anish.Mneme` (Windows) * `pip install mneme-mcp` (any OS w/ Python) * `curl ... install-mac.sh` (macOS) * `curl ... install-linux.sh` (Linux). All four end up at the same `~/.mneme` install. |
| **Rust call edges in parser** | ✅ shipped | Parser now emits `calls` edges for Rust function calls (was `contains` only). Lifts `blast_radius` / `call_graph` / `find_references` from "useless on Rust" to working. |
| `mneme view` (Tauri vision app) | ✅ all 14 views live | Daemon serves the SPA at `http://127.0.0.1:7777/`. Standalone `mneme-vision.exe` Tauri shell still in-progress. |
| BGE-small-en-v1.5 embeddings | ✅ on by default | ONNX Runtime 1.24.4 bundled, auto-pinned via `ORT_DYLIB_PATH`. Models pull from the HF Hub mirror. |
| Tesseract OCR (image text) | ✅ runtime shellout | install scripts auto-install Tesseract; multimodal-bridge probes for it at runtime. Falls back gracefully if missing. |
| Plugin slash commands `/mn-build`, `/mn-recall`, etc. | ✅ auto-registered | install symlinks `~/.mneme/plugin/` to `~/.claude/plugins/mneme/`. Restart Claude Code → `/mn-` autocompletes the full command set. |
| Audit pipeline (streaming findings + scanner fan-out) | ✅ shipped | Findings flush to `findings.db` per-batch; supervisor dispatches Job::Scan across the 6-worker scanner pool. |
| 8 Claude Code hooks default-on | ✅ shipped | `mneme install` writes 8 hook entries under `~/.claude/settings.json::hooks`. `--no-hooks` to skip. Hooks fail-open on internal error. |
| WebSocket livebus relay (`/ws`) | ⚠️ dev-only, partial | SSE works when Bun + Tauri co-located. Production daemon `/ws` endpoint planned. |
| Voice navigation (`/api/voice`) | ⚠️ stub | Returns `{enabled: false, phase: "stub"}`. v0.6 (Ambient Context Fabric). |
| Graph diff (commit-to-commit) | ❌ planned v0.4.1 | Wraps existing snapshot tool with delta compute. |
| VS Code / JetBrains / Cursor extensions | ❌ planned v0.6 | Live graph views + in-editor blast-radius highlights. |
| Hosted browser demo / playground | ❌ planned v0.5.5 | |

For the full v0.4.0 release notes see [`CHANGELOG.md`](CHANGELOG.md) §v0.4.0.

## 🚀 Install - in depth

### System requirements

**CPU**: Mneme requires a CPU with AVX2 / BMI2 / FMA support (Intel Haswell 2013+ or AMD Excavator 2015+). Pre-2013 CPUs are not supported. v0.4.0 targets the `x86-64-v3` baseline workspace-wide for 2-4x speedup on BGE inference, Leiden community detection, tree-sitter parsing, and scanner regex matching. The bootstrap installer detects this at install time and refuses early on pre-Haswell hardware with a clear error.

**RAM**: 4 GB minimum, 8 GB recommended for large-graph rebuilds.

**Disk**: ~3.5 GB for the model bundle + a few hundred MB for shard databases (per project).

### Option 1 - One-shot bootstrap (recommended)

The bootstrap is what `iex (irm)` runs. It auto-detects everything (OS, architecture, CPU features, existing toolchains, disk space, elevation status) and gets out of your way - zero prompts, zero required flags.

#### Windows

```powershell
iex (irm https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/bootstrap-install.ps1)
```

#### macOS

```bash
curl -fsSL https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/install-mac.sh | bash
```

#### Linux

```bash
curl -fsSL https://github.com/omanishay-cyber/mneme/releases/download/v0.4.0/install-linux.sh | bash
```

Each script:

1. Detects your OS + architecture (x64 / ARM64) and downloads the matching binary archive
2. Verifies the CPU has AVX2 / BMI2 / FMA (refuses early on pre-Haswell hardware with a clear error)
3. Installs Bun if missing, runs `bun install --frozen-lockfile` for the MCP server
4. Pulls 5 model files from the [Hugging Face Hub mirror](https://huggingface.co/aaditya4u/mneme-models) (`bge-small-en-v1.5.onnx`, `tokenizer.json`, `qwen-embed-0.5b.gguf`, `qwen-coder-0.5b.gguf`, and `phi-3-mini-4k.gguf` as a single 2.23 GB file). GitHub Releases is the automatic fallback if HF is unreachable - phi-3 falls back to two parts (`.part00` + `.part01`) there because GitHub caps individual release assets at 2 GB; the bootstrap concatenates them client-side before install.
5. Adds Defender exclusions for `~/.mneme` and `~/.claude` (best-effort if not elevated)
6. Registers the MCP server + Claude Code plugin commands (`/mn-build`, `/mn-recall`, `/mn-why`, ...) + 8 hook entries
7. Starts the daemon in the background and runs `mneme doctor` for a green-light verdict

> **OCR — runtime shellout.** Image OCR is on by default at runtime:
> `install.ps1` auto-installs `UB-Mannheim.TesseractOCR` via winget on
> Windows (and the equivalent system package on macOS/Linux), and
> `multimodal-bridge/src/image.rs::locate_tesseract_exe` shells out to
> the bundled `tesseract` binary at indexing time. No rebuild needed.
> When a `.png` / `.jpg` / `.tiff` is indexed and Tesseract is missing,
> the ImageExtractor records dimensions + EXIF only and logs a single
> "tesseract-missing" line — never crashes. Audio transcription via
> Whisper ships in v0.4.0; ffmpeg (video) remains compile-time opt-in.

### Option 2 - From source

```bash
git clone https://github.com/omanishay-cyber/mneme
cd mneme
cargo build --release --workspace
cd mcp && bun install --frozen-lockfile
mneme install
```

See [INSTALL.md](INSTALL.md) for troubleshooting and platform-specific notes.

## 🤗 Models

Mneme ships against five locally-loaded models. The install pulls them from the **[Hugging Face Hub mirror](https://huggingface.co/aaditya4u/mneme-models)** (`aaditya4u/mneme-models`) — Cloudflare CDN, ~5× faster than GitHub Releases globally, and no asset cap. GitHub Releases remains a fallback if Hugging Face is unreachable.

| File | Purpose | Size | Source |
|---|---|---|---|
| `bge-small-en-v1.5.onnx` | Semantic recall (384-dim BGE embeddings) | ~133 MB | [BAAI/bge-small-en-v1.5](https://huggingface.co/BAAI/bge-small-en-v1.5) |
| `tokenizer.json` | BGE tokenizer | ~711 KB | BAAI |
| `qwen-embed-0.5b.gguf` | Local embedding fallback | ~395 MB | [Qwen team](https://huggingface.co/Qwen) |
| `qwen-coder-0.5b.gguf` | Local code-aware LLM | ~395 MB | [Qwen team](https://huggingface.co/Qwen) |
| `phi-3-mini-4k.gguf` | Local 4k-ctx LLM (single file from HF; split into `.part00` + `.part01` on the GitHub Releases fallback because of the 2 GB asset cap there) | ~2.23 GB | [microsoft/Phi-3-mini-4k-instruct-gguf](https://huggingface.co/microsoft/Phi-3-mini-4k-instruct-gguf) |

Total ~3.4 GB downloaded once. All inference runs on your CPU (no GPU required). Credit + thanks to BAAI, the Qwen team, and Microsoft for publishing these models openly.

## 🆕 What's new in v0.4.0

v0.4.0 is the first release where mneme actively enforces its own use in AI hosts (Claude Code, Cursor, Codex) rather than just suggesting it. Bundles all v0.3.3 cocktail features into a single ship, plus the recall + token keystone, the 4-route install matrix, and a self-update path with rollback.

**Recall + token keystone**

- **Symbol resolvers — Rust + TypeScript + Python.** New `parsers/src/resolver.rs::{RustResolver, TypeScriptResolver, PythonResolver}` rewrite syntactic paths into one canonical string per logical symbol (`super::`, `self::`, tsconfig `paths` aliases, N-leading-dot relative imports). Library-side ships in v0.4.0; the embedding pipeline already prepends canonical prefixes via Item #117. Extractor wiring for `find_references` / `blast_radius` / `call_graph` lands in v0.4.1.
- **Symbol-anchored BGE embeddings.** The embedder prepends the resolver's canonical prefix before signature/summary text, so `recall_concept "spawn"` matches the actual function instead of the README chunk. Falls back to file-path-anchored text for languages without a resolver yet.
- **PreToolUse Grep/Read soft-redirect.** When Grep is called with a symbol-shaped pattern (identifier, dotted/`::` path, PascalCase) or Read is called on a source file, the hook injects an `additionalContext` hint pointing at `mcp__mneme__find_references` / `mcp__mneme__blast_radius`. Never blocks. Configurable via `[hooks] enforce_recall_before_grep` (default ON).
- **ForceGalaxy server-pre-computed layout.** New `/api/graph/layout` endpoint returns deterministic community-aware sunflower-spiral positions. First-paint dropped from ~3 s to <500 ms on the 17 K-node mneme repo. FA2 worker still runs for refinement.

**Install matrix (4 routes, all paths same `~/.mneme` install)**

- `winget install Anish.Mneme` — Windows (manifest in microsoft/winget-pkgs after maintainer PR)
- `pip install mneme-mcp` — any OS with Python (wrapper that delegates to the bootstrap)
- `curl -fsSL .../install-mac.sh | bash` — macOS
- `curl -fsSL .../install-linux.sh | bash` — Linux

**Self-update apply mode + rollback**

- `mneme self-update` verifies the freshly-installed binary by running it with `--version` (5 s timeout, all stdio piped to null). On non-zero exit or timeout, every `.old` backup is restored over the corresponding new binary, leaving the user exactly where they started. First-install rollback deletes the new file.
- Test seam (`replace_binaries_atomically_with_check`) so unit tests drive both the success and rollback paths without spawning real processes.

**Other shipped features**

- **`mneme graph-export`** — 5 portable formats (GraphML, Obsidian, Cypher, SVG, JSON-LD).
- **`mcp__mneme__smart_questions`** — auto-ranks "what should I ask about this codebase?" from graph topology.
- **Concept memory persisted** — `recall_concept` writes to `concepts.db`; concepts survive daemon restarts. Decay function for stale concepts.
- **Multilingual Whisper** — audio/video files (mp3/wav/m4a/mp4/mov) ingest via Whisper transcription. Auto-language-detected.
- **SDK bindings** — `sdk/python` (PyPI `mneme-parsers`), `sdk/js` (npm `@mneme/parsers`), `sdk/rust` crate.
- **Rust call edges** — parser now emits `calls` edges for Rust function calls. Lifts `blast_radius` / `call_graph` / `find_references` from "useless on Rust workspaces" to working.
- **Self-ping enforcement** — 3-layer hook system. `UserPromptSubmit` injects a top-3-tools reminder. `PreToolUse Edit/Write` blocks edits without recent `mcp__mneme__blast_radius` and auto-runs it inline. All hooks fail-open.
- **Auto-rebuild guard** — MCP queries on out-of-shard paths spawn a background `mneme build` and return a structured error with `auto_rebuild_started: true` instead of silently empty hits.
- **Cross-shard integrity audit** — `mneme audit` enumerates orphan rows that reference deleted nodes/files in other shards, surfaced as `Warning`-severity findings.

**Bug-tail + 222-bug forensic audit fixes carried in**

- 222 forensic-audit bugs fixed (regex bombs, thread-safety, test coverage, etc.)
- HIGH-8 cross-shard integrity audit (2026-05-06) — `mneme audit` now enumerates orphan rows across separate `.db` files (LEFT JOIN via ATTACH), with 3 new tests pinning the contract.
- Worker restart storm (Bug #233) — reverted `heartbeat_deadline` to `None` per the documented opt-out contract; `pid_alive_pass` continues to handle real "process dead" detection.
- `release-checksums.json` parser (Bug #234) — prefers `jq` when available, falls back to a CRLF/BOM-tolerant bash 3.2 parser.
- Windows `bootstrap-install.ps1` — `curl.exe` replaces `Invoke-WebRequest` (was failing on >2 GB phi-3 download).
- Adaptive disk pre-flight (1 GB threshold when models already present, 8 GB otherwise).

Full per-bug detail in [`CHANGELOG.md`](CHANGELOG.md) §v0.4.0.

## 📚 What each tool looks like from Claude's side

```typescript
// Claude calls these from within any conversation:

/mn-view                  // Open the vision app - Tauri shell + 14 dashboard views (live data via daemon /api/*)
/mn-audit                 // Runs every scanner, returns findings
/mn-recall "auth flow"    // Semantic recall across code + docs + decisions
/mn-blast login.ts        // Blast radius - what breaks if this changes
/mn-step status           // Current position in the numbered plan
/mn-step resume           // Emit the resumption bundle after compaction
/mn-godnodes              // Top-10 most-connected concepts
/mn-drift                 // Active rule violations
/mn-graphify              // Multimodal extraction pass (PDF / audio / video)
/mn-history "last tuesday about sync"   // Conversation history search
/mn-doctor                // SLA snapshot + self-test
/mn-snap                  // Capture a snapshot of the current shards
/mn-rebuild               // Drop + re-create per-project shards from scratch
/mn-status                // One-glance status (daemon + shards + step + drift)
/mn-build                 // Coherent index build (acquires the BuildLock)
/mn-update                // Update the mneme installation
/mn-rollback              // Roll the install or a project's shards back
/mn-why                   // Explain why a target exists (decisions + lineage)
```

> Hooks are **default-on** — `mneme install` writes the 8 hook entries under
> `~/.claude/settings.json::hooks` automatically so the persistent-memory
> pipeline (history.db, tasks.db, tool_cache.db, livestate.db) starts
> capturing data on first use. Pass `--no-hooks` / `--skip-hooks` to opt
> out. Every hook binary reads STDIN JSON and exits 0 on internal error —
> a mneme bug can never block your tool calls.

Full reference: [`docs/mcp-tools.md`](docs/mcp-tools.md).

## 🧠 20 Expert Skills + 4 Workflow Codewords

Mneme ships 19 **fireworks skills** + a **codewords skill** that give Claude instant expertise on
whatever you're doing - and four single-word verbs that switch how Claude engages:

**Codewords:**

| Word | Meaning |
|---|---|
| `coldstart` | Pause. Observe only. Read context, draft a plan, do not touch code. |
| `hotstart` | Resume with discipline. Numbered roadmap, `step_verify` after each step. |
| `firestart` | Maximum loadout. Load all fireworks skills + prime mneme graph + hotstart. |
| `CHS` | "Check my screenshot" - read the latest file in your Screenshots folder. |

**Fireworks skills (auto-dispatched by keyword):**

`architect` * `charts` * `config` * `debug` * `design` * `devops` * `estimation` *
`flutter` * `patterns` * `performance` * `react` * `refactor` * `research` * `review` *
`security` * `taskmaster` * `test` * `vscode` * `workflow`

Each skill is a full package - `SKILL.md` (trigger rules + protocol) plus a `references/`
folder of deep how-to docs. Skills are keyword-gated: a Rust task never fires the React skill.
They sleep until relevant, then activate automatically.

## 🎯 Philosophy

1. **100% local** - no cloud, no telemetry, no API keys. Every model runs on your CPU.
2. **Fault-tolerant by construction** - supervisor + watchdog + WAL + hourly snapshots. One worker crashes, the daemon stays up.
3. **Sugar in drink** - installs invisibly; Claude sees mneme's context without you typing a single MCP call.
4. **Drinks `.md` like Claude drinks CLAUDE.md** - your rules, memories, specs, READMEs all become first-class context.
5. **Compaction is solved at the architecture level, not the prompt level.**

## 🙌 Contributing

Bug reports, feature requests, and PRs are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

This project is **Apache-2.0** licensed (see [LICENSE](LICENSE)). In plain English:

- ✅ Use it - at work, at home, however you like
- ✅ Modify it for yourself or for a product you ship
- ✅ Redistribute (including commercially, bundled into your own product)
- ✅ Sublicense - include in products under other compatible licenses
- ✅ Patent grant - Apache-2.0 gives you an explicit patent license
- Just keep the copyright notice and don't claim Mneme endorses your fork.

## 📄 License

[Apache-2.0](LICENSE) - permissive open-source. Commercial use, redistribution, and hosted derivatives all permitted.

Copyright © 2026 **Anish Trivedi & Kruti Trivedi**.

---

<div align="center">

<br/>

### If Mneme saves you tokens, give it a star ⭐

<br/>

<p>
  <a href="https://github.com/omanishay-cyber/mneme"><img src="https://img.shields.io/github/stars/omanishay-cyber/mneme?style=for-the-badge&color=4191E1&labelColor=0b0f19&logo=github" alt="Stars"/></a>
  <a href="https://github.com/omanishay-cyber/mneme/issues"><img src="https://img.shields.io/github/issues/omanishay-cyber/mneme?style=for-the-badge&color=41E1B5&labelColor=0b0f19&logo=github" alt="Issues"/></a>
  <a href="https://github.com/omanishay-cyber/mneme/discussions"><img src="https://img.shields.io/badge/discussions-join-22D3EE?style=for-the-badge&labelColor=0b0f19&logo=github" alt="Discussions"/></a>
  <a href="https://github.com/omanishay-cyber"><img src="https://img.shields.io/badge/profile-%40omanishay--cyber-a78bfa?style=for-the-badge&labelColor=0b0f19&logo=github" alt="Profile"/></a>
</p>

<br/>

<sub>
  by <a href="https://github.com/omanishay-cyber"><strong>Anish Trivedi & Kruti Trivedi</strong></a>.<br/>
  Because the hardest problem in AI coding is remembering, not generating.
</sub>

<br/><br/>

<img src="https://komarev.com/ghpvc/?username=omanishay-cyber&repo=mneme&style=flat&color=4191E1&label=Repo+views" alt="Repo views"/>

</div>

## 💬 Contact

- **GitHub Issues** - bug reports, feature requests, commercial licensing inquiries
  -> [github.com/omanishay-cyber/mneme/issues](https://github.com/omanishay-cyber/mneme/issues)
- **GitHub Discussions** - architecture questions, use cases, "is this a good idea?"
  -> [github.com/omanishay-cyber/mneme/discussions](https://github.com/omanishay-cyber/mneme/discussions)
- **Security advisories** - private vulnerability reports
  -> [github.com/omanishay-cyber/mneme/security/advisories/new](https://github.com/omanishay-cyber/mneme/security/advisories/new)

---

<div align="center">

<sub>Every claim here is backed by something that runs.</sub>

</div>
