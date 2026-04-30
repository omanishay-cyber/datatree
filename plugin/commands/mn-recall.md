---
name: /mn-recall
description: Semantic search across decisions, conversations, concepts, files, todos, and constraints stored in the local mneme shards.
command: mneme recall
---

# /mn-recall

Query the persistent project memory + knowledge graph without re-reading
any files.

## Usage

```
/mn-recall <query>                          # auto-route to best tool
/mn-recall decision "<query>"               # decisions log
/mn-recall conversation "<query>"           # past conversation turns
/mn-recall concept "<query>" --modality code # concepts (filter by modality)
/mn-recall file <path>                      # file state + summary
/mn-recall todo                             # open TODOs
/mn-recall constraint --scope project       # active rules
```

## What this does

Routes to the matching MCP recall tool:

| Sub-command | Tool |
|---|---|
| (auto) | best of `recall_decision`, `recall_concept`, `recall_conversation` |
| `decision` | `recall_decision(query, since?, limit?)` |
| `conversation` | `recall_conversation(query, since?, session_id?, limit?)` |
| `concept` | `recall_concept(query, modality?, limit?)` |
| `file` | `recall_file(path)` |
| `todo` | `recall_todo(filter?)` |
| `constraint` | `recall_constraint(scope?, file?)` |

Returns a concise rendered answer with `source_location` citations.

See also: the `mneme-query` skill.
