---
name: /mn-snap
description: Take a manual snapshot of the active shard
---

Run `mneme snap` and surface the results in this conversation.

When the user invokes `/mn-snap`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme snap` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- project path (optional) — directory whose shard should be snapshotted (defaults to current workspace).

## Example

```
$ mneme snap
shard: ~/.mneme/shards/mneme-v0.3.0
snapshot id: snap_2026-04-24T18-22-04Z
size: 14.2 MiB
rows: history=8421 semantic=2104 decisions=87 tasks=44 findings=312
ok
```
