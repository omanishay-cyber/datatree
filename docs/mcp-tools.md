# mneme MCP tools reference

48 tools wired to real data as of v0.3.2 (2026-05-02), grouped by category.
Every tool is callable from Claude Code, Codex, Cursor, or any MCP-aware AI
client once the bootstrap installer (or `mneme install`) has registered the
MCP server.

Every tool is hot-reloadable - drop a new file into `mcp/src/tools/` while
the daemon is running and the registry picks it up within 250 ms. Every input
and output is zod-validated at the MCP server boundary; the schemas live in
[`mcp/src/types.ts`](../mcp/src/types.ts).

> **v0.3.2 status:** 48 of 48 tools wired to real data. Every tool either
> hits supervisor IPC (with graceful-degrade fallback when the verb isn't
> present) or reads live sqlite via `bun:sqlite`. See
> [`BENCHMARKS.md`](../benchmarks/BENCHMARKS.md) for the measured harness.

---

## Recall & search (10 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `recall` | Hybrid retrieval (semantic + keyword) across all shards | `{query, scope?, limit?}` | `Hit[]` |
| `recall_decision` | Semantic search over decisions logged in `history.db` | `{query, since?}` | `Decision[]` |
| `recall_conversation` | Search verbatim conversation history | `{query, since?}` | `Turn[]` |
| `recall_concept` | Semantic search across extracted symbols & concepts | `{query, limit?}` | `Concept[]` |
| `recall_file` | Full file state: hash + summary + last read + blast radius | `{path}` | `FileInfo` |
| `recall_todo` | Open TaskCreate items | `{filter?}` | `Todo[]` |
| `recall_constraint` | Active constraints for current project | `{scope?}` | `Constraint[]` |
| `context` | Build a focused context bundle for a query (token-budgeted) | `{query, budget?}` | `ContextBundle` |
| `federated_similar` | Find similar code across all indexed projects (multi-shard) | `{snippet, k?}` | `Match[]` |
| `why` | Why-chain: why does this file/symbol exist? Provenance trace | `{target}` | `WhyChain` |

## Code graph (5 tools, CRG-mode)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `blast_radius` | Everything affected by a change | `{target, depth?}` | `BlastRadius` |
| `call_graph` | Direct + transitive call graph | `{function, depth?}` | `CallGraph` |
| `find_references` | All usages of a symbol | `{symbol}` | `Reference[]` |
| `dependency_chain` | Forward + reverse import chain | `{file}` | `Dependencies` |
| `cyclic_deps` | Detect circular dependencies | `{}` | `Cycle[]` |

## Multimodal & corpus (4 tools, Graphify-mode)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `graphify_corpus` | Run multimodal extraction pass | `{path?}` | `CorpusReport` |
| `god_nodes` | Top-N most-connected concepts | `{n?}` | `GodNode[]` |
| `surprising_connections` | High-confidence unexpected edges | `{}` | `Edge[]` |
| `audit_corpus` | Generate `GRAPH_REPORT.md` | `{}` | `CorpusAudit` |

## Drift & audit (7 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `audit` | Run every scanner, return findings (streams incrementally to findings.db) | `{scope?, scanners?}` | `Finding[]` |
| `drift_findings` | Current rule violations | `{severity?}` | `Finding[]` |
| `audit_theme` | Hardcoded colors, dark: variants | `{}` | `ThemeFinding[]` |
| `audit_security` | Secrets, eval, IPC validation | `{}` | `SecurityFinding[]` |
| `audit_a11y` | Missing aria-labels, contrast | `{}` | `A11yFinding[]` |
| `audit_perf` | Missing memoization, sync I/O | `{}` | `PerfFinding[]` |
| `audit_types` | `any`, non-null assertions | `{}` | `TypesFinding[]` |

> The `audit` tool fans the file list across scanner-workers (B12 in v0.3.2,
> ~5x faster on multi-core) and streams findings into `findings.db` so a
> long audit never loses partial results on timeout.

## Step Ledger / Command Center (8 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `step_status` | Current step + ledger snapshot | `{}` | `StepStatus` |
| `step_show` | Detail of one step | `{step_id}` | `Step` |
| `step_verify` | Run acceptance check | `{step_id}` | `VerifyResult` |
| `step_complete` | Mark complete (only if verify passes) | `{step_id}` | `Ok` |
| `step_resume` | Emit resumption bundle (compaction recovery) | `{}` | `ResumptionBundle` |
| `step_plan_from` | Ingest markdown roadmap -> ledger | `{markdown_path}` | `Roadmap` |
| `resume` | Wide resume - latest session + step + open work | `{}` | `ResumeBundle` |
| `suggest_skill` | Read `plugin/skills/*/SKILL.md` and recommend next skill | `{context?}` | `SkillSuggestion[]` |

## Time machine (3 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `snapshot` | Manual snapshot | `{}` | `SnapshotId` |
| `compare` | Diff two snapshots | `{a, b}` | `Diff` |
| `rewind` | File content at a past time | `{file, when}` | `FileContent` |

