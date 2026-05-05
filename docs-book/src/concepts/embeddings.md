# Symbol-anchored embeddings

The second piece of the v0.4.0 keystone. The symbol resolver from the previous chapter produces canonical names; this chapter explains how those names get stitched into the embedding text so semantic recall actually returns the right thing.

## Before v0.4.0 — file-anchored

The embedding pipeline ran on graph nodes. For each node it produced text from this fallback chain:

```text
embed_text = signature
          || summary
          || "{kind} {name}"
```

So a function `pub async fn spawn(...) -> Result<JobId>` got embedded with text that looked exactly like that — the literal signature. The README chunk that said "the spawn function manages workers" got embedded with text "the spawn function manages workers".

Vector similarity for the query "where is spawn?" weighted both candidates roughly the same. Often the README won because it mentioned `spawn` more times than the signature did.

This is the root cause of the recall gap the 2026-05-05 audit measured at **2/10 vs CRG's 6/10**.

## After v0.4.0 — symbol-anchored

`derive_text_for_embedding` now prepends a canonical anchor in front of the body:

```text
embed_text = canonical_anchor + " " + (signature || summary || "{kind} {name}")
```

For Rust:

```text
canonical_anchor = "crate::manager::WorkerPool::spawn"
embed_text       = "crate::manager::WorkerPool::spawn pub async fn spawn(&self, job: Job) -> Result<JobId>"
```

For TypeScript:

```text
canonical_anchor = "vision/src/views/ForceGalaxy::ForceGalaxy"
embed_text       = "vision/src/views/ForceGalaxy::ForceGalaxy export function ForceGalaxy(): JSX.Element"
```

For Python:

```text
canonical_anchor = "pkg.sub.mod.spawn"
embed_text       = "pkg.sub.mod.spawn def spawn(job): ..."
```

The leaf segment (`spawn`, `ForceGalaxy`) is what dominates BGE's similarity score for typical queries. The prefix gives the model two more signals: the file path / module structure, and the language separator that lets it disambiguate cross-language matches.

## Cross-language consistency

Rust and TS share the `::` separator; Python uses `.`. The BGE tokenizer treats both the same way at the leaf — `crate::manager::spawn` and `pkg.sub.spawn` both end with the `spawn` token, so cross-language recall works regardless of separator.

We don't normalise to one separator because:

- Rust's tooling (rustdoc, cargo) speaks `::`. Forcing `.` would break round-tripping.
- Python's tooling (jedi, mypy, sphinx) speaks `.`. Forcing `::` would break round-tripping.
- TS doesn't have a strong native separator; we picked `::` to match Rust because the audience overlaps.

## Anchor for unsupported languages

For languages without a resolver yet (Go, Java, Ruby, etc.), the anchor falls back to:

```text
canonical_anchor = "{file_path} :: {name}"
```

Less precise than a real resolver, but better than nothing — the file path still constrains the vector space so a query for "FooBar" matches FooBar's row, not random README chunks that mention it.

## Migration on upgrade

v0.3.x users have populated `embedding_id` columns pointing at file-anchored vectors. The v0.4.0 schema migration v1→v2 clears them:

- `graph.db`: `UPDATE nodes SET embedding_id = NULL WHERE embedding_id IS NOT NULL`
- `semantic.db`: `DELETE FROM embeddings`

Then the next `mneme build` pass re-embeds every node with the new symbol-anchored text. On a 17K-node project this takes 1-3 minutes; on a 100K-node project, 10-15 minutes. Both surfaces emit progress via the heartbeat:

```text
[INFO ] applying schema migration (may take a moment on large shards) layer=Graph from=1 to=2
[INFO ] applying schema migration (may take a moment on large shards) layer=Semantic from=1 to=2
[INFO ] embedding pass starting — large projects can take several minutes
phase=embed processed=2342/17280 rate=240/s
phase=embed processed=8841/17280 rate=240/s
phase=embed processed=17280/17280 rate=240/s
```

## Test surface

9 dedicated tests in `cli/src/commands/build.rs` covering:

- Anchor for each language (Rust / TS / Python)
- Anchor fallback for unknown languages
- Anchor returns None for empty inputs (caller filters)
- Anchor + signature priority
- Anchor + summary fallback when signature is empty/whitespace
- Legacy behaviour preserved when language metadata is unavailable
- Empty body → empty text → caller filters

[Symbol resolver →](./resolver.md) · [Architecture →](./architecture.md)
