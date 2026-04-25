---
name: /mn-godnodes
description: Top-N most-connected concepts in the graph
---

Run `mneme godnodes` and surface the results in this conversation.

When the user invokes `/mn-godnodes`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme godnodes` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- `--n N` (default 10) — how many top-ranked concepts to return.
- `--project PATH` — override the project root (defaults to current workspace).

## Example

```
$ mneme godnodes --n 20
rank  concept                       degree  modality
   1  AuthService                      142  code
   2  PathManager                       98  code
   3  ShardHandle                       87  code
   4  Step Ledger                       71  decision
   5  bge-small embedding model         63  config
  ... (15 more)
```