## Refactor & wiki (4 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `refactor_suggest` | Suggest a refactor for a file/symbol | `{target, kind?}` | `RefactorPlan` |
| `refactor_apply` | Apply a previously-suggested refactor | `{plan_id}` | `RefactorResult` |
| `wiki_generate` | Generate a wiki page for a module / topic | `{topic, scope?}` | `WikiPage` |
| `wiki_page` | Read an existing wiki page | `{slug}` | `WikiPage` |

## Architecture & intent (4 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `architecture_overview` | High-level architecture summary for the project | `{}` | `Overview` |
| `identity` | Project identity card (name, language, stack, conventions) | `{}` | `Identity` |
| `conventions` | Active code-style conventions (lint rules, format rules) | `{}` | `Convention[]` |
| `file_intent` | Per-file intent annotation - "what is this file for?" | `{path}` | `FileIntent` |

## Health & ops (3 tools)

| Tool | Purpose | Input | Output |
|---|---|---|---|
| `health` | Full SLA snapshot (latency p50/p99 + worker uptime) | `{}` | `SlaSnapshot` |
| `doctor` | Self-test, return diagnostics | `{}` | `Doctor` |
| `rebuild` | Re-parse from scratch (last resort) | `{scope?}` | `Ok` |

---

## Example calls

### From Claude Code (it does this automatically)

Claude picks up the MCP server automatically after the bootstrap installer
has written the entry to `~/.claude.json`. You can see the tools by running
`/mcp` in any Claude Code session after a restart, or invoke any plugin
slash command:

```
/mn-recall "auth flow"
/mn-blast src/auth/login.ts
/mn-doctor
/mn-build .
```

The plugin slash commands auto-register at install (v0.3.2 / B14.5).

### From the command line (for debugging)

```bash
# Raw JSON-RPC 2024-11-05 call:
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"blast_radius","arguments":{"target":"src/auth/login.ts","depth":2}}}' | mneme mcp stdio
```

### Example output (real, from running mneme v0.3.2)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"target\":\"src/auth/login.ts\",\"affected_files\":[\"src/auth/validator.ts\",\"src/pages/login.tsx\"],\"affected_symbols\":[\"validateCredentials\",\"LoginForm\"],\"test_files\":[\"src/auth/login.test.ts\"],\"total_count\":7,\"critical_paths\":[\"validateCredentials\"]}"
    }]
  }
}
```

---

## Schema contract

Every tool's input and output is validated with `zod` at the MCP server boundary. Schemas live in [`mcp/src/types.ts`](../mcp/src/types.ts). They're the source of truth - the tables above are a summary.

If you want to add a new tool, the pattern is:

1. Add the input/output Zod schema to `mcp/src/types.ts`
2. Create `mcp/src/tools/your_tool.ts` following the pattern in [`mcp/src/tools/blast_radius.ts`](../mcp/src/tools/blast_radius.ts)
3. Add a helper in `mcp/src/store.ts` if you need a new DB query shape
4. The hot-reload watcher picks it up within 250 ms - no daemon restart needed

See [`docs/dev-setup.md`](dev-setup.md#add-a-new-mcp-tool) for the full walkthrough.

---

## Permissions

Most MCP tools run read-only against the project's SQLite shard. Tools that
write (e.g. `step_complete`, `refactor_apply`, `snapshot`) go through the
supervisor's single-writer IPC so the Single-Writer Invariant is preserved.
No tool can corrupt the graph by racing another writer.

## Latency budgets

Expected response times for each tool on a warm shard - in plain English:

| Tool category | Typical | Slow case |
|---|---|---|
| `recall_*` (single query) | sub-millisecond | ~5 ms |
| `blast_radius` (depth 2) | ~2 ms | ~10 ms |
| `call_graph` (depth 5) | ~5 ms | ~25 ms |
| `audit_*` (per category) | ~10 ms | ~80 ms |
| `step_*` | sub-millisecond | ~3 ms |
| `health` | ~3 ms (HTTP to supervisor) | ~15 ms |
| `graphify_corpus` | seconds (async; returns immediately, streams progress) | - |

For p50 / p99 microsecond detail, see the `SlaSnapshot` returned by
[`health`](../mcp/src/tools/health.ts) - v0.3.2 / B15 added human-readable
`typical_response_ms` and `slow_response_ms` fields alongside the raw
microsecond counters.

---

## See also

- [`docs/architecture.md`](architecture.md) - how the daemon + workers + MCP fit together
- [`docs/INSTALL.md`](INSTALL.md) - install paths + troubleshooting
- [`docs/dev-setup.md`](dev-setup.md) - build from source + add a tool
- [`docs/env-vars.md`](env-vars.md) - all `MNEME_*` env vars
- [`BENCHMARKS.md`](../BENCHMARKS.md) - the measured-harness numbers
- [`mcp/src/types.ts`](../mcp/src/types.ts) - the canonical zod schemas

---

[← back to README](../README.md)
