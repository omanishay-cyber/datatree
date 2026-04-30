---
name: /mn-godnodes
description: List the architectural pivots — files and symbols that the most code depends on. Touch with care.
command: mneme godnodes
---

# /mn-godnodes

Show the project's god nodes — the files and symbols sitting at the
center of the dependency graph. Editing a god node ripples into the
largest blast radius in the codebase.

## Usage

```
/mn-godnodes                           # top 20 god nodes
/mn-godnodes --limit 50                # extend the list
/mn-godnodes --threshold 30            # min in-degree for inclusion
/mn-godnodes --json                    # machine-readable output
```

## What this does

1. Calls the `god_nodes(limit?, threshold?)` MCP tool.
2. Ranks nodes by in-degree, betweenness centrality, and PageRank.
3. Renders a table: rank · target · in-degree · score · short reason.

## Suggested workflow

- New contributor onboarding: read the top 10 to learn the architecture.
- Before architectural change: re-run; pivot points may have shifted.
- Pair with `/mn-blast` on each god node to see what depends on it.

See also: `/mn-blast` (blast radius for a single target), `/mn-recall`
(decisions about why a node became central).
