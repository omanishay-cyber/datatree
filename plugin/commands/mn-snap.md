---
name: /mn-snap
description: Take a snapshot of the current shards (graph, history, tasks, etc.) for later inspection or rollback.
command: mneme snap
---

# /mn-snap

Capture the live state of the mneme shards into an immutable snapshot.
Snapshots are stored under `~/.mneme/snapshots/<project-id>/<ts>/` and
remain readable forever. The Step Ledger references the latest snapshot
ID so resume after compaction can verify "you were here".

## Usage

```
/mn-snap                               # snapshot all shards, default label
/mn-snap --label "before refactor"     # human-readable label
/mn-snap --shard graph,history         # subset of shards
/mn-snap list                          # list existing snapshots
/mn-snap show <snap-id>                # metadata for one snapshot
```

## What this does

1. Calls the `snapshot(scope?, label?)` MCP tool.
2. The store crate hard-links the SQLite shard files into the snapshot
   directory (zero-copy on the same filesystem).
3. Returns a snapshot id + path that can be passed to `/mn-rollback`.

## When to use

- Before a destructive refactor.
- Before running `/mn-rebuild`.
- At the start of a long task — gives the resumer a stable anchor.

See also: `/mn-rollback` (restore a snapshot), `/mn-rebuild`.
