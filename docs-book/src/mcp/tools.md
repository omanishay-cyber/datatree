# MCP tools (50)

Mneme's MCP server is what your AI host actually talks to. Stdio transport, JSON-RPC. Defined in `mcp/src/tools/*.ts` and registered in `mcp/src/index.ts`. Each tool has a stable name, a Zod-validated input schema, and a deterministic output shape.

## Tool catalogue

### Recall + memory

| Tool | What it does |
|---|---|
| `mcp__mneme__mneme_recall` | Semantic recall across the corpus. The keystone read — "find me the function that does X". |
| `mcp__mneme__mneme_resume` | Compaction-safe resume: returns the active step + last 5 conversation turns + active project context. |
| `mcp__mneme__mneme_why` | Why-Chain: decision trace combining git history + ledger + concept graph for any target. |
| `mcp__mneme__mneme_context` | Returns the active project + last build time + node/edge counts. |
| `mcp__mneme__mneme_identity` | Project identity (name, root, hash). |
| `mcp__mneme__mneme_conventions` | Coding conventions detected by the conventions scanner. |
| `mcp__mneme__mneme_federated_similar` | (opt-in) blake3-hashed signature exchange across a configured federation. |
| `mcp__mneme__recall_concept` | Concept-only recall (no decisions, todos, etc.). |
| `mcp__mneme__recall_constraint` | Constraint recall (architectural decisions, type contracts). |
| `mcp__mneme__recall_decision` | Decision recall (from the ledger). |
| `mcp__mneme__recall_conversation` | Conversation history recall. |
| `mcp__mneme__recall_file` | File-anchored recall (returns file metadata + summary). |
| `mcp__mneme__recall_todo` | Outstanding TODO/FIXME items in scope. |

### Graph queries

| Tool | What it does |
|---|---|
| `mcp__mneme__find_references` | Structural find-references across the project or workspace. |
| `mcp__mneme__call_graph` | Call graph for a target (callers + callees). |
| `mcp__mneme__blast_radius` | Direct + transitive consumers, affected tests, risk level. |
| `mcp__mneme__dependency_chain` | File-to-file dependency walk. |
| `mcp__mneme__cyclic_deps` | Detect import cycles. |
| `mcp__mneme__god_nodes` | Top-N most-connected nodes (anti-pattern detector). |
| `mcp__mneme__architecture_overview` | Layered architecture summary. |
| `mcp__mneme__file_intent` | Project-relative intent classification for a single file. |
| `mcp__mneme__compare` | Compare two graph snapshots. |
| `mcp__mneme__surprising_connections` | Cross-domain edges that span unrelated subsystems (refactor candidates). |

### Audit

| Tool | What it does |
|---|---|
| `mcp__mneme__audit` | Run all configured scanners. |
| `mcp__mneme__audit_a11y` | Accessibility scanner. |
| `mcp__mneme__audit_corpus` | Corpus-wide audit. |
| `mcp__mneme__audit_perf` | Performance scanner. |
| `mcp__mneme__audit_security` | Security scanner (input validation, IPC boundaries, secrets). |
| `mcp__mneme__audit_theme` | Theme/UI scanner. |
| `mcp__mneme__audit_types` | Type-correctness scanner (TS strict mode, Rust clippy patterns). |
| `mcp__mneme__drift_findings` | Read drift findings already in the DB. |

### Build + lifecycle

| Tool | What it does |
|---|---|
| `mcp__mneme__rebuild` | Drop graph, re-parse from scratch. |
| `mcp__mneme__graphify_corpus` | Multimodal extract pass (PDF, image, audio, video, .ipynb). |
| `mcp__mneme__snapshot` | Take a manual snapshot. |
| `mcp__mneme__rewind` | Roll back to a snapshot. |
| `mcp__mneme__doctor` | Full health check. |
| `mcp__mneme__health` | Cheap liveness ping. |

### Step Ledger

The Step Ledger is Mneme's compaction-safe task tracker. AI sessions can plan a multi-step task, get re-keyed by step ID after a compaction, and verify completion deterministically.

| Tool | What it does |
|---|---|
| `mcp__mneme__step_plan_from` | Create a plan from an issue / spec. |
| `mcp__mneme__step_show` | Show a step's full context. |
| `mcp__mneme__step_status` | List all steps with their current status. |
| `mcp__mneme__step_complete` | Mark a step done (records the diff). |
| `mcp__mneme__step_verify` | Verify a step's acceptance criteria. |
| `mcp__mneme__step_resume` | Resume the active step after a compaction. |

### Refactoring + suggestions

| Tool | What it does |
|---|---|
| `mcp__mneme__refactor_suggest` | Suggest a refactor for a target. |
| `mcp__mneme__refactor_apply` | Apply a previously-suggested refactor. |
| `mcp__mneme__suggest_skill` | Suggest a Mneme skill (slash command) for the current task. |

### Wiki

| Tool | What it does |
|---|---|
| `mcp__mneme__wiki_generate` | Generate a wiki page from a community / module. |
| `mcp__mneme__wiki_page` | Read an existing wiki page. |

## Common usage from the AI

```text
# Code symbol queries → find_references, not Grep
mcp__mneme__find_references symbol="WorkerPool"

# Before any non-trivial Edit
mcp__mneme__blast_radius target="supervisor/src/manager.rs" depth=2

# Cross-session resume after a compaction
mcp__mneme__mneme_resume

# Audit a single dimension
mcp__mneme__audit_security
```

The Layer 3 PreToolUse hook injects the right tool name into `additionalContext` when the AI does Grep/Read on something resolver-shaped. See [Self-ping enforcement](../concepts/self-ping.md).

## Tool result envelope

Every tool returns a JSON envelope:

```json
{
  "ok": true,
  "data": { ... tool-specific shape ... },
  "meta": {
    "latency_ms": 42,
    "cache_hit": false,
    "schema_version": 2
  }
}
```

Errors are explicit:

```json
{ "ok": false, "error": "...", "meta": {...} }
```

Both shapes are stable contracts — the AI host parses them with confidence.

## Caching

Tool responses go through a 5-min LRU + TTL cache (Item #121). The cache key is the tool name + serialised input. NEVER_CACHE bypass list: `audit`, `snapshot`, `refactor_apply`, `step_*`, `rebuild`, `health` (every other tool is cached).

## Size budgets

Every tool has a max byte cap in `mcp/src/result-cap.ts` (Item #118). Heavy tools (`architecture_overview`, `wiki_page`) get larger caps (32 KB / 24 KB); audit tools get 16 KB. When a result exceeds the cap it returns a truncation envelope:

```json
{
  "_truncated": true,
  "original_bytes": 84000,
  "max_bytes": 16000,
  "hint": "use ?limit=... or scope to a single subdir to narrow",
  "preview": "...first 2 KB..."
}
```

This protects the AI's context from accidental floods and gives a hint about how to scope the next call.

## See also

- [Hooks](../hooks/index.md) — the 3-layer self-ping integration that nudges the AI toward these tools
- [CLI commands](../cli/reference.md) — the same surface area, but for humans
