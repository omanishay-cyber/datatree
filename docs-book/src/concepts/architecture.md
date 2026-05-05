# Architecture

Mneme runs as a daemon next to your AI host. Every binary is local. Every database is SQLite under `~/.mneme/`. Every HTTP endpoint binds to `127.0.0.1`. Nothing leaves the machine without your explicit opt-in (the `federated_similar` MCP tool, off by default).

## Process tree

```text
                 Claude Code / Cursor / Codex
                          │
          ┌───────────────┼───────────────────┐
          │               │                   │
          ▼               ▼                   ▼
    PreToolUse      UserPromptSubmit      MCP stdio
       hook              hook                │
          │               │                   │
          ▼               ▼                   ▼
   ~/.mneme/bin/mneme-hook   (Windows: GUI subsystem)
          │
          ▼
   ~/.mneme/bin/mneme  (CLI dispatch)
          │
          │  IPC over Unix socket / Named Pipe
          ▼
   ~/.mneme/bin/mneme-daemon  (always-on supervisor)
          │
          ├──► mneme-parsers   (Tree-sitter worker pool)
          ├──► mneme-store     (SQLite single-writer per shard)
          ├──► mneme-scanners  (audit pass workers)
          ├──► mneme-livebus   (in-process pub/sub)
          ├──► mneme-multimodal (PDF/image/audio/video extractors)
          └──► HTTP server :7777  (vision SPA + /api/graph/*)
```

## Storage layout

`~/.mneme/projects/<hash>/`:

```text
graph.db      — nodes, edges, communities, centralities (~10-100 MB typical)
semantic.db   — embeddings (text_hash → vector) (~5-50 MB)
history.db    — conversation log (per-session decisions)
findings.db   — audit results
concepts.db   — learned concept graph
multimodal.db — extracted text from PDF/images/audio
git.db        — git history mirror
errors.db     — error registry
tasks.db      — step ledger
```

22 layers in total. The "shard" terminology refers to a per-project directory; the daemon's `mneme-store` worker holds a SQLite connection per shard per layer. Single-writer-per-shard is enforced via mpsc channels — every write goes through one writer task per shard, eliminating SQLite's busy-waiter contention.

## Schema versions

Each shard has its own `PRAGMA user_version`. Migrations run forward-only on shard open via `apply_migrations(layer)`. Per-layer dispatch lets each layer's migration set reference its own tables without breaking shards on other layers.

v0.4.0 schema is at `user_version = 2`. The v1→v2 migration clears file-anchored embeddings on first build after upgrade.

## IPC

The CLI talks to the daemon via Unix domain socket on Linux/macOS, Named Pipe on Windows (path written to `~/.mneme/supervisor.sock` for the socket case, or auto-discovered via the registry on Windows).

Wire format: line-delimited JSON. Each request gets one response. Concurrent requests multiplex over a single socket. Reply timeouts default to 30 s; long operations stream progress events back over the same socket.

## HTTP

The daemon also exposes a 17-endpoint HTTP API on `127.0.0.1:7777` (the port is fixed; no override yet). Used by:

- The vision SPA (Tauri shell or browser at `http://127.0.0.1:7777/`)
- `/api/health` for liveness probes
- `/api/graph/nodes`, `/edges`, `/files`, `/findings`, `/status`, `/layout`, `/file-tree`, `/kind-flow`, `/domain-flow`, `/community-matrix`, `/commits`, `/heatmap`, `/layers`, `/galaxy-3d`, `/test-coverage`, `/theme-palette`, `/hierarchy`

No auth — the daemon assumes localhost-only binding. Browser requests via DNS-rebinding-style attacks are not in the threat model (this is a local dev tool); if you want stricter isolation, run the daemon inside a sandbox.

## Embedding pipeline

```text
Tree-sitter parse  →  AST
                      │
                      ▼
                 extract_nodes()  →  graph.db nodes table
                      │                  │
                      │                  ▼
                      │            (qualified_name = blake3 stable_id)
                      │                  │
                      ▼                  │
              derive_text_for_embedding()│
                      │                  │
                      ▼                  │
              canonical_anchor + body    │
                      │                  │
                      ▼                  │
               BGE inference             │
                  (batched 64)           │
                      │                  │
                      ▼                  │
                 vector (384 f32)        │
                      │                  │
                      ▼                  ▼
              semantic.db ←──── nodes.embedding_id back-link
```

The vector dimension is 384 (BGE-small-en-v1.5). Other dimensions (768 for BGE-base) are supported via the model_name column on the embeddings table, but the bundled installer only ships small.

## Symbol resolution

[See the resolver concept page →](./resolver.md)

The resolver currently feeds the embedding pass via `*_file_prefix` helpers. Wiring the full algorithm into the extractor (so `qualified_name` is the resolved canonical, not a blake3 hash) is queued for v0.4.1 — the keystone gap the audit flagged is mostly closed by the embedding side; the graph-query side still uses the legacy stable_id.

## Local-only model loading

Models live at `~/.mneme/llm/`:

- `bge-small-en-v1.5.onnx` (130 MB) — embedding model
- `tokenizer.json` (paired tokenizer, ~700 KB)
- `phi-3-mini-4k-q4.gguf` (2 GB, optional) — local LLM for `mneme why` summaries
- `qwen-embed-0.5b.gguf` (alternate embedder, optional)

The runtime is bundled ONNX Runtime 1.24.4 (Windows: shipped `onnxruntime.dll`; macOS/Linux: linked at runtime via `ORT_DYLIB_PATH`). Load happens via `brain::embeddings::RealBackend::try_new`. If the model files are missing, the embedder falls back to a hashing-trick backend — works for triage but not for semantic recall.

## Federation (opt-in)

The `federated_similar` MCP tool exchanges blake3-hashed concept signatures between Mneme instances inside an explicitly-configured federation. The protocol never carries the underlying source — only signature fingerprints. Default is OFF (no federation).

[Vision SPA →](./vision.md) · [Resolver →](./resolver.md) · [Self-ping enforcement →](./self-ping.md)
