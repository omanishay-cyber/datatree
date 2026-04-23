---
name: /dt-recall
description: Semantic search across decisions, conversations, concepts, files, todos, and constraints stored in the local datatree shards.
command: datatree recall
---

# /dt-recall

Query the persistent project memory + knowledge graph without re-reading
any files.

## Usage

```
/dt-recall <query>                          # auto-route to best tool
/dt-recall decision "<query>"               # decisions log
/dt-recall conversation "<query>"           # past conversation turns
/dt-recall concept "<query>" --modality code # concepts (filter by modality)
/dt-recall file <path>                      # file state + summary
/dt-recall todo                             # open TODOs
/dt-recall constraint --scope project       # active rules
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

See also: the `datatree-query` skill.
