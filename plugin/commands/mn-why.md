---
name: /mn-why
description: Explain why a target exists — the chain of decisions, concepts, and ancestors that led to its current shape.
command: mneme why
---

# /mn-why

Trace the lineage of a file or symbol — every decision, concept, and
ancestor that explains why it looks the way it does. Pulls from the
decisions shard + concept graph + git blame.

## Usage

```
/mn-why <file>                         # why does this file exist
/mn-why <module::symbol>               # why does this symbol exist
/mn-why <file> --since v0.3.0          # decisions since a release
/mn-why <file> --json                  # machine-readable
```

## What this does

1. Calls the `why(target, since?)` MCP tool.
2. Aggregates: decisions referencing the target, concepts the target
   anchors, git history of the introducing commit.
3. Renders a timeline grouped by source: decisions · concepts · commits.

## When to use

- New contributor reading a strange file: "why is this here?"
- Code review: "did we already decide this approach?"
- Debugging: "what was the original intent — has it drifted?"

See also: `/mn-recall decision` (decisions log), `/mn-godnodes`
(architectural pivots).
