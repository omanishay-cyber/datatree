<!-- mneme-start v1.0 -->
# Mneme — Qoder Manifest

> This file is consumed by **Qoder** as a project context file. Mneme
> integrates with Qoder via the same MCP server + hooks used by Claude
> Code; this manifest documents the Qoder-specific routing rules.

This project has the **mneme** local daemon installed. Mneme provides
Qoder with persistent SQLite memory, a live code graph, drift detection, a
compaction-resilient step ledger, and 30+ MCP tools.

## Qoder-specific Tool Routing

Qoder's chat-then-act loop benefits from mneme's structured recall.
Before any code action:

1. `recall_file(path)` — confirm the file is in the index and get a summary.
2. `recall_constraint(scope='file', file=path)` — get rules that apply.
3. `blast_radius(target=path)` — know what your edit will ripple into.

Then act, and after the action ends call no extra hooks — mneme's
PostToolUse capture writes the change to history.db automatically.

## Tool Catalog

Same as Claude/AGENTS.md — see those files for the full list. Highlights:

- `recall_*` family for memory
- `blast_radius`, `call_graph`, `find_references` for impact
- `audit_*` family for drift / quality scanning
- `step_*` family for the step ledger (use after every Qoder context reset)

## Step Ledger

Qoder workflows that span multiple actions should be tracked in the step
ledger. After Qoder restarts or compacts, call `step_resume()` first.

## Local Only

No outbound network calls. All inference and embeddings run locally.

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
