# Mneme — The AI Superbrain

Persistent per-project memory + 14-view code graph + drift detector + 50 MCP tools. Local-only. Apache-2.0.

Mneme runs as a daemon next to your AI host (Claude Code, Cursor, Codex). It indexes your project once, then keeps the graph fresh as you edit. The AI sees your code through Mneme's lens — not through ad-hoc grep — and gets the same answers across sessions, restarts, and compaction.

## Why Mneme

When you ask an AI "where does `WorkerPool::spawn` get called?", the cheap answer is regex over text files. The slightly less-cheap answer is grep. Both miss `super::spawn`, `crate::manager::spawn`, `use crate::manager; spawn()`, and aliased re-exports. Mneme answers with structural certainty: parser-built call graphs, symbol resolver, BGE embeddings anchored on canonical names, all in a daemon the AI talks to via MCP.

```text
mneme recall_concept "spawn"
  →  WorkerPool::spawn  (supervisor/src/manager.rs:1100)
     pub async fn spawn(&self, job: Job) -> Result<JobId>
     [callers: 5, dependents: 12, tests: 3]
```

## What's in v0.4.0

The 2026-05-05 audit comparing Mneme to CRG and graphify identified one root cause behind both the recall gap (Mneme 2/10 vs CRG 6/10) and the token gap (Mneme 1.34× vs CRG's claimed 6.8×): no symbol resolver. v0.4.0 ships the keystone.

- **Symbol resolver (Rust + TypeScript + Python)** — turns syntactic names into one canonical string per logical symbol.
- **Symbol-anchored embeddings** — BGE vectors are now anchored on the resolved canonical name, not the file. `recall_concept "spawn"` matches the actual function instead of the README.
- **PreToolUse soft-redirect** — when the AI calls Grep on something resolver-shaped, the hook injects a hint pointing at `mcp__mneme__find_references`.
- **Server-pre-computed graph layout** — Force-directed view paints in <500 ms instead of 3 s.
- **Auto-update with rollback** — `mneme self-update` runs `--version` against the new binary and restores the old one if anything goes wrong.

[Read the v0.4.0 release notes →](./releases/v0.4.0.md)

## Install in one line

| Platform | Command |
|----|----|
| Linux | `curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-linux.sh \| bash` |
| macOS | `curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-mac.sh \| bash` |
| Windows | `iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/bootstrap-install.ps1 \| iex` |

[Full install guide →](./install/index.md)

## Three things to read first

1. [**Architecture**](./concepts/architecture.md) — what's running on your machine, what data goes where, what doesn't go on the network.
2. [**Symbol resolver**](./concepts/resolver.md) — the keystone of v0.4.0, the answer to "why did mneme give a better answer this time?"
3. [**MCP tools**](./mcp/tools.md) — the 50 tools your AI can call. Every one is local, deterministic, and audited.

## Local-only by design

Nothing leaves your machine. Mneme's HTTP daemon binds to 127.0.0.1, the embedding model runs locally via ONNX Runtime, the LLM (when enabled) runs locally via llama.cpp, and the graph database is plain SQLite under `~/.mneme/`. No telemetry, no analytics, no cloud sync. The optional federated-similar tool exchanges blake3-hashed signatures only if you opt in, and even then it's machine-to-machine within your own infrastructure.

## License

[Apache-2.0](https://github.com/omanishay-cyber/mneme/blob/main/LICENSE). Free to use, modify, and ship inside commercial products.
