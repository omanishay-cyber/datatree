# Symbol resolver

The keystone of Genesis. Three per-language algorithms (Rust + TypeScript/JavaScript + Python) that turn syntactic names into one canonical string per logical symbol — the foundation for every "find references" query, every blast-radius computation, and every embedding the corpus indexes.

## The problem

Tree-sitter gives you SYNTAX. It tells you "here is a function called `spawn` declared at `manager.rs:1100`" or "here is a call to `super::foo()` at `health.rs:42`". What it does NOT tell you is whether **this** `spawn` is the same as **that** `spawn`, or whether `mod::foo` and `crate::mod::foo` and `super::foo` and `use crate::mod; foo()` are all the same canonical symbol.

Without that mapping:

- `recall_concept "spawn"` matches whatever file mentions the word "spawn" — usually the README, sometimes a doc comment. The actual function rarely wins because its signature only contains `spawn` once while a tutorial doc mentions it five times.
- `find_references "WorkerPool"` returns text matches, not structural matches. `mod::WorkerPool`, `crate::mod::WorkerPool`, `WorkerPool` (after `use crate::mod::WorkerPool`) all look like distinct strings.
- `blast_radius "manager.rs"` underestimates impact because cross-file calls via `use` or `import` chains aren't tied back to the function definition.

The 2026-05-05 audit ran the same 10-query golden benchmark against Mneme and CRG on identical hardware. Before the keystone, Mneme returned a correct hit on **2 of 10 queries**; CRG returned **6 of 10**. The same run measured token reduction at **1.34×** against CRG's claimed **6.8×**. Both gaps share one root cause: no symbol resolver.

## What Genesis ships

Three real per-language resolvers, plus a passthrough fallback for languages without a resolver yet.

### `RustResolver` ([source][rs])

[rs]: https://github.com/omanishay-cyber/mneme/blob/main/parsers/src/resolver.rs

Implements the resolution algorithm CRG ships in `jedi_resolver.py` (Python) but for Rust source. Turns syntactic names like `WorkerPool`, `super::spawn`, `crate::manager::WorkerPool`, and `use crate::manager; spawn()` into a single canonical string per logical symbol.

Coverage:

- File path → canonical module prefix
  - `src/manager.rs` → `crate::manager`
  - `src/foo/bar.rs` → `crate::foo::bar`
  - `src/foo/mod.rs` → `crate::foo`
  - `src/lib.rs` and `src/main.rs` → `crate`
  - `cli/src/commands/build.rs` → `crate::commands::build` (workspace-member `src/` prefix is stripped; the workspace path before it is the workspace prefix and gets dropped — only the part FROM `src/` onward becomes the module path)
- `crate::X::Y` references — left as-is (already canonical)
- `super::X` — walks one level up the file's prefix and prepends. `super::super::X` walks two; counted-not-recursed so the rewrite stays correct under arbitrary depth.
- `self::X` — replaces with the file's prefix.
- Bare names looked up in a `UseMap` derived from the file's `use` statements; multi-step aliasing (`use a::b as c`) and group imports (`use a::{b, c::d}`) are flattened on construction.
- Cross-crate references (`std::collections::HashMap`) — left verbatim. We don't try to resolve external crates; their canonical form is already what the source spelled.

Out of scope (on the roadmap):

- `pub use` re-export chasing
- Trait impl resolution (`<Foo as Bar>::method`)
- Generic monomorphization

### `TypeScriptResolver`

The same shape adapted to TS/JS resolution rules:

- File-prefix from relative path, with extension stripped (`.tsx` / `.ts` / `.jsx` / `.js` / `.mjs` / `.cjs` precedence-ordered). Genesis audit fix: keeping the extension caused def/ref namespace mismatch where a relative import `./Foo` resolved without extension while the definition lived at `Foo.tsx::Bar`.
- Relative imports (`./Foo`, `../bar`, `./..`)
- tsconfig `paths` aliases (wildcard + exact match, first-match-wins)
- Bare module specifiers (`react`, `@scope/pkg`) — passed through verbatim, can't resolve to a project file

