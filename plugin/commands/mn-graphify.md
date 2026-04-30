---
name: /mn-graphify
description: Re-run the corpus graphifier — embeddings + Leiden clusters + concept extraction across the project.
command: mneme graphify
---

# /mn-graphify

Re-build the semantic + concept layer over the working tree. Graphify is
the heavy step that produces embeddings, runs Leiden community detection,
and extracts concepts. Most of the time the supervisor handles this
incrementally — but for a clean rebuild or a forced re-embed, use this
command.

## Usage

```
/mn-graphify                           # incremental refresh of changed files
/mn-graphify --full                    # full rebuild from scratch
/mn-graphify --since <git-ref>         # only files changed since ref
/mn-graphify --modality code           # restrict to code modality
```

## What this does

1. Calls the `graphify_corpus(scope?, modality?)` MCP tool.
2. The supervisor dispatches the brain worker (embeddings) and parser
   pool (tree-sitter + concept extraction) in parallel.
3. Progress is streamed back to the terminal; final summary shows
   added / updated / unchanged counts and elapsed time.

## When to use

- After importing a large external repo.
- After upgrading the embeddings model.
- When `recall_concept(query)` returns degraded results.

See also: `/mn-rebuild` (full rebuild including non-graph shards),
`/mn-audit` (run scanners after graphify).
