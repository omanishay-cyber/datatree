<!-- datatree-start v1.0 -->
# Datatree — Qoder Manifest

> This file is consumed by **Qoder** as a project context file. Datatree
> integrates with Qoder via the same MCP server + hooks used by Claude
> Code; this manifest documents the Qoder-specific routing rules.

This project has the **datatree** local daemon installed. Datatree provides
Qoder with persistent SQLite memory, a live code graph, drift detection, a
compaction-resilient step ledger, and 30+ MCP tools.

## Qoder-specific Tool Routing

Qoder's chat-then-act loop benefits from datatree's structured recall.
Before any code action:

1. `recall_file(path)` — confirm the file is in the index and get a summary.
2. `recall_constraint(scope='file', file=path)` — get rules that apply.
3. `blast_radius(target=path)` — know what your edit will ripple into.

Then act, and after the action ends call no extra hooks — datatree's
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

<!-- datatree-end v1.0 -->
