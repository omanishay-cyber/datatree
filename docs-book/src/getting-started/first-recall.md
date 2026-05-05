# First recall

After your first build, exercise the recall surface to confirm the index is alive.

## CLI

```bash
mneme recall "spawn"
```

Output:

```text
1. crate::manager::WorkerPool::spawn          (supervisor/src/manager.rs:1100)
   pub async fn spawn(&self, job: Job) -> Result<JobId>
   callers=12, dependents=5, tests=3
2. crate::worker_ipc::spawn_worker             (supervisor/src/worker_ipc.rs:42)
   ...
```

The Symbol resolver + symbol-anchored embeddings (v0.4.0 keystone) are why the function row outranks any README chunk that mentions the word "spawn". Pre-v0.4.0, the README usually won.

## More recall flavours

```bash
mneme recall_concept "WorkerPool"     # concept graph (no decisions/todos)
mneme recall_decision "auth"          # ledger-only
mneme recall_constraint "thread-safe" # architectural constraints
mneme recall_todo                     # all open TODOs/FIXMEs
mneme recall_file "supervisor"        # files matching the keyword
```

## Graph queries

```bash
mneme blast supervisor/src/manager.rs --depth=2
mneme why "Why does v0.4.0 exist?"
mneme history "auth refactor"
mneme godnodes --n=20
```

## Same surface from your AI

```text
mcp__mneme__mneme_recall query="spawn"
mcp__mneme__find_references symbol="WorkerPool"
mcp__mneme__blast_radius target="supervisor/src/manager.rs"
mcp__mneme__god_nodes
mcp__mneme__architecture_overview
```

Your AI host's tool browser should show all 50 tools after `mneme install --platform=claude-code`. Restart the AI host once to pick up the new MCP entry.

## When recall returns README chunks

If `recall_concept "spawn"` returns the README's "the spawn function manages workers" line as the top hit, two things to check:

1. **Have you re-built since upgrading to v0.4.0?** v0.3.x file-anchored embeddings still work but don't get the symbol-anchor benefit. The schema migration v1→v2 clears them on first build after upgrade — run `mneme build .` to trigger.

2. **Is BGE actually loaded?**

   ```bash
   mneme doctor
   ```

   The `embeddings:` line at the bottom should say `bge-small-en-v1.5 active` not `hashing-trick fallback`. If it says fallback, install the model:

   ```bash
   mneme models install bge-small-en-v1.5
   mneme build .          # re-embed with the real model
   ```

## See also

- [Symbol resolver](../concepts/resolver.md) — why v0.4.0 changed the recall game
- [Symbol-anchored embeddings](../concepts/embeddings.md) — how the anchor gets stitched in
- [MCP tools](../mcp/tools.md) — the full 50-tool inventory for AI consumption
