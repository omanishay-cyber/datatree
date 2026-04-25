---
name: /mn-blast
description: Blast radius — what breaks if I change this
---

Run `mneme blast <target>` and surface the results in this conversation.

When the user invokes `/mn-blast`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme blast` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- `target` (required) — the file path or symbol to compute blast radius for.
- `--depth N` (default 2) — how many hops outward to traverse the dependency graph.
- `--project PATH` — override the project root (defaults to current workspace).

## Example

```
$ mneme blast handleLogin --depth 2
target: handleLogin (auth/handlers.ts)
direct callers (3):
  - routes/auth.ts:42
  - routes/sso.ts:18
  - tests/auth.test.ts:91
transitive (depth 2): 7 more files, 12 tests
```
