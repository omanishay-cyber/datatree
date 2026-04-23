<!-- mneme-start v1.0 -->
# Mneme — Qwen-Code Manifest

> This file is consumed by **Qwen-Code** (Alibaba's Qwen-coder CLI) as a
> project context file. Mneme integrates via stdio MCP, identically to
> Claude Code.

This project has the **mneme** local daemon installed. Mneme gives
Qwen-Code persistent SQLite memory, a live code graph, drift detection, a
compaction-resilient step ledger, and 30+ MCP tools.

## Qwen-specific Tool Routing

Qwen tends to over-Read large files. Mneme's `recall_file` returns the
hash + summary in <5ms — use it first and skip the Read entirely if the
file is unchanged since you last saw it.

For large refactors, ALWAYS run `blast_radius(target)` first. Qwen's wide
context can hide impact; mneme makes it explicit.

## Tool Catalog

Same set as the universal AGENTS.md. Quick reference:

| Need | Tool |
|---|---|
| File summary | `recall_file(path)` |
| Past decisions | `recall_decision(query)` |
| Active rules | `recall_constraint(scope, file?)` |
| Impact analysis | `blast_radius(target)` |
| References | `find_references(symbol)` |
| All scanners | `audit(scope='project')` |
| Step ledger | `step_status`, `step_resume`, `step_complete` |
| Architecture | `god_nodes()` + `audit_corpus()` |

## Step Ledger + Compaction

After any Qwen context reset, call `step_resume()` first. The bundle
includes the verification gate for the current step — pass it before
`step_complete()`.

## Local Only

Mneme never calls Qwen's cloud or any other remote service. All
inference is local (llama.cpp + Phi-3-mini for any text inference;
bge-small for embeddings).

<!-- mneme-end v1.0 -->
