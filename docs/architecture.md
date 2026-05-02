# mneme architecture

The 10-minute read on how mneme is built, in plain English, without exposing the internal design plan.

Current version: **v0.3.2** (hotfix, 2026-05-02).

## Mental model in one paragraph

mneme is a **local daemon** that **indexes your project into a SQLite graph** and **feeds Claude exactly the right slice of that graph at every turn**. The daemon runs as a supervisor that spawns 22 worker processes (auto-scaled to your CPU count) for parsing, scanning, embedding, pushing live events, and bridging Python. An MCP server speaks JSON-RPC to Claude (or Codex, Cursor, etc.) and hits the graph via direct `bun:sqlite` reads or through the supervisor for writes. A Step Ledger stored in the graph is what lets Claude survive context compaction - it's just numbered rows in SQLite with verification commands attached.

## The moving parts

```
┌─────────────────────────────────────────────────────────────────────┐
│                      SUPERVISOR (Rust)                              │
│  watchdog * restart * SLA * HTTP /health * 22 workers auto-scaled   │
└──┬──────────┬───────────┬──────────┬──────────┬──────────┬─────────┘
   │          │           │          │          │          │
   ▼          ▼           ▼          ▼          ▼          ▼
┌──────┐ ┌────────┐ ┌─────────┐ ┌─────────┐ ┌──────┐ ┌──────────┐
│STORE │ │PARSERS │ │SCANNERS │ │MD-INGEST│ │BRAIN │ │ LIVE BUS │
│(Rust)│ │(Rust)  │ │(Rust)   │ │(Rust)   │ │(Rust)│ │(Rust)    │
│  x1  │ │ xCPU   │ │xCPU/2   │ │  x1     │ │ x1   │ │  x1      │
└──────┘ └────────┘ └─────────┘ └─────────┘ └──────┘ └──────────┘
   │
   ▼  (single-writer, many-reader)
~/.mneme/projects/<sha>/
   ├─ graph.db          ← Tree-sitter-parsed nodes + edges
   ├─ history.db        ← conversation turns + decisions
   ├─ tasks.db          ← Step Ledger
   ├─ findings.db       ← scanner output (streams incrementally)
   ├─ semantic.db       ← embeddings + concepts
   └─ (17 more layer-specific shards, 22 total + meta.db)

Separately:
┌─────────────┐     ┌──────────────┐    ┌───────────────────────┐
│ MCP server  │     │ Vision app   │    │ Multimodal sidecar    │
│ (Bun TS)    │     │ (Bun TS SPA) │    │ (Python)              │
│             │     │              │    │                       │
│ 48 tools    │     │ 14 views     │    │ PDF / Whisper / OCR   │
│ JSON-RPC    │     │ WebGL        │    │ msgpack over stdio    │
│ over stdio  │     │              │    │                       │
└──────┬──────┘     └──────┬───────┘    └───────────┬───────────┘
       │                   │                         │
       │ bun:sqlite        │ WebSocket               │ spawned by
       │ read-only         │ to Live Bus             │ multimodal-bridge
       ▼                   ▼                         ▼
   [ same shards ]     [ live updates ]         [ async jobs ]
```

> The Tauri shell that previously wrapped the Vision app is dev/build-only as of
> v0.3.2 - the shipped Vision surface is the Bun-served SPA at
> `http://127.0.0.1:7777/`.

## Design principles (the ones worth knowing)

### 1. Single writer per shard, unlimited readers

SQLite in WAL mode supports unlimited concurrent readers while a single writer holds the write lock. mneme enforces this by routing every write through the store-worker process (over an MPSC channel) and letting any reader open the shard directly. This eliminates the entire class of "database is locked" errors.

The Rust code calls this the **Single-Writer Invariant**. Do not bypass it.

### 2. Fault domains are OS processes

Each worker (parsers, scanners, brain, livebus, store, md-ingest) runs as a separate OS process supervised by the root daemon. When one crashes, the supervisor captures a log entry and restarts it without affecting the others. The MCP server you talk to via Claude is a *different* process from the supervisor - if you only want the MCP server, it runs perfectly well without the daemon.

### 3. 100% local

