---
name: /mn-rebuild
description: Drop the entire index and re-parse from scratch (DESTRUCTIVE)
---

Run `mneme rebuild <project>` and surface the results in this conversation.

When the user invokes `/mn-rebuild`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme rebuild` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- project path (required) — the directory whose shard will be dropped and reindexed from scratch. This is destructive.

## Example

```
$ mneme rebuild .
WARNING: this will drop the existing index for .
  shard: ~/.mneme/shards/mneme-v0.3.0
  rows to discard: 11068
proceed? [y/N] y
dropped. reindexing...
  parsed:  847 files / 12.4s
  graph:   229 concepts / 1124 edges
  embed:   847 vectors / 4.1s
ok
```
