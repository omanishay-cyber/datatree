<!-- mneme-start v1.0 -->
# Mneme - Qwen-Code Manifest

> This file is consumed by **Qwen-Code** (Alibaba's Qwen-coder CLI) as a
> project context file. Mneme integrates via stdio MCP, identically to
> Claude Code.

This project has the **mneme** local daemon installed (v0.3.2). Mneme gives
Qwen-Code persistent SQLite memory, a live code graph, drift detection, a
compaction-resilient step ledger, and **48 MCP tools** + **11 scanners** +
**8 hooks** + **14 WebGL views**.

## Qwen-specific Tool Routing

Qwen tends to over-Read large files. Mneme's `recall_file` returns the
hash + summary in <5ms - use it first and skip the Read entirely if the
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
includes the verification gate for the current step - pass it before
`step_complete()`.

## Local Only

Mneme never calls Qwen's cloud or any other remote service. All
inference is local (llama.cpp + Phi-3-mini for any text inference;
bge-small for embeddings).

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
