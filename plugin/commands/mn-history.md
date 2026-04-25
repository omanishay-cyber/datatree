---
name: /mn-history
description: Search the conversation / step ledger history
---

Run `mneme history <query>` and surface the results in this conversation.

When the user invokes `/mn-history`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme history` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- `query` (required) — free-text search across conversation turns and step ledger entries.
- `--since UNIX_MS` — only return entries newer than this timestamp.
- `--limit N` (default 20) — cap the number of results.
- `--project PATH` — override the project root (defaults to current workspace).

## Example

```
$ mneme history "auth refactor" --limit 10
2026-04-22 14:08  step  refactor login handler to use AuthService
2026-04-22 13:51  turn  user: "let's pull AuthService out of routes/auth.ts"
2026-04-21 09:14  step  add unit tests for AuthService.verifyToken
2026-04-20 17:32  turn  assistant: "drafted decision: AuthService owns token lifecycle"
... (6 more)
```
