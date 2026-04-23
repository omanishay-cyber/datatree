<!-- datatree-start v1.0 -->
# Datatree — Gemini CLI Manifest

> This file is consumed by **Gemini CLI** as a project context file. Datatree
> works identically in Gemini sessions; the rules below are tuned for
> Gemini's tool-use model.

This project has the **datatree** local daemon installed. Datatree gives
Gemini CLI a persistent SQLite memory, live code graph, drift detector,
step ledger, and 30+ MCP tools — all stored locally under
`~/.datatree/projects/`.

## Gemini-specific Tool Routing

Gemini CLI's strongest pattern is "ask one tool, get structured JSON, decide
next." Datatree returns deterministic JSON for every tool, so chain like:

```
recall_file(path) → if hash unchanged && summary present, skip the read
recall_constraint(scope='file', file=path) → constraints to honor in your edit
blast_radius(target) → know what you'll affect before you edit
```

Prefer these over `read_file` / `glob` / `search_file_content` whenever
the structural answer is sufficient.

## Datatree MCP Tool Catalog (relevant subset)

| Category | Tools |
|---|---|
| Recall | `recall_decision`, `recall_conversation`, `recall_concept`, `recall_file`, `recall_todo`, `recall_constraint` |
| Graph | `blast_radius`, `call_graph`, `find_references`, `dependency_chain`, `cyclic_deps` |
| Multimodal | `graphify_corpus`, `god_nodes`, `surprising_connections`, `audit_corpus` |
| Drift | `audit`, `drift_findings`, `audit_theme`, `audit_security`, `audit_a11y`, `audit_perf`, `audit_types` |
| Step Ledger | `step_status`, `step_show`, `step_verify`, `step_complete`, `step_resume`, `step_plan_from` |
| Time Machine | `snapshot`, `compare`, `rewind` |
| Health | `health`, `doctor`, `rebuild` |

## Step Ledger

Gemini's chat compaction can occur silently. After **any** unexpected
context reset, call `step_resume()` immediately. Continue from the
current step — do not restart.

## Drift Redirects

If two consecutive responses drift from the active goal, the next prompt
will be prefixed with `<datatree-redirect>`. Treat it as a hard signal:
re-anchor before doing more work.

## Local Only

Datatree makes zero outbound network calls. No remote LLMs (uses llama.cpp
for any local inference), no embeddings APIs (bge-small ONNX), no
telemetry, no cloud sync.

<!-- datatree-end v1.0 -->
