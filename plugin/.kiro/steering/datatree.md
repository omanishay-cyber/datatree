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

<!-- mneme-end v1.0 -->
