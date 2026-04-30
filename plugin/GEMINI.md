<!-- mneme-start v1.0 -->
# Mneme — Gemini CLI Manifest

> This file is consumed by **Gemini CLI** as a project context file. Mneme
> works identically in Gemini sessions; the rules below are tuned for
> Gemini's tool-use model.

This project has the **mneme** local daemon installed. Mneme gives
Gemini CLI a persistent SQLite memory, live code graph, drift detector,
step ledger, and 30+ MCP tools — all stored locally under
`~/.mneme/projects/`.

## Gemini-specific Tool Routing

Gemini CLI's strongest pattern is "ask one tool, get structured JSON, decide
next." Mneme returns deterministic JSON for every tool, so chain like:

```
recall_file(path) → if hash unchanged && summary present, skip the read
recall_constraint(scope='file', file=path) → constraints to honor in your edit
blast_radius(target) → know what you'll affect before you edit
```

Prefer these over `read_file` / `glob` / `search_file_content` whenever
the structural answer is sufficient.

## Mneme MCP Tool Catalog (relevant subset)

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
will be prefixed with `<mneme-redirect>`. Treat it as a hard signal:
re-anchor before doing more work.

## Local Only

Mneme makes zero outbound network calls. No remote LLMs (uses llama.cpp
for any local inference), no embeddings APIs (bge-small ONNX), no
telemetry, no cloud sync.

## Workflow Codewords

When the user starts a message with one of these single words, switch how you engage:

| Word | What it means |
|---|---|
| `coldstart` | Pause. Observe only. Read context, draft a plan, do not touch code. Wait for `hotstart` or `firestart` before doing anything. |
| `hotstart` | Resume with discipline. Numbered roadmap. Verify each step before moving to the next. |
| `firestart` | Maximum loadout. Load every fireworks skill that matches the task, prime the mneme graph (`god_nodes`, `audit_corpus`, `recall_decision`), then proceed with `hotstart` discipline. |
| `CHS` | "Check my screenshot" - read the latest file in the user's OS-native screenshot folder (Windows `Pictures/Screenshots`, macOS `Desktop`, Linux `Pictures/Screenshots`) and respond based on its contents. |

These are not casual conversation. Treat them as commands. Full protocol per codeword lives in `~/.mneme/plugin/skills/mneme-codewords/SKILL.md`.

<!-- mneme-end v1.0 -->
