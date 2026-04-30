---
name: /mn-history
description: Browse the local conversation + tool-call history captured by mneme — every prompt, every tool call, every result.
command: mneme history
---

# /mn-history

Search the local history shard for past conversation turns and tool
calls. Everything stays on this machine — mneme captures only what the
host harness exposes via hooks.

## Usage

```
/mn-history                            # last 20 turns, current session
/mn-history --session <id>             # a specific session
/mn-history --since 2026-04-20         # since a date
/mn-history --tool <name>              # filter by tool call
/mn-history --query "<text>"           # full-text search
/mn-history --json                     # machine-readable
```

## What this does

1. Reads from `~/.mneme/projects/<project-id>/history.db`.
2. Renders a table: turn id · timestamp · role · brief.
3. Each row collapsed to one line; expand with `--id <turn-id>`.

## Suggested workflow

- "Where did we discuss X?" → `/mn-history --query X`.
- Postmortem: `/mn-history --session <id> --json | jq` for analysis.
- Resume context after compaction: prefer `/mn-step` (Step Ledger) first.

See also: `/mn-recall conversation` (semantic search across history),
`/mn-step` (Step Ledger).
