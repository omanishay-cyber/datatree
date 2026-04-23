---
name: /dt-audit
description: Run mneme's drift + quality scanners over the working tree, the diff, or one file.
command: mneme audit
---

# /dt-audit

Run mneme's full scanner suite (theme, types, security, accessibility,
performance) and surface findings ranked by severity.

## Usage

```
/dt-audit                       # all scanners on the project
/dt-audit --scope diff          # only files in `git status`
/dt-audit --file <path>         # single file
/dt-audit --scanner theme       # one scanner
/dt-audit --json                # machine-readable output
```

## What this does

1. Calls the `audit(scope, file?, scanners?)` MCP tool.
2. Renders findings as a table grouped by severity.
3. Exits non-zero if any critical findings are present (useful in CI).

## Suggested workflow

- Before commit: `/dt-audit --scope diff`
- After major refactor: `/dt-audit --scope project`
- Investigate one file: `/dt-audit --file src/auth/session.ts`
- CI gate: `mneme audit --scope diff --json | jq .summary.by_severity`

See also: `/dt-drift` (open findings only) and the `mneme-audit` skill.
