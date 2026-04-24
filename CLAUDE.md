# CLAUDE.md — Working on the mneme project itself

This file tells Claude Code (and any AI tool that reads `CLAUDE.md`) how to navigate **the mneme codebase** when developing or maintaining mneme itself.

If you're a user *consuming* mneme as an MCP plugin, see [README.md](README.md) instead. This file is for working on mneme's source.

---

## Project context

- **Owner / sole copyright holder**: Anish Trivedi.
- **License**: Apache-2.0. See [LICENSE](LICENSE). Permissive: use, modify, distribute, sublicense, including commercially. Requires attribution + NOTICE file preservation.
- **Status**: alpha — actively being iterated.
- **Architecture**: multi-process (Rust supervisor + Bun TS MCP + Bun TS Vision app + Python multimodal sidecar). Architecture overview in [`docs/architecture.md`](docs/architecture.md).

---

## Hard rules when editing this codebase

### Rust
- Strict mode, no `unsafe` outside `service.rs` daemonize path.
- All errors via `thiserror`; no `.unwrap()` on user-input paths.
- All async via `tokio`; no `block_on` inside an async context.
- Per-shard single-writer invariant in store crate is sacred. Reads can come from anywhere; writes always go through the writer task for that shard. Do not bypass.
- All paths constructed via `mneme_common::PathManager`. Never join paths manually.
- `panic = "abort"` in release profile. Don't change this.

### TypeScript (mcp/ and vision/)
- Bun-flavored TS. ES2022. moduleResolution: bundler. Strict mode.
- No `any`. Use `unknown` + type guards for boundaries.
- All MCP tool inputs/outputs validated with `zod`. No raw JSON in tool handlers.
- All MCP tools call the Rust supervisor via IPC, not SQLite directly.
- Hot-reload safe: never hold module-level mutable state in `mcp/src/tools/`; new versions must be drop-in replaceable.

### Python (workers/multimodal/)
- 3.10+. Strict type hints. Pydantic models at IPC boundaries.
- 100% local: every extractor must REFUSE network access. No `requests`, no `urllib`, no `httpx` to remote endpoints. Models loaded from disk paths only.
- All extractors implement the `Extractor` interface. Failure must return an `ExtractionResult` with `success=False`, never raise to the supervisor.

### Local-only invariant (project-wide)
- Mneme must NEVER make outbound network calls during normal operation. The only exceptions are user-initiated `mneme models install --from <local-mirror>` (still local) or, with explicit opt-in, `mneme update --check` (polls a single user-configured URL).
- Section 22 of the design doc has the full ban list. Any new feature that contemplates network access is auto-rejected unless explicitly approved by Anish.

### Resource policy
- No artificial caps on RAM, CPU, or disk. Use `num_cpus` for worker pool sizing. Cache size unlimited unless user opts in. See `docs/design/2026-04-23-resource-policy-addendum.md`.

---

## Working on mneme without a real Cargo / Bun toolchain

If you're modifying source code:

1. **Foundation crates (`common/`, `store/`)** are the most architecturally sensitive. Changes here ripple through every other crate. Always read the consumer crates before changing a public type in `common/`.
2. **Tree-sitter grammars** must match the version pinned in workspace `Cargo.toml`. Do not casually upgrade.
3. **Plugin manifests** in `plugin/templates/` use marker-based idempotent injection (`<!-- mneme-start v1.0 --> ... <!-- mneme-end -->`). Preserve the markers.
4. **`store/src/schema.rs`** is append-only. Never drop or rename a column. To rename conceptually: add a new column, stop writing the old one, leave the old in place forever.

---

## Crate / module map

| Path | Purpose | Owner |
|---|---|---|
| `common/` | Shared types (ProjectId, ShardHandle, DbLayer, Response, ...) | hand-written |
| `store/` | DB Operations Layer (Builder/Finder/Path/Query/Inject/Lifecycle) | hand-written |
| `supervisor/` | Process tree, watchdog, Windows service | agent-generated |
| `parsers/` | Tree-sitter pool, query cache, extractor | agent-generated |
| `scanners/` | Theme/security/perf/a11y/drift/IPC scanners | agent-generated |
| `brain/` | Embeddings + Leiden + concept extraction | agent-generated |
| `livebus/` | SSE/WebSocket push channel | agent-generated |
| `multimodal-bridge/` | Rust shim for Python sidecar | hand-written |
| `cli/` | `mneme` CLI (install/build/audit/recall/step/etc.) | agent-generated |
| `workers/multimodal/` | Python sidecar (PDF/Whisper/OCR) | agent-generated |
| `mcp/` | Bun TS MCP server (47 tools, 6 hooks) | agent-generated |
| `vision/` | Tauri + Bun TS app (14 views + Command Center) | agent-generated |
| `plugin/` | plugin.json + templates + agents + skills + commands | agent-generated |
| `scripts/` | Install scripts (POSIX + PowerShell), runtime deps | agent-generated |
| `docs/design/` | Architecture spec + addenda | hand-written |

---

## Mneme's own MCP tools — use them on mneme itself

Once mneme is installed and indexed on its own source:

```
/mn-recall "compaction recovery"     → finds the Step Ledger §7 design + impl
/mn-blast common/src/layer.rs        → who depends on the DbLayer enum
/mn-audit                            → drift findings across the workspace
/mn-step status                      → current goal stack for in-progress work
/mn-doctor                           → SLA + storage health
```

When working on mneme itself, prefer these MCP tools over Grep/Read/Glob — that's the whole point of mneme.

---

## Coding rules inherited from Anish's global setup

- Functional React only (vision/), no class components
- Strict TypeScript, no `any`
- Tailwind classes with `dark:` variants where applicable (vision/ uses inline utility classes since Tailwind isn't a dep)
- Named exports only, no default exports (TS)
- Test in both light/dark themes if UI changes (vision/)
- One task at a time; verify each step before moving to the next
- No shortcuts; read the file, understand context, then change

---

## Build commands (when toolchain present)

```bash
# Workspace
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace --deny warnings

# MCP server
cd mcp && bun install && bun test

# Vision app
cd vision && bun install && bun run build
cd vision/tauri && cargo build --release

# Python sidecar
cd workers/multimodal && pip install -e ".[dev]" && pytest
```

---

## Where to ask questions

There is no public discussion forum. This is a private, proprietary project. If you have access to this codebase, you have a direct relationship with Anish Trivedi — ask him directly.
