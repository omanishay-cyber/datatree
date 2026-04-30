---
name: /mn-status
description: One-glance status: daemon, shards, indexer queue, current step, recent drift findings.
command: mneme status
---

# /mn-status

Single-page snapshot of the mneme system. Combines daemon health, shard
sizes, indexer queue depth, the active Step Ledger entry, and any
critical drift findings into one terminal screen.

## Usage

```
/mn-status                             # human-readable status panel
/mn-status --json                      # machine-readable
/mn-status --watch                     # live refresh every 2s
```

## What this does

1. Reads the daemon health endpoint via local IPC (no network).
2. Stat-walks `~/.mneme/projects/<project-id>/` for shard sizes.
3. Calls `step_status()` for the current ledger entry.
4. Calls `drift_findings(severity='critical')` for the red list.
5. Renders one panel: daemon · shards · queue · step · drift.

## When to use

- First command of the day.
- After any unexplained slowdown.
- Before claiming "everything is working" — verify it.

See also: `/mn-doctor` (deep self-test), `/mn-step` (Step Ledger
detail), `/mn-drift` (full drift findings list).
