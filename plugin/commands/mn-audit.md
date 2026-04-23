---
name: /mn-audit
description: Run mneme's drift + quality scanners over the working tree, the diff, or one file.
command: mneme audit
---

# /mn-audit

Run mneme's full scanner suite (theme, types, security, accessibility,
performance) and surface findings ranked by severity.

## Usage

```
/mn-audit                       # all scanners on the project
/mn-audit --scope diff          # only files in `git status`
/mn-audit --file <path>         # single file
/mn-audit --scanner theme       # one scanner
/mn-audit --json                # machine-readable output
```

## What this does

1. Calls the `audit(scope, file?, scanners?)` MCP tool.
2. Renders findings as a table grouped by severity.
3. Exits non-zero if any critical findings are present (useful in CI).

## Suggested workflow

- Before commit: `/mn-audit --scope diff`
- After major refactor: `/mn-audit --scope project`
- Investigate one file: `/mn-audit --file src/auth/session.ts`
- CI gate: `mneme audit --scope diff --json | jq .summary.by_severity`

See also: `/mn-drift` (open findings only) and the `mneme-audit` skill.