No outbound network calls in the hot path. **As of v0.3.2, real
BGE-small-en-v1.5 ONNX embeddings (384-dim) are on by default**: the
bootstrap downloads the model once (~133 MB) from the HF Hub mirror
at `huggingface.co/aaditya4u/mneme-models` and the `brain` crate
loads it via ONNX Runtime 1.24.4 (bundled `onnxruntime.dll` in
`~/.mneme/bin/`, auto-pinned via `ORT_DYLIB_PATH` so the bundled
copy always wins over Win11 24H2's System32 hijack). The pure-Rust
hashing-trick embedder is still in the tree as a fallback - flip
`MNEME_FORCE_HASH_EMBED=1` to use it instead of BGE. After install,
nothing leaves your machine: block mneme at the firewall and it keeps
working.

The model lineup that ships with v0.3.2:

| Model | Size | Role |
|---|---|---|
| **bge-small-en-v1.5** (ONNX) | ~33 MB | semantic concept embeddings (384-dim) |
| **Qwen 2.5 Coder 0.5B** (GGUF) | ~340 MB | code-aware completion / tool selection |
| **Qwen 2.5 Embed 0.5B** (GGUF) | ~340 MB | code embedding for cross-file recall |
| **Phi-3-mini-4k-instruct Q4_K_M** (GGUF) | ~2.28 GB | reasoning + summary |

Models are downloaded once from
https://huggingface.co/aaditya4u/mneme-models (primary) with a GitHub
Releases fallback. The only other "network" exception is `mneme models
install --from-path <local-mirror>` which copies pre-downloaded model files
from a path you specify - still local.

### 4. Marker-based idempotent injection

When `mneme install` writes to your `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.codex/config.toml`, etc., it wraps its section in `<!-- mneme-start v1.0 -->` / `<!-- mneme-end -->`. Re-running install replaces the block, never duplicates. You can edit outside the markers freely; mneme won't touch your edits.

### 5. Append-only schema

`store/src/schema.rs` is append-only. Columns get added; they never get dropped or renamed. To rename something conceptually, add the new column, stop writing the old one, and leave the old column in place forever. This makes rolling upgrades safe and means downgrading is always OK.

## The 11 built-in scanners

`mneme audit` fans the file list across the scanner-worker pool (~5x faster on
multi-core machines as of v0.3.2 / B12) and streams findings into `findings.db`
incrementally so a long audit never loses partial results on timeout.

| Scanner | Catches |
|---|---|
| `theme` | Hardcoded colors, missing `dark:` variants in Tailwind classes |
| `types_ts` | `any`, non-null `!`, unsafe casts |
| `security` | Secrets, `eval`, missing IPC validation, dangerous patterns |
| `a11y` | Missing aria-labels, contrast issues, alt-text gaps |
| `perf` | Missing `useMemo`/`useCallback`, sync I/O on render path |
| `drift` | CLAUDE.md rule violations (custom constraints) |
| `ipc` | Electron IPC handlers without zod schemas |
| `markdown_drift` | Stale `.md` claims that no longer match source |
| `secrets` | Credential shapes (AWS keys, GitHub tokens, etc.) |
| `refactor` | Code-smell suggestions (long functions, deep nesting) |
| `architecture` | Cross-layer violations, cyclic deps |

## Data flow - "what happens when I run `mneme build`"

1. **CLI walks the project** with `walkdir`, respecting `.gitignore` + common ignore patterns
2. For each file with a supported language:
   - **Read bytes** (skip if content is binary-looking)
   - **Parse** via the Tree-sitter parser pool (one `tree_sitter::Parser` per worker, cached query patterns)
   - **Extract** `Node` + `Edge` records via the extractor (function defs, class defs, imports, calls, decorators, comments)
3. **Write** every node and edge into `graph.db` through the store's single-writer channel
4. Done - the shard is now queryable by any MCP tool or any other client

Incremental rebuilds reuse cached Tree-sitter trees keyed by file content hash (blake3). Unchanged files are zero-cost on subsequent builds. Pass `--rebuild` to force a full re-parse from scratch (added in v0.3.2 / B11).

## Data flow - "what happens when Claude calls `blast_radius()`"

1. Claude's MCP client sends `{"jsonrpc":"2.0","method":"tools/call","params":{"name":"blast_radius","arguments":{"target":"src/auth/login.ts","depth":2}}}` over stdio to the `mneme mcp stdio` process
2. The MCP server validates the input with zod
3. It opens `graph.db` read-only via `bun:sqlite`
4. It runs a recursive CTE that walks `edges` from the target, bounded by depth
5. It transforms the result into the schema the MCP client expects and sends it back
6. Total time: **<5 ms on a warm shard**

## Data flow - "what happens during context compaction"

This is the killer feature. Simplified:

1. At any moment you give Claude a numbered plan, every step gets an entry in `tasks.db` with `status`, `acceptance_cmd`, `started_at`, etc.
2. Your session proceeds; steps progress; the ledger updates
3. Context compaction wipes Claude's in-memory conversation history
4. **Next time Claude tries to resume**, mneme's `session-prime` or `step_resume` tool is called first
5. The tool reads `tasks.db`, finds the current step, and returns a resumption bundle:
   - The verbatim original goal (as first typed)
   - The goal stack
   - Completed steps with proof artifacts
   - Current step + where Claude left off
   - Remaining steps with acceptance checks
   - Active constraints
6. Claude's next turn receives this bundle as context and resumes at the correct step

No prompt engineering. No "remember the rules". The state lives in SQLite - it can't be forgotten.

## Language choices, briefly

- **Rust** for the supervisor, store, parsers, scanners, livebus, brain - everything that must be fast, fault-tolerant, and statically linkable. The workspace ships 12 crates (`common`, `supervisor`, `store`, `parsers`, `scanners`, `livebus`, `multimodal-bridge`, `cli`, `md-ingest`, `brain`, `benchmarks`, `vision/tauri` excluded) with binaries 5-50 MB each.
- **Bun + TypeScript** for the MCP server and vision app - hot-reloadable tool definitions, fast cold start, zod at the boundary. `bun:sqlite` is the fastest SQLite binding in any runtime.
- **Python** for the multimodal sidecar - the ecosystem around PDF extraction (PyMuPDF), OCR (Tesseract), and speech-to-text (faster-whisper) is irreplaceable.

The three languages talk over msgpack or JSON on Unix-domain sockets / Windows named pipes - no shared memory, no dynamic linking across language boundaries.

## Where to go next

- [`docs/INSTALL.md`](INSTALL.md) - install paths + troubleshooting
- [`docs/dev-setup.md`](dev-setup.md) - build from source
- [`docs/mcp-tools.md`](mcp-tools.md) - reference for every MCP tool
- [`docs/faq.md`](faq.md) - common questions
- [`docs/env-vars.md`](env-vars.md) - all `MNEME_*` env vars
- [`CONTRIBUTING.md`](../CONTRIBUTING.md) - how to add a scanner, language, view, or MCP tool

---

[← back to README](../README.md)
