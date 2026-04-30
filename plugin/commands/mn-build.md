---
name: /mn-build
description: Build (or incremental-rebuild) the mneme index for the current project, taking the BuildLock so reads stay coherent.
command: mneme build
---

# /mn-build

Run a coherent index build for the current project. Acquires the
BuildLock for the duration so concurrent readers see a consistent
snapshot, then releases it on completion.

## Usage

```
/mn-build                              # incremental build (default)
/mn-build --full                       # full rebuild (slow, comprehensive)
/mn-build --shard graph                # build a single shard
/mn-build --since <git-ref>            # only files changed since ref
/mn-build --json                       # machine-readable progress
```

## What this does

1. Acquires the BuildLock for the project.
2. Dispatches the parser pool over changed files.
3. Updates the affected shards (graph, semantic, deps, ...).
4. Releases the BuildLock.
5. Returns a summary: files indexed · errors · elapsed.

## When to use

- After a large `git pull` to bring the index up to date.
- After importing files from outside `git`.
- In CI before running `/mn-audit --scope project`.

## Difference from `/mn-graphify` and `/mn-rebuild`

- `/mn-build` — the everyday incremental indexer.
- `/mn-graphify` — heavier; re-runs embeddings + concept extraction.
- `/mn-rebuild` — heaviest; drops and re-creates shards.

See also: `/mn-graphify`, `/mn-rebuild`, `/mn-status`.