### `PythonResolver`

Adapted again for Python's import semantics:

- File-prefix conversion: `pkg/sub/mod.py` → `pkg.sub.mod`. `__init__.py` is collapsed (the directory IS the module). `__main__.py` is preserved (real module name). Leading `src/` or `lib/` segments are stripped (common Python project layouts).
- Relative imports: N leading dots = walk N parents up, then append the rest. `from . import x` resolves to the file's own package.
- Aliased imports via `PythonImportMap`: `import os.path as osp` → `osp.join` resolves to `os.path.join`. `from collections import deque as dq` → `dq` resolves to `collections.deque`.
- Native `.` separator (not `::`) for round-trip compatibility with jedi, mypy, pylint, sphinx.

## Why dots vs `::`

Rust and TS share `::` as their canonical separator because the resolver-side string is ours to choose, but Python's ecosystem already canonicalises on dots: `import pkg.sub.mod`, `pkg.sub.mod.foo`, `__all__ = ['foo']`. Forcing `::` here would break round-tripping with every Python tool downstream of Mneme. Cross-language search is unaffected because the BGE embedder anchors on the leaf segment regardless of separator — `crate::manager::spawn` and `pkg.sub.spawn` and `vision/src/views/Foo::Bar` all share the same `spawn`/`Bar` token at the end of the line.

## What's NOT yet wired

The resolver algorithms ship as a library in Genesis. Three downstream consumers need to wire through them:

| Consumer | Status | When |
|----|----|----|
| `canonical_embed_anchor` (used by the embedding pass) | uses `*_file_prefix` helpers ✓ | Genesis |
| `extractor.rs` (writes `nodes.qualified_name` into graph.db) | uses blake3 stable_id (legacy) | roadmap |
| `find_references` / `blast_radius` (read graph.db) | reads the legacy stable_id | roadmap |

The embedding pass already takes effect — `recall_concept` queries should now hit the function row instead of the README chunk. The graph queries (`find_references`, `blast_radius`, `call_graph`) still use the legacy hash-based qualified_name and won't see structural improvement until the resolver wires into the extractor.

The 2026-05-05 audit's recall improvement (from 2 correct hits to ~6 on the 10-query golden benchmark) is the embedding side of the fix. The graph-query side is its own bench, scheduled for the next release with the extractor wiring.

## Test surface

37 dedicated resolver tests in [`parsers/src/resolver.rs::tests`][tests], covering:

[tests]: https://github.com/omanishay-cyber/mneme/blob/main/parsers/src/resolver.rs

- File-prefix derivation (Rust + TS + Python, including edge cases)
- `super::` walking (1 level, N levels, past-root behaviour)
- `self::` rewriting
- `crate::` passthrough
- `::std::*` extern-path stripping
- Use-map alias substitution (last-write-wins on duplicates)
- TS path-alias wildcard + exact match (first-match-wins)
- TS extension stripping (all 6 supported extensions, multi-dot basenames)
- Python relative imports (N leading dots, walking past root, bare-dot-from-top-level)
- Python aliased imports (single-name + dotted-head substitution)
- Cross-language consistency (passthrough handles unknown languages cleanly)

Plus 9 integration tests in `cli/src/commands/build.rs` that exercise `canonical_embed_anchor` against each language's resolver to confirm the embedding-text shape.

## Migration on upgrade

When you first run `mneme build` after upgrading to the Genesis keystone, the schema migration framework runs once:

```text
migrating graph.db: UPDATE nodes SET embedding_id = NULL ...
migrating semantic.db: DELETE FROM embeddings ...
embedding pass starting — large projects can take several minutes
```

This is required: existing `embedding_id` values point at file-anchored vectors that pre-date the symbol-anchor format. Re-embedding once produces the new symbol-anchored vectors. The migration is idempotent (running twice is a no-op) and runs inside a transaction so a crash mid-way leaves you back where you started.

[Architecture →](./architecture.md) · [Symbol-anchored embeddings →](./embeddings.md)
