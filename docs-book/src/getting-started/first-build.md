# First build

After install, point Mneme at any code repo and let it index.

## Quick start

```bash
cd ~/code/your-project
mneme build .
```

That's it. The first build produces:

- `~/.mneme/projects/<hash>/graph.db` — nodes + edges
- `~/.mneme/projects/<hash>/semantic.db` — BGE embeddings
- `~/.mneme/projects/<hash>/findings.db` — audit findings (empty until first audit)
- `~/.mneme/projects/<hash>/concepts.db` — concept graph

## What "build" does

```text
1. Tree-sitter parses every source file        — usually fast (~0.5-2 ms per file)
2. Extract nodes (functions, classes, imports) — bottleneck on big repos
3. Extract edges (calls, contains, depends-on) — second bottleneck
4. Compute file hashes for incremental tracking
5. Write to graph.db (single-writer per shard)
6. Run multimodal pass (PDF/image/audio/video/.ipynb)
7. Run BGE embedding pass on every node       — symbol-anchored anchor + signature
8. Run community detection (Leiden algorithm)
9. Compute centrality scores
10. Run audit scanners (security, perf, types, theme, a11y, etc.)
11. Write a snapshot
```

Total time on the mneme repo (~17K nodes): 1-3 minutes on a typical SSD. Larger projects scale roughly linearly.

## Subsequent builds

```bash
mneme update .
```

`update` is the incremental form — only files changed since the last build get re-parsed and re-embedded. Typical incremental build on the mneme repo: 5-15 seconds.

`mneme build` always does the full pass. Use it after upgrading Mneme, or when you suspect the index is stale.

## Verify

```bash
mneme status
# project: ~/code/your-project
# nodes: 17,280  edges: 80,529  files: 893
# last build: 2 minutes ago
# embeddings: bge-small-en-v1.5 active
# audit findings: 41 (3 high, 12 medium, 26 low)
```

## See it in the browser

```bash
mneme view
```

Opens `http://localhost:7777/?project=<hash>` in your default browser (or in the Tauri shell on Windows). The 14 graph views populate from the freshly-built `graph.db`.

## When build is slow

The keystone v0.4.0 migration runs ONE TIME on first build after upgrade — it clears v0.3.x file-anchored embeddings so the new symbol-anchored anchor takes effect. On a 100K-row shard this can take 5-30 seconds with no progress output. Watch the heartbeat:

```text
[INFO ] applying schema migration (may take a moment on large shards) layer=Graph from=1 to=2
[INFO ] applying schema migration (may take a moment on large shards) layer=Semantic from=1 to=2
```

After the migration runs once, subsequent builds skip it.

The embedding pass itself is 30-120 seconds on a 17K-node project depending on CPU + whether BGE is available (falls back to hashing-trick if not). The Wave 2 audit added a `tracing::info` line announcing the embed pass:

```text
[INFO ] embedding pass starting — large projects can take several minutes
phase=embed processed=2342/17280 rate=240/s
phase=embed processed=8841/17280 rate=240/s
phase=embed processed=17280/17280 rate=240/s
```

## See also

- [First recall](./first-recall.md) — how to query what you just built
- [Architecture](../concepts/architecture.md) — what the daemon is doing under the hood
- [Troubleshooting](../troubleshooting.md) — when builds stall or fail
