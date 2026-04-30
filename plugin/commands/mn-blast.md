---
name: /mn-blast
description: Compute blast radius — show every file, function, and test that would be impacted by changing a target.
command: mneme blast
---

# /mn-blast

Show the full ripple effect of a proposed change before you make it.

## Usage

```
/mn-blast <file>                       # blast radius for a file path
/mn-blast <module::symbol>             # blast radius for a fully-qualified symbol
/mn-blast <target> --depth 3           # explicit traversal depth
/mn-blast <target> --json              # machine-readable output
```

## What this does

1. Calls the `blast_radius(target, depth?)` MCP tool.
2. Categorizes results into critical paths, tests, and other dependents.
3. Renders a tree grouped by category, with severity badges for any file
   that hosts a god node from `god_nodes()`.

## Suggested workflow

- Before any non-trivial Edit: `/mn-blast <file>` to see what you'll touch.
- Before deleting a function: `/mn-blast <module::function>` to find callers.
- During refactor planning: pipe `--json` into `jq` to build a checklist.

See also: `/mn-godnodes` (architectural pivots), `/mn-recall` (concept search).
