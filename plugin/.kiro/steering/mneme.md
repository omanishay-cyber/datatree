---
title: Mneme
description: Local persistent project memory + live code graph + drift detector + 30+ MCP tools.
inclusion: always
---

<!-- mneme-start v1.0 -->
# Mneme — Kiro Steering

This project has the **mneme** local daemon installed. Mneme gives
Kiro persistent SQLite memory, a live code graph, drift detection, a
compaction-resilient step ledger, and 30+ MCP tools.

## Kiro-specific Routing

Kiro's spec-driven workflow benefits enormously from mneme's step ledger.
Every spec section becomes a step row in `tasks.db`, with its acceptance
check automatically captured. After Kiro restarts:

1. `step_resume()` returns the resumption bundle — original goal,
   completed steps with proofs, YOU ARE HERE marker, planned steps,
   active constraints, verification gate.
2. Continue from the current step. Do not restart from the beginning.

## Tool Catalog

| Need | Tool |
|---|---|
| File summary | `recall_file(path)` |
| Past decisions | `recall_decision(query)` |
| Active rules | `recall_constraint(scope='file', file=path)` |
| Impact analysis | `blast_radius(target)` |
| References | `find_references(symbol)` |
| All scanners | `audit(scope='project')` |
| Step ledger | `step_status`, `step_resume`, `step_complete` |
| Architecture | `god_nodes()` + `audit_corpus()` |

## Drift Redirects

If two consecutive Kiro turns drift from the goal, the next prompt is
prefixed with `<mneme-redirect>`. Re-anchor before continuing.

## Local Only

Mneme makes zero outbound network calls. All inference and embeddings
are local. State lives in `~/.mneme/projects/<hash>/`.

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
