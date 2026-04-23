<!-- mneme-start v1.0 -->
# Mneme — Universal Agent Manifest

> This file is the **universal** version of mneme's plugin manifest. It is
> used identically by Codex, Cursor (via .cursor/rules), OpenCode, Aider,
> Trae, Continue, Cline, RooCode, and any other AI harness that reads
> AGENTS.md as a project-rules file.

This project has the **mneme** local daemon installed. Mneme exposes:
- 30+ MCP tools (recall, code graph, drift, step ledger, time machine, health)
- 6 hooks (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SessionEnd)
- 14 view modes for the live code graph
- Compaction-resilient Step Ledger

All mneme state is stored in local SQLite shards under
`~/.mneme/projects/<project-hash>/`. Nothing leaves this machine.

## Use mneme tools first

Before reaching for grep / glob / file reads, check whether mneme can
answer the question structurally:

| Goal | mneme tool |
|---|---|
| Find usages | `find_references(symbol)` |
| Trace impact of a change | `blast_radius(target)` |
| Get file metadata + summary | `recall_file(path)` |
| Recall prior decisions | `recall_decision(query)` |
| Open TODOs | `recall_todo()` |
| Active rules | `recall_constraint(scope, file?)` |
| Concept search across the corpus | `recall_concept(query)` |
| Architecture overview | `god_nodes()` + `audit_corpus()` |
| Find cycles | `cyclic_deps()` |
| Run scanners | `audit(scope?)`, `audit_theme()`, `audit_security()`, `audit_perf()`, `audit_a11y()`, `audit_types()` |

Token-efficiency target: **<= 5 tool calls per task, <= 800 tokens of context.**

## Step Ledger — Compaction Resilience

Mneme's killer feature is the Step Ledger. Whenever you take on a task
with three or more steps:

1. `step_plan_from(markdown_path)` — ingest a plan, or `step_status` to read it.
2. For each step: `step_show` → do the work → `step_verify` → `step_complete`.
3. After **any context reset**, call `step_resume()` first. Continue from the
   current step — do not restart from the beginning.

## Drift Detection

Mneme continuously scans changed files for rule violations and surfaces
findings via `drift_findings(severity?)`. Critical findings show up as
`<mneme-redirect>` blocks at the top of the next prompt. Re-anchor.

## Local-Only Constraint

Mneme never makes outbound network calls. No remote LLMs, no telemetry,
no cloud sync. All extraction, embedding, and audit work runs locally
(llama.cpp, bge-small ONNX, whisper.cpp, Tesseract).

## Quick Commands

```
/dt-view     open the live graph (Tauri or web)
/dt-step     view current step ledger
/dt-recall   semantic search
/dt-blast    blast radius
/dt-audit    run all scanners
/dt-doctor   self-test
```

## Performance Budgets

| Op | Target |
|---|---|
| Cold-start daemon | <100ms |
| MCP tool call (cached) | <1ms |
| MCP tool call (graph) | <5ms |
| MCP tool call (semantic) | <50ms |
| Live push latency | <50ms |
| Resume bundle | <100ms |

If you observe a tool exceeding budget, run `health()` — supervisor will
report the slow worker.

<!-- mneme-end v1.0 -->
