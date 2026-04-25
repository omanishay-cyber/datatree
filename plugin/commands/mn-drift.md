---
name: /mn-drift
description: Show current drift findings (rule violations)
---

Run `mneme drift` and surface the results in this conversation.

When the user invokes `/mn-drift`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme drift` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- `--severity` (info|warn|error|critical) — filter findings by severity threshold.
- `--project PATH` — override the project root (defaults to current workspace).

## Example

```
$ mneme drift --severity critical
critical findings: 2

  src/auth/login.ts:34
    rule: no-plaintext-secrets
    detail: literal token committed to source

  vision/src/views/Settings.tsx:118
    rule: theme-must-have-dark-variant
    detail: bg-white missing dark: counterpart
```
