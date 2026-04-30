---
name: /mn-rebuild
description: Full clean rebuild of the local project shards — drops and re-creates every per-project SQLite database.
command: mneme rebuild
---

# /mn-rebuild

Drop every per-project shard for the current project and re-create them
from scratch. Use this when an upgrade changed schema in an
incompatible way, or when corruption is suspected. This is heavier than
`/mn-graphify --full`: it also rebuilds history, tasks, audit, etc.

## Usage

```
/mn-rebuild                            # confirm prompt, then rebuild all shards
/mn-rebuild --shard graph              # rebuild a single shard
/mn-rebuild --no-confirm               # CI / scripted use
/mn-rebuild --dry-run                  # show what would be dropped
```

## What this does

1. Acquires the BuildLock so concurrent indexers stay coherent.
2. Drops the shards selected (default: all shards under
   `~/.mneme/projects/<project-id>/`).
3. Re-creates them via the canonical schema, then re-indexes the
   working tree.
4. Logs every action so a partial failure is recoverable.

## When to use

- After a mneme version upgrade flagged "schema incompatible".
- When `/mn-doctor` reports shard-integrity errors.
- When you want to reset experimental data captured during a debug session.

See also: `/mn-graphify` (cheaper — only the semantic layer),
`/mn-doctor` (verify integrity before / after).
