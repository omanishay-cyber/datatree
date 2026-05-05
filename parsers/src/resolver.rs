//! Symbol resolver — the keystone v0.4.0 work.
//!
//! ## Why this module exists
//!
//! Tree-sitter gives us SYNTAX. It tells us "here is a function called
//! `spawn` declared at `manager.rs:1100`" or "here is a call to
//! `mod::foo()` at `build.rs:42`". What it does NOT tell us is whether
//! THIS `spawn` is the same as THAT `spawn`, or whether `mod::foo` and
//! `crate::mod::foo` and `super::foo` and `use crate::mod; foo()` are
//! all the same canonical symbol.
//!
//! That gap is the difference between mneme's measured 2/10 retrieval
//! hits and CRG's measured 6/10 on the same golden-query suite. CRG
//! has three resolvers (`jedi_resolver.py` for Python,
//! `tsconfig_resolver.py` for TypeScript path aliases,
//! `rescript_resolver.py` for ReScript module resolution); mneme has
//! none. The audit report (CRG-vs-Mneme comparison, 2026-05-05)
//! explicitly identified this as the keystone gap.
//!
//! Every other token-savings + recall fix in v0.4.0 (size budgets,
//! negative cache, smart context injection, edge filter, etc.) is
//! plumbing around the same fundamental answer being unreliable.
//! Without the resolver, recall returns README chunks instead of
//! code; with the resolver, recall returns the function's location
//! and excerpt directly.
//!
//! ## What this module ships in v0.4.0
//!
//! The trait + a Rust skeleton + a TS skeleton + a Python skeleton.
//! Skeletons are deliberate "always-resolve-to-input" identity
//! functions for now — the data structures and integration points
//! are in place so future sessions can fill in the per-language
//! resolution logic without breaking the build.
//!
//! ## v0.4.0 vs v0.4.1+ split
//!
//! - **v0.4.0 (this commit)**: trait + types + per-language skeleton
//!   + integration point in [`crate::extractor::Extractor`]. The
//!   resolver runs on every parse but currently passes through
//!   unchanged.
//!
//! - **v0.4.1**: Rust resolver fills in `use` path resolution,
//!   `super::`/`self::`/`crate::` rewriting, and `pub use`
//!   re-export tracking.
//!
//! - **v0.4.2**: TS/JS resolver (tsconfig path aliases, barrel
//!   re-exports, declaration merging).
//!
//! - **v0.4.3**: Python resolver (jedi-style: relative imports,
//!   `__init__.py` re-exports, namespace packages).
//!
//! - **v0.4.4**: BGE embeddings rebuilt from canonical symbols
//!   (currently file-anchored — that's why `recall_concept "spawn"`
//!   returns README pages instead of the code).
//!
//! Each phase is testable + shippable on its own; the trait gives
//! us the integration surface up front so individual resolvers can
//! land independently.
//!
//! ## Authors
//!
//! Anish Trivedi & Kruti Trivedi. Apache-2.0.

use crate::language::Language;
use std::fmt;

/// A canonical, resolved symbol identifier.
///
/// Two `CanonicalSymbol` values are equal if and only if the resolver
/// proved they refer to the same logical symbol — even if the source
/// code spelled them differently (`mod::foo` vs `crate::mod::foo` vs
/// `super::foo` from a sibling module).
///
/// The string form is intentionally human-readable so it can be stored
/// in `graph.db` directly without an extra symbol-table layer. The
/// resolver guarantees stable strings within a single index pass —
/// across re-indexes the same source produces the same canonical names
/// (deterministic ordering).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalSymbol(String);

impl CanonicalSymbol {
    /// Construct a canonical symbol from its already-resolved string
    /// form. The resolver is responsible for ensuring the string is
    /// in canonical form; this constructor does not validate.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the canonical string. Used by `extractor` when writing
    /// `nodes.qualified_name` and `edges.source_qualified` /
    /// `edges.target_qualified` into `graph.db`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner string. Useful for
    /// places that want to move the string into a `StatementBuilder`
    /// without an extra clone.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for CanonicalSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CanonicalSymbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Per-file context passed to the resolver. Future per-language
/// resolvers will read from this to figure out the source file's
/// own canonical prefix (e.g. for Rust: walk up to the `Cargo.toml`,
/// derive `crate::module::path::to::file`).
///
/// Kept deliberately minimal in v0.4.0 — a real resolver pass needs
/// this to grow with workspace context (tsconfig paths, Python
/// `sys.path`, Cargo workspace members). For now the skeleton just
/// wraps the file path.
#[derive(Debug, Clone)]
pub struct FileContext<'a> {
    /// Path of the source file being resolved, relative to the
    /// project root. Always uses forward slashes for cross-platform
    /// stability.
    pub relative_path: &'a str,
    /// Language detected for this file. Drives which resolver is
    /// dispatched.
    pub language: Language,
}

/// The resolver trait. One implementation per language; dispatched
/// from [`resolve`] below based on `FileContext::language`.
///
/// Every method takes the syntactic name as found in the AST and
/// returns the canonical form. v0.4.0 skeleton just wraps the input
/// — the integration test surface is what matters here, not the
/// quality of resolution. v0.4.1+ fills in the real logic.
pub trait SymbolResolver: fmt::Debug + Send + Sync {
    /// Resolve a definition site (where a symbol is declared).
    ///
    /// Examples (Rust):
    /// - input: `("WorkerPool", FileContext{ "supervisor/src/manager.rs", Rust })`
    /// - output: `crate::manager::WorkerPool`
    ///
    /// The default impl returns the syntactic name unchanged so a
    /// new language can be added without immediately filling in the
    /// resolution logic.
    fn resolve_definition(&self, syntactic_name: &str, _ctx: &FileContext<'_>) -> CanonicalSymbol {
        CanonicalSymbol::new(syntactic_name)
    }

    /// Resolve a reference site (where a symbol is used).
    ///
    /// Examples (Rust):
    /// - input: `("super::spawn", FileContext{ "supervisor/src/health.rs", Rust })`
    /// - output: `crate::manager::spawn`
    ///
    /// Skeleton: return the input unchanged.
    fn resolve_reference(&self, syntactic_name: &str, _ctx: &FileContext<'_>) -> CanonicalSymbol {
        CanonicalSymbol::new(syntactic_name)
    }
}

/// v0.4.0 skeleton: the always-pass-through resolver. This is what
/// `resolve()` returns for every language until per-language resolvers
/// land in v0.4.1+. Equivalent to no-op; the value of having it now
/// is that the integration call site in `extractor` already runs
/// through this trait, so swapping it for a real resolver is a
/// single-line change rather than a big-bang rewrite.
#[derive(Debug, Default)]
pub struct PassthroughResolver;

impl SymbolResolver for PassthroughResolver {}

/// Rust symbol resolver — v0.4.0 keystone.
///
/// Implements the resolution algorithm CRG ships in `jedi_resolver.py`
/// (for Python) but for Rust source: turns syntactic names like
/// `WorkerPool`, `super::spawn`, `crate::manager::WorkerPool`, and
/// `use crate::manager; spawn()` into a single canonical string per
/// logical symbol — the foundation for closing the recall gap that
/// the CRG comparison (2026-05-05) flagged as the keystone issue.
///
/// ## Coverage in this commit
///
/// - File path → canonical module prefix (`src/manager.rs` →
///   `crate::manager`; `src/foo/bar.rs` → `crate::foo::bar`;
///   `src/foo/mod.rs` → `crate::foo`; `src/lib.rs` and `src/main.rs`
///   → `crate`).
/// - `crate::X::Y` references — left as-is; the path is already
///   canonical.
/// - `super::X` — walks one level up the file's prefix and prepends.
/// - `self::X` — replaces with the file's prefix.
/// - Bare names looked up in a [`UseMap`] derived from the file's
///   `use` statements; multi-step aliasing (`use a::b as c`) and
///   group imports (`use a::{b, c::d}`) are flattened on construction.
/// - Cross-crate references (`std::collections::HashMap`) — left
///   verbatim (we don't try to resolve external crates; their
///   canonical form is already what the source spelled).
///
/// ## Out of scope (deferred)
///
/// - `pub use` re-export chasing (v0.4.2): if module A does
///   `pub use crate::B::Foo`, references to `A::Foo` in other files
///   currently resolve to `A::Foo` rather than `crate::B::Foo`.
/// - Trait impl resolution: `<Foo as Bar>::method` stays verbatim.
/// - Generic monomorphization: `Vec<Foo>::new` is left as
///   `Vec::new` (the type parameter is dropped at the syntactic
///   level anyway).
///
/// These need either deeper AST walks (Tree-sitter doesn't carry
/// the cross-file information by itself) or a second pass after all
/// `use` maps are built. v0.4.2 wires that pass; v0.4.1 ships the
/// 80%-case algorithm here.
#[derive(Debug, Default)]
pub struct RustResolver;

impl SymbolResolver for RustResolver {
    fn resolve_definition(&self, syntactic_name: &str, ctx: &FileContext<'_>) -> CanonicalSymbol {
        // A definition site already says "this thing is named X here";
        // the canonical form is `<file_prefix>::<syntactic_name>`
        // unless the syntactic name is already a path (e.g. an `impl
        // Trait for Foo` would emit `Foo::method` with a `::`).
        let prefix = rust_file_prefix(ctx.relative_path);
        if syntactic_name.contains("::") {
            // Already qualified — assume the caller meant it.
            CanonicalSymbol::new(syntactic_name)
        } else if prefix.is_empty() {
            CanonicalSymbol::new(syntactic_name)
        } else {
            CanonicalSymbol::new(format!("{prefix}::{syntactic_name}"))
        }
    }

    fn resolve_reference(&self, syntactic_name: &str, ctx: &FileContext<'_>) -> CanonicalSymbol {
        // Without a use-map we can still rewrite `super::*` and
        // `self::*` deterministically from the file path. Bare names
        // round-trip unchanged (they NEED a use-map to be lifted to
        // canonical) — so callers that have one should use
        // [`Self::resolve_reference_with_uses`] for the full
        // algorithm.
        let prefix = rust_file_prefix(ctx.relative_path);
        CanonicalSymbol::new(rewrite_rust_path(
            syntactic_name,
            &prefix,
            &UseMap::default(),
        ))
    }
}

impl RustResolver {
    /// The full algorithm — takes the file's `use`-map alongside the
    /// reference. Use sites in the extractor walk should call this
    /// rather than the trait method to get the lookup-driven rewrite.
    pub fn resolve_reference_with_uses(
        &self,
        syntactic_name: &str,
        ctx: &FileContext<'_>,
        uses: &UseMap,
    ) -> CanonicalSymbol {
        let prefix = rust_file_prefix(ctx.relative_path);
        CanonicalSymbol::new(rewrite_rust_path(syntactic_name, &prefix, uses))
    }
}

/// Build the canonical module prefix for a Rust source file given
/// its project-relative path. Public so the extractor can call it
/// without having to redo the same logic.
///
/// Examples:
/// - `src/lib.rs` → `crate`
/// - `src/main.rs` → `crate`
/// - `src/manager.rs` → `crate::manager`
/// - `src/foo/mod.rs` → `crate::foo`
/// - `src/foo/bar.rs` → `crate::foo::bar`
/// - `cli/src/commands/build.rs` → `crate::commands::build`
///   (we only consume the part FROM `src/` onward; pre-`src/` is
///   the workspace-member prefix and gets dropped)
/// - non-`src/`-rooted paths → empty string (caller decides how to
///   handle workspace-test fixtures, examples/, benches/, etc.)
pub fn rust_file_prefix(relative_path: &str) -> String {
    // Normalise separators — Windows callers may pass backslashes.
    let normalised = relative_path.replace('\\', "/");
    // Find the last `src/` in the path and treat everything after
    // it as the module path. This handles workspace members
    // (`cli/src/commands/build.rs` → `commands/build.rs`) and the
    // single-crate case (`src/manager.rs` → `manager.rs`)
    // identically.
    let after_src = match normalised.rfind("src/") {
        Some(i) => &normalised[i + 4..],
        None => return String::new(),
    };
    // Strip the .rs extension.
    let trimmed = after_src.strip_suffix(".rs").unwrap_or(after_src);
    // `lib`, `main`, and `mod` are the canonical "this IS the
    // module's root" filenames. Rust doesn't include them in the
    // module path.
    let segments: Vec<&str> = trimmed.split('/').collect();
    let mut prefix_parts: Vec<&str> = Vec::with_capacity(segments.len() + 1);
    prefix_parts.push("crate");
    for (i, seg) in segments.iter().enumerate() {
        // The LAST segment "lib" / "main" / "mod" is dropped (it's
        // the module's own file); intermediate segments named the
        // same way are still real modules and stay.
        let is_last = i + 1 == segments.len();
        if is_last && (*seg == "lib" || *seg == "main" || *seg == "mod") {
            continue;
        }
        prefix_parts.push(seg);
    }
    prefix_parts.join("::")
}

/// A flattened map from `local_alias` → `fully_qualified_path`
/// derived from a Rust file's `use` statements. Construction is
/// the responsibility of the extractor (it has the parsed AST);
/// this struct is the consumer side.
///
/// Multi-segment use paths get one entry per leaf:
///
/// ```text
/// use crate::manager::{WorkerPool, spawn as kick};
/// →  WorkerPool → crate::manager::WorkerPool
///    kick       → crate::manager::spawn
/// ```
#[derive(Debug, Default, Clone)]
pub struct UseMap {
    inner: std::collections::HashMap<String, String>,
}

impl UseMap {
    /// Empty map — used when a file has no `use` statements or when
    /// the extractor hasn't built one yet. References to bare names
    /// then round-trip unchanged.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `local_alias` → `fully_qualified` mapping. If the
    /// same alias is registered twice, the later registration wins
    /// (matches Rust's "last `use` wins" shadowing rule within a
    /// scope).
    pub fn insert(&mut self, local: impl Into<String>, full: impl Into<String>) {
        self.inner.insert(local.into(), full.into());
    }

    /// Look up an alias. Returns `None` if not registered.
    pub fn get(&self, local: &str) -> Option<&str> {
        self.inner.get(local).map(String::as_str)
    }

    /// Number of entries — used by tests and diagnostics.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// True when no entries have been registered.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// The pure rewrite function. Given a syntactic Rust path, the
/// file's canonical prefix, and the file's use-map, return the
/// canonical form. Public so the extractor can call it directly
/// after building the use-map without going through the trait.
pub fn rewrite_rust_path(syntactic: &str, file_prefix: &str, uses: &UseMap) -> String {
    let trimmed = syntactic.trim();

    // Already-canonical paths: `crate::*`, `::*` (absolute path).
    if let Some(rest) = trimmed.strip_prefix("crate::") {
        return format!("crate::{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("::") {
        // Absolute extern path (e.g. `::std::collections::HashMap`).
        // Drop the leading `::` so the canonical form is
        // `std::collections::HashMap`.
        return rest.to_string();
    }

    // `super::*` — count consecutive `super::` and walk up that many
    // levels in one go. Recursive substitution would prepend the
    // parent each pass and end up with `crate::a::b::super::foo`
    // (super:: is no longer at the start, so the next pass falls
    // through). Counting + collapsing is the only correct rewrite.
    if trimmed.starts_with("super::") {
        let mut after_super = trimmed;
        let mut levels = 0usize;
        while let Some(rest) = after_super.strip_prefix("super::") {
            levels += 1;
            after_super = rest;
        }
        let mut prefix = file_prefix.to_string();
        for _ in 0..levels {
            prefix = parent_module(&prefix);
        }
        return if prefix.is_empty() {
            after_super.to_string()
        } else {
            format!("{prefix}::{after_super}")
        };
    }

    // `self::*` — current module.
    if let Some(rest) = trimmed.strip_prefix("self::") {
        return if file_prefix.is_empty() {
            rest.to_string()
        } else {
            format!("{file_prefix}::{rest}")
        };
    }

    // Bare name or `head::tail::leaf`. Look up the head segment in
    // the use-map; if found, replace.
    if let Some((head, tail)) = trimmed.split_once("::") {
        if let Some(full) = uses.get(head) {
            return format!("{full}::{tail}");
        }
        // Head wasn't in the use-map. Could still be an external
        // crate (`std::*`, `tokio::*`) or a top-level module the
        // file imports implicitly. Leave verbatim — the embedder
        // can still match on the leaf.
        return trimmed.to_string();
    }

    // Single-segment: `WorkerPool`. Look up directly.
    if let Some(full) = uses.get(trimmed) {
        return full.to_string();
    }
    // No use entry — could be a primitive (`i32`, `bool`) or a
    // local definition. Return unchanged — the canonical form is
    // the bare name itself.
    trimmed.to_string()
}

/// Helper: drop the rightmost segment of a `crate::a::b::c` path.
fn parent_module(prefix: &str) -> String {
    match prefix.rsplit_once("::") {
        Some((parent, _)) => parent.to_string(),
        None => prefix.to_string(), // already at root
    }
}

/// TypeScript / JavaScript symbol resolver — v0.4.0.
///
/// Closes the same recall gap for TS/JS that [`RustResolver`] closes
/// for Rust. TS has its own resolution rules, so the canonical-name
/// shape differs:
///
///   `src/components/Foo.tsx::Bar`         — named export from a file
///   `src/components/Foo.tsx::default`     — default export
///   `react::useState`                     — bare module specifier
///                                           (external crate-equivalent)
///
/// The `::` separator is the same one Rust uses, so cross-language
/// queries like "where is the spawn function?" can match symbols
/// across the project regardless of source language.
///
/// ## Coverage in this commit
///
/// - File prefix from relative path: `src/components/Foo.tsx` →
///   `src/components/Foo.tsx`. Unlike Rust (which strips `lib`,
///   `main`, `mod`), TS files aren't directory-as-module so the
///   filename is meaningful.
/// - Relative imports: `./Foo` from `src/components/index.ts`
///   resolves to `src/components/Foo`. The resolver doesn't try
///   to find the actual file extension — that's the extractor's
///   job (it has the project root + the AST).
/// - tsconfig.json `paths` aliases: `@/components/Foo` rewrites to
///   `src/components/Foo` when the alias map has `@/*` → `src/*`.
///   The [`TsPathAliases`] struct captures this from a parsed
///   tsconfig.json — construction is the extractor's job.
/// - Bare module specifiers (`react`, `lodash`, `@scope/pkg`):
///   passed through verbatim — external packages can't be resolved
///   to a file the project owns.
///
/// ## Out of scope (deferred)
///
/// - Barrel re-export chasing (`export * from './x'`): needs a
///   second pass once all module exports are mapped.
/// - Declaration merging across files (interface X in two places).
/// - Implicit index resolution (`./components` → `./components/index.ts`).
/// - ESM `package.json` `exports` field.
/// - Type-only vs value imports — the resolver treats them
///   identically since both contribute to the canonical surface.
#[derive(Debug, Default)]
pub struct TypeScriptResolver;

impl SymbolResolver for TypeScriptResolver {
    fn resolve_definition(&self, syntactic_name: &str, ctx: &FileContext<'_>) -> CanonicalSymbol {
        let prefix = ts_file_prefix(ctx.relative_path);
        if syntactic_name.contains("::") {
            CanonicalSymbol::new(syntactic_name)
        } else if prefix.is_empty() {
            CanonicalSymbol::new(syntactic_name)
        } else {
            CanonicalSymbol::new(format!("{prefix}::{syntactic_name}"))
        }
    }

    fn resolve_reference(&self, syntactic_name: &str, ctx: &FileContext<'_>) -> CanonicalSymbol {
        // Without an alias map we can still resolve relative imports
        // (`./foo`, `../bar`) deterministically from the file path.
        let aliases = TsPathAliases::default();
        CanonicalSymbol::new(rewrite_ts_module_specifier(
            syntactic_name,
            ctx.relative_path,
            &aliases,
        ))
    }
}

impl TypeScriptResolver {
    /// Full algorithm — takes the project's tsconfig path aliases.
    pub fn resolve_reference_with_aliases(
        &self,
        syntactic_name: &str,
        ctx: &FileContext<'_>,
        aliases: &TsPathAliases,
    ) -> CanonicalSymbol {
        CanonicalSymbol::new(rewrite_ts_module_specifier(
            syntactic_name,
            ctx.relative_path,
            aliases,
        ))
    }
}

/// File-prefix for TS/JS canonical names. Unlike Rust we keep the
/// extension because TS files aren't directory-as-module — `Foo.tsx`
/// vs `Foo.ts` vs `Foo.js` are distinct files with potentially
/// distinct exports. Path uses forward slashes always.
pub fn ts_file_prefix(relative_path: &str) -> String {
    relative_path.replace('\\', "/")
}

/// tsconfig.json `compilerOptions.paths` alias map. The extractor
/// builds one per project from the parsed JSON; this struct is the
/// consumer side. Each entry is a `(prefix_pattern, replacement)`
/// pair — both can end with `*` for wildcard match.
///
/// Example tsconfig:
/// ```json
/// {
///   "compilerOptions": {
///     "paths": {
///       "@/*": ["src/*"],
///       "@components/*": ["src/components/*"],
///       "~/utils": ["src/utils/index"]
///     }
///   }
/// }
/// ```
#[derive(Debug, Default, Clone)]
pub struct TsPathAliases {
    rules: Vec<(String, String)>,
}

impl TsPathAliases {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `pattern` → `replacement` rule. Patterns may end
    /// with `*` for wildcard match; if so, replacement should also
    /// end with `*` and the matched suffix is substituted.
    pub fn insert(&mut self, pattern: impl Into<String>, replacement: impl Into<String>) {
        self.rules.push((pattern.into(), replacement.into()));
    }

    /// Try to apply each rule in order; return the first match.
    /// Wildcard rules end with `*`; suffix substitution preserves
    /// the path beyond the prefix.
    pub fn rewrite(&self, specifier: &str) -> Option<String> {
        for (pattern, replacement) in &self.rules {
            if let Some(p_prefix) = pattern.strip_suffix('*') {
                if let Some(suffix) = specifier.strip_prefix(p_prefix) {
                    let r_prefix = replacement.strip_suffix('*').unwrap_or(replacement);
                    return Some(format!("{r_prefix}{suffix}"));
                }
            } else if specifier == pattern {
                return Some(replacement.clone());
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Pure rewrite for TS/JS module specifiers. Handles the four
/// canonical shapes:
///
/// 1. tsconfig `paths` alias hit → use the rewritten target.
/// 2. Bare specifier (`react`, `@scope/pkg`) → pass through verbatim.
/// 3. Relative (`./x`, `../x`) → resolved against `from_file`.
/// 4. Already absolute project-relative path (`src/foo`) → pass through.
pub fn rewrite_ts_module_specifier(
    specifier: &str,
    from_file: &str,
    aliases: &TsPathAliases,
) -> String {
    // 1. tsconfig path alias.
    if let Some(rewritten) = aliases.rewrite(specifier) {
        return rewritten;
    }
    // 2. Relative specifier — resolve against the importing file's
    //    directory. Path semantics are forward-slash only.
    if specifier.starts_with("./") || specifier.starts_with("../") {
        let from = from_file.replace('\\', "/");
        let from_dir = match from.rfind('/') {
            Some(i) => &from[..i],
            None => "",
        };
        return resolve_relative(from_dir, specifier);
    }
    // 3. Bare module / absolute project path / unknown — passthrough.
    specifier.to_string()
}

/// Path-segment resolution for `./` and `../` specifiers. Walks
/// the from_dir's segments and composes with the specifier's,
/// canonicalising `.` and `..` along the way.
fn resolve_relative(from_dir: &str, specifier: &str) -> String {
    let mut segments: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').collect()
    };
    for piece in specifier.split('/') {
        match piece {
            "." | "" => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// Python symbol resolver — v0.4.0.
///
/// Closes the same recall gap [`RustResolver`] / [`TypeScriptResolver`]
/// close, adapted for Python's import semantics. Canonical names use
/// the dot-separator convention Python tooling already speaks (jedi,
/// mypy, pylint, sphinx) so a stored symbol like
/// `pkg.subpkg.module.foo` matches what a user types when searching.
///
/// ## Why dots, not `::`
///
/// Rust and TS share `::` as their canonical separator because the
/// resolver-side string is ours to choose, but Python's ecosystem
/// already canonicalises on dots: `import pkg.sub.mod`,
/// `pkg.sub.mod.foo`, `__all__ = ['foo']`. Forcing `::` here would
/// break round-tripping with every Python tool downstream of mneme.
/// The cross-language match is unaffected: queries like "where is
/// `spawn`?" still hit because the leaf segment is the same — only
/// the separator differs, and the embedder anchors on the leaf.
///
/// ## Coverage in this commit
///
/// - File path → canonical module path
///   (`pkg/sub/mod.py` → `pkg.sub.mod`,
///    `pkg/sub/__init__.py` → `pkg.sub`,
///    `pkg/__main__.py` → `pkg.__main__`,
///    `cli.py` → `cli`,
///    `src/pkg/foo.py` → `pkg.foo` — leading `src/` is dropped).
/// - Relative imports: `.x` and `..pkg.y` resolve against the file's
///   canonical prefix (one parent walk per leading dot).
/// - Absolute imports: `pkg.sub.mod` left verbatim (already canonical).
/// - Aliased imports via [`PythonImportMap`]: `import x.y as z` →
///   `z` resolves to `x.y`, `from x import foo as bar` → `bar`
///   resolves to `x.foo`. Built by the extractor when it walks the
///   file's `import_statement` / `import_from_statement` nodes.
/// - Star imports (`from x import *`): the wildcard itself isn't a
///   real symbol; references that show up as bare names against a
///   star-imported module fall through to the bare-name path. The
///   downstream `__all__` chasing that would resolve `foo()` back to
///   `x.foo` is deferred to v0.4.2.
///
/// ## Out of scope (deferred)
///
/// - `__all__` re-export chasing across `from x import *`.
/// - Namespace packages (PEP 420) where `__init__.py` is absent —
///   the file-prefix builder treats every directory the same way,
///   so this works in the common case but doesn't model the full
///   PEP 420 multi-portion semantics.
/// - Class-scoped name resolution (jedi-style scope walking) —
///   Tree-sitter can't carry runtime scope without a second pass,
///   and v0.4.0 does not add one.
/// - Implicit relative imports (Python 2 style) — Python 3 only.
#[derive(Debug, Default)]
pub struct PythonResolver;

impl SymbolResolver for PythonResolver {
    fn resolve_definition(&self, syntactic_name: &str, ctx: &FileContext<'_>) -> CanonicalSymbol {
        // Definition: `<file_prefix>.<name>` unless the syntactic name
        // is already a dotted path (e.g. extractor emits `Class.method`
        // for class methods).
        let prefix = python_file_prefix(ctx.relative_path);
        if syntactic_name.contains('.') {
            CanonicalSymbol::new(syntactic_name)
        } else if prefix.is_empty() {
            CanonicalSymbol::new(syntactic_name)
        } else {
            CanonicalSymbol::new(format!("{prefix}.{syntactic_name}"))
        }
    }

    fn resolve_reference(&self, syntactic_name: &str, ctx: &FileContext<'_>) -> CanonicalSymbol {
        // Without an import-map we can still rewrite leading-dot
        // relative imports deterministically from the file path.
        let prefix = python_file_prefix(ctx.relative_path);
        CanonicalSymbol::new(rewrite_python_path(
            syntactic_name,
            &prefix,
            &PythonImportMap::default(),
        ))
    }
}

impl PythonResolver {
    /// Full algorithm — takes the file's import-map alongside the
    /// reference. Use sites in the extractor walk should call this
    /// rather than the trait method to get the lookup-driven rewrite.
    pub fn resolve_reference_with_imports(
        &self,
        syntactic_name: &str,
        ctx: &FileContext<'_>,
        imports: &PythonImportMap,
    ) -> CanonicalSymbol {
        let prefix = python_file_prefix(ctx.relative_path);
        CanonicalSymbol::new(rewrite_python_path(syntactic_name, &prefix, imports))
    }
}

/// Build the canonical Python module path for a source file given its
/// project-relative path. Public so the extractor can call it without
/// having to redo the same logic.
///
/// Examples:
/// - `cli.py` → `cli`
/// - `pkg/__init__.py` → `pkg`
/// - `pkg/sub/mod.py` → `pkg.sub.mod`
/// - `pkg/sub/__init__.py` → `pkg.sub`
/// - `pkg/__main__.py` → `pkg.__main__` (unlike `__init__`, the
///   `__main__` entry point is itself a module name)
/// - `src/pkg/foo.py` → `pkg.foo` — a leading `src/` segment is
///   dropped (mirrors the Rust resolver, which strips the workspace
///   `src/` prefix). Same for `lib/`.
/// - non-`.py` paths → empty string (caller decides how to handle
///   them — typically these are out-of-scope for resolution).
pub fn python_file_prefix(relative_path: &str) -> String {
    let normalised = relative_path.replace('\\', "/");
    // Only resolve `.py` source files.
    let trimmed = match normalised.strip_suffix(".py") {
        Some(t) => t,
        None => return String::new(),
    };
    // Strip a leading `src/` or `lib/` segment so `src/pkg/foo.py`
    // and `pkg/foo.py` both canonicalise to `pkg.foo`. This matches
    // common Python project layouts (`src/`-style and flat).
    let stripped = trimmed
        .strip_prefix("src/")
        .or_else(|| trimmed.strip_prefix("lib/"))
        .unwrap_or(trimmed);
    let segments: Vec<&str> = stripped.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return String::new();
    }
    let mut parts: Vec<&str> = Vec::with_capacity(segments.len());
    for (i, seg) in segments.iter().enumerate() {
        let is_last = i + 1 == segments.len();
        // `__init__.py` IS the package's module — its filename is
        // dropped, the directory name takes over. Everything else
        // (including `__main__`) is a real module name.
        if is_last && *seg == "__init__" {
            continue;
        }
        parts.push(seg);
    }
    parts.join(".")
}

/// A flattened map from `local_name` → `fully_qualified_path` derived
/// from a Python file's `import` and `from ... import ...` statements.
/// Construction is the responsibility of the extractor (it has the
/// parsed AST); this struct is the consumer side.
///
/// Examples of what the extractor flattens into entries:
///
/// ```text
/// import collections                  →  collections        → collections
/// import os.path as osp               →  osp                → os.path
/// from .baz import qux                →  qux                → <pkg>.baz.qux  (already absolute)
/// from .baz import qux as kick        →  kick               → <pkg>.baz.qux
/// from x.y import a, b as c           →  a → x.y.a, c → x.y.b
/// ```
///
/// Relative-import resolution (`.baz` → `<pkg>.baz`) is the
/// extractor's responsibility — it has the file path and can call
/// [`resolve_python_relative`] before inserting into the map. By the
/// time entries arrive here, every `full` value is absolute.
#[derive(Debug, Default, Clone)]
pub struct PythonImportMap {
    inner: std::collections::HashMap<String, String>,
}

impl PythonImportMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `local_name` → `fully_qualified` mapping. If the
    /// same local name is registered twice, the later registration
    /// wins (matches Python's "last `import` shadows" rule).
    pub fn insert(&mut self, local: impl Into<String>, full: impl Into<String>) {
        self.inner.insert(local.into(), full.into());
    }

    /// Look up a local name. Returns `None` if not registered.
    pub fn get(&self, local: &str) -> Option<&str> {
        self.inner.get(local).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Resolve a relative Python import string against the importing
/// file's canonical module prefix.
///
/// Python's relative-import grammar:
/// - one leading dot = the importing file's own package
/// - each additional leading dot = one parent level
/// - the rest of the string (after the dots) is the path within
///   that level
///
/// Examples:
/// - file `pkg.sub.mod`, import `.x`     → `pkg.sub.x`
/// - file `pkg.sub.mod`, import `..foo`  → `pkg.foo`
/// - file `pkg.sub.mod`, import `.`      → `pkg.sub`
/// - file `pkg.sub.mod`, import `...top` → `top`  (walked all the way up)
///
/// If the walk-up exceeds the available levels (e.g. four dots from
/// `pkg.sub.mod`), the surplus dots are absorbed and the result is
/// just the after-dot remainder — Python would raise
/// `ImportError` at runtime, but for canonical-name purposes the
/// best-effort resolution is preferable to dropping the symbol.
pub fn resolve_python_relative(import: &str, file_prefix: &str) -> String {
    if !import.starts_with('.') {
        return import.to_string();
    }
    // Count leading dots.
    let dots = import.chars().take_while(|c| *c == '.').count();
    let remainder = &import[dots..];
    // First dot anchors at the file's own package; each additional
    // dot walks one parent up. So a file at `pkg.sub.mod` with `.x`
    // (1 dot) anchors at `pkg.sub`; with `..x` (2 dots) walks up
    // once to `pkg`; with `...x` (3 dots) walks up twice to `` (root).
    let mut anchor = parent_python_module(file_prefix);
    for _ in 1..dots {
        anchor = parent_python_module(&anchor);
    }
    if remainder.is_empty() {
        anchor
    } else if anchor.is_empty() {
        remainder.to_string()
    } else {
        format!("{anchor}.{remainder}")
    }
}

/// Pure rewrite for Python references. Given a syntactic Python name,
/// the file's canonical prefix, and the file's import-map, return the
/// canonical form. Public so the extractor can call it directly after
/// building the import-map without going through the trait.
pub fn rewrite_python_path(
    syntactic: &str,
    file_prefix: &str,
    imports: &PythonImportMap,
) -> String {
    let trimmed = syntactic.trim();

    // Leading-dot relative import: resolve against file prefix.
    if trimmed.starts_with('.') {
        return resolve_python_relative(trimmed, file_prefix);
    }

    // Dotted reference: look up the head in the import-map. If found,
    // splice in the canonical path; tail is preserved.
    if let Some((head, tail)) = trimmed.split_once('.') {
        if let Some(full) = imports.get(head) {
            return format!("{full}.{tail}");
        }
        // Head wasn't aliased — could be a stdlib / third-party
        // reference (`os.path.join`, `numpy.array`) or a self-rooted
        // dotted name. Pass through verbatim.
        return trimmed.to_string();
    }

    // Bare single-segment name. Look up directly.
    if let Some(full) = imports.get(trimmed) {
        return full.to_string();
    }
    // No import entry — could be a builtin (`int`, `len`, `range`)
    // or a local definition. Pass through unchanged.
    trimmed.to_string()
}

/// Drop the rightmost dot-separated segment of a Python module path.
fn parent_python_module(prefix: &str) -> String {
    match prefix.rsplit_once('.') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(), // already at root
    }
}

/// Dispatch entry point. Picks the per-language resolver and runs
/// reference + definition resolution. v0.4.0 always uses the
/// passthrough trait default — this is the call site `extractor`
/// will plug into when the v0.4.1 Rust resolver lands.
pub fn resolver_for(language: Language) -> Box<dyn SymbolResolver> {
    match language {
        Language::Rust => Box::new(RustResolver),
        Language::TypeScript | Language::Tsx | Language::JavaScript | Language::Jsx => {
            Box::new(TypeScriptResolver)
        }
        Language::Python => Box::new(PythonResolver),
        // All other languages fall through to the passthrough until
        // their resolvers land in v0.5+ (the resolver-per-language
        // model is genuinely O(N) work).
        _ => Box::new(PassthroughResolver),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(language: Language) -> FileContext<'static> {
        FileContext {
            relative_path: "src/foo.rs",
            language,
        }
    }

    #[test]
    fn passthrough_returns_input_unchanged() {
        let r = PassthroughResolver;
        let c = ctx(Language::Rust);
        assert_eq!(
            r.resolve_definition("WorkerPool", &c).as_str(),
            "WorkerPool"
        );
        assert_eq!(
            r.resolve_reference("crate::manager::spawn", &c).as_str(),
            "crate::manager::spawn"
        );
    }

    #[test]
    fn rust_file_prefix_handles_canonical_paths() {
        // Single-crate: `src/lib.rs` is the crate root.
        assert_eq!(rust_file_prefix("src/lib.rs"), "crate");
        assert_eq!(rust_file_prefix("src/main.rs"), "crate");
        // Top-level module file.
        assert_eq!(rust_file_prefix("src/manager.rs"), "crate::manager");
        // Module via mod.rs.
        assert_eq!(rust_file_prefix("src/foo/mod.rs"), "crate::foo");
        // Nested module.
        assert_eq!(rust_file_prefix("src/foo/bar.rs"), "crate::foo::bar");
        // Workspace member: only the part FROM `src/` is kept.
        assert_eq!(
            rust_file_prefix("cli/src/commands/build.rs"),
            "crate::commands::build"
        );
        // Workspace member with mod.rs.
        assert_eq!(
            rust_file_prefix("supervisor/src/manager/mod.rs"),
            "crate::manager"
        );
        // Windows-style path.
        assert_eq!(
            rust_file_prefix(r"cli\src\commands\build.rs"),
            "crate::commands::build"
        );
        // Path without a `src/` segment — outside scope, returns empty.
        assert_eq!(rust_file_prefix("examples/hello.rs"), "");
        assert_eq!(rust_file_prefix("benches/bench.rs"), "");
    }

    #[test]
    fn rust_definition_qualifies_with_file_prefix() {
        let r = RustResolver;
        let c = FileContext {
            relative_path: "supervisor/src/manager.rs",
            language: Language::Rust,
        };
        assert_eq!(
            r.resolve_definition("WorkerPool", &c).as_str(),
            "crate::manager::WorkerPool"
        );
        // Method-style def: already has `::` — left as-is.
        assert_eq!(
            r.resolve_definition("WorkerPool::spawn", &c).as_str(),
            "WorkerPool::spawn"
        );
    }

    #[test]
    fn rewrite_rust_super_walks_up_one_level() {
        let uses = UseMap::default();
        // From manager/health.rs, `super::spawn` resolves to
        // crate::manager::spawn.
        assert_eq!(
            rewrite_rust_path("super::spawn", "crate::manager::health", &uses),
            "crate::manager::spawn"
        );
        // Two-level super::super:: walks up twice.
        assert_eq!(
            rewrite_rust_path("super::super::foo", "crate::a::b::c", &uses),
            "crate::a::foo"
        );
    }

    #[test]
    fn rewrite_rust_self_uses_current_module() {
        let uses = UseMap::default();
        assert_eq!(
            rewrite_rust_path("self::helper", "crate::manager", &uses),
            "crate::manager::helper"
        );
    }

    #[test]
    fn rewrite_rust_crate_path_passes_through() {
        let uses = UseMap::default();
        assert_eq!(
            rewrite_rust_path("crate::manager::WorkerPool", "crate::supervisor", &uses),
            "crate::manager::WorkerPool"
        );
    }

    #[test]
    fn rewrite_rust_extern_absolute_path_drops_leading_colons() {
        let uses = UseMap::default();
        assert_eq!(
            rewrite_rust_path("::std::collections::HashMap", "crate", &uses),
            "std::collections::HashMap"
        );
    }

    #[test]
    fn rewrite_rust_use_alias_substitutes_head() {
        let mut uses = UseMap::new();
        // `use crate::manager::WorkerPool;` → bare `WorkerPool`
        // resolves to the full path.
        uses.insert("WorkerPool", "crate::manager::WorkerPool");
        assert_eq!(
            rewrite_rust_path("WorkerPool", "crate::supervisor", &uses),
            "crate::manager::WorkerPool"
        );
        // `use crate::manager::spawn as kick;` → `kick(...)` resolves
        // to `crate::manager::spawn`.
        uses.insert("kick", "crate::manager::spawn");
        assert_eq!(
            rewrite_rust_path("kick", "crate::supervisor", &uses),
            "crate::manager::spawn"
        );
    }

    #[test]
    fn rewrite_rust_use_with_method_call_keeps_tail() {
        let mut uses = UseMap::new();
        uses.insert("WorkerPool", "crate::manager::WorkerPool");
        // `WorkerPool::spawn` → head substituted, tail preserved.
        assert_eq!(
            rewrite_rust_path("WorkerPool::spawn", "crate::supervisor", &uses),
            "crate::manager::WorkerPool::spawn"
        );
    }

    #[test]
    fn rewrite_rust_unknown_head_passes_through() {
        let uses = UseMap::default();
        // Not in use-map and not super/self/crate/:: — could be an
        // external crate or a primitive. Pass through verbatim.
        assert_eq!(
            rewrite_rust_path("std::collections::HashMap", "crate::supervisor", &uses),
            "std::collections::HashMap"
        );
        assert_eq!(rewrite_rust_path("i32", "crate::supervisor", &uses), "i32");
    }

    #[test]
    fn rust_resolver_full_algorithm_end_to_end() {
        let r = RustResolver;
        let c = FileContext {
            relative_path: "supervisor/src/manager/health.rs",
            language: Language::Rust,
        };
        let mut uses = UseMap::new();
        uses.insert("WorkerPool", "crate::manager::WorkerPool");
        uses.insert("Result", "std::result::Result"); // external

        // super::spawn — walks up from crate::manager::health.
        assert_eq!(
            r.resolve_reference_with_uses("super::spawn", &c, &uses)
                .as_str(),
            "crate::manager::spawn"
        );
        // self::sub_helper — current module.
        assert_eq!(
            r.resolve_reference_with_uses("self::sub_helper", &c, &uses)
                .as_str(),
            "crate::manager::health::sub_helper"
        );
        // Bare alias — looked up.
        assert_eq!(
            r.resolve_reference_with_uses("WorkerPool", &c, &uses)
                .as_str(),
            "crate::manager::WorkerPool"
        );
        // Aliased method call — head substituted.
        assert_eq!(
            r.resolve_reference_with_uses("WorkerPool::spawn", &c, &uses)
                .as_str(),
            "crate::manager::WorkerPool::spawn"
        );
        // Already canonical — passes through.
        assert_eq!(
            r.resolve_reference_with_uses("crate::supervisor::run", &c, &uses,)
                .as_str(),
            "crate::supervisor::run"
        );
    }

    #[test]
    fn rust_resolver_trait_method_handles_super_without_use_map() {
        // The plain trait method (no use-map) still resolves
        // super::* / self::* / crate::* from path alone.
        let r = RustResolver;
        let c = FileContext {
            relative_path: "supervisor/src/manager/health.rs",
            language: Language::Rust,
        };
        assert_eq!(
            r.resolve_reference("super::spawn", &c).as_str(),
            "crate::manager::spawn"
        );
        // Bare names without a use-map are untouched.
        assert_eq!(r.resolve_reference("WorkerPool", &c).as_str(), "WorkerPool");
    }

    #[test]
    fn use_map_last_insert_wins() {
        let mut uses = UseMap::new();
        uses.insert("Foo", "crate::a::Foo");
        uses.insert("Foo", "crate::b::Foo");
        assert_eq!(uses.get("Foo"), Some("crate::b::Foo"));
    }

    #[test]
    fn ts_definition_qualifies_with_file_prefix() {
        let r = TypeScriptResolver;
        let c = FileContext {
            relative_path: "vision/src/views/ForceGalaxy.tsx",
            language: Language::TypeScript,
        };
        assert_eq!(
            r.resolve_definition("ForceGalaxy", &c).as_str(),
            "vision/src/views/ForceGalaxy.tsx::ForceGalaxy"
        );
    }

    #[test]
    fn rewrite_ts_relative_imports_resolve_against_importing_file() {
        let aliases = TsPathAliases::default();
        // Same-dir sibling.
        assert_eq!(
            rewrite_ts_module_specifier("./Sidebar", "src/components/Layout.tsx", &aliases),
            "src/components/Sidebar"
        );
        // One level up.
        assert_eq!(
            rewrite_ts_module_specifier("../api/graph", "src/views/ForceGalaxy.tsx", &aliases),
            "src/api/graph"
        );
        // Two levels up.
        assert_eq!(
            rewrite_ts_module_specifier("../../utils/format", "src/views/sub/Inner.tsx", &aliases),
            "src/utils/format"
        );
        // `./` no-op + dotted segment.
        assert_eq!(
            rewrite_ts_module_specifier("./", "src/components/Layout.tsx", &aliases),
            "src/components"
        );
    }

    #[test]
    fn rewrite_ts_tsconfig_wildcard_alias() {
        let mut aliases = TsPathAliases::new();
        aliases.insert("@/*", "src/*");
        // `@/components/Foo` rewrites via the wildcard.
        assert_eq!(
            rewrite_ts_module_specifier("@/components/Foo", "x.ts", &aliases),
            "src/components/Foo"
        );
    }

    #[test]
    fn rewrite_ts_tsconfig_exact_alias() {
        let mut aliases = TsPathAliases::new();
        aliases.insert("~/utils", "src/utils/index");
        // Exact match (no wildcard) replaces directly.
        assert_eq!(
            rewrite_ts_module_specifier("~/utils", "x.ts", &aliases),
            "src/utils/index"
        );
        // Same alias prefix as a different specifier doesn't match.
        assert_eq!(
            rewrite_ts_module_specifier("~/utilities", "x.ts", &aliases),
            "~/utilities"
        );
    }

    #[test]
    fn rewrite_ts_bare_module_specifier_passes_through() {
        let aliases = TsPathAliases::default();
        // External packages — can't resolve to a project file.
        assert_eq!(
            rewrite_ts_module_specifier("react", "src/Foo.tsx", &aliases),
            "react"
        );
        assert_eq!(
            rewrite_ts_module_specifier(
                "@modelcontextprotocol/sdk/server/index.js",
                "src/Foo.tsx",
                &aliases
            ),
            "@modelcontextprotocol/sdk/server/index.js"
        );
    }

    #[test]
    fn rewrite_ts_alias_takes_precedence_over_relative_lookalike() {
        // If the project mapped @/* → src/*, an "@/foo" specifier
        // should hit the alias path FIRST. We confirm by registering
        // the alias and asserting the alias wins.
        let mut aliases = TsPathAliases::new();
        aliases.insert("@/*", "src/*");
        // Even if the importing file is in a deep dir, the alias
        // resolves to the absolute project path.
        assert_eq!(
            rewrite_ts_module_specifier("@/components/Foo", "src/views/sub/inner/x.tsx", &aliases),
            "src/components/Foo"
        );
    }

    #[test]
    fn ts_resolver_full_algorithm_end_to_end() {
        let r = TypeScriptResolver;
        let c = FileContext {
            relative_path: "vision/src/views/ForceGalaxy.tsx",
            language: Language::TypeScript,
        };
        let mut aliases = TsPathAliases::new();
        aliases.insert("@/*", "vision/src/*");

        // Relative import.
        assert_eq!(
            r.resolve_reference_with_aliases("../api/graph", &c, &aliases)
                .as_str(),
            "vision/src/api/graph"
        );
        // Alias.
        assert_eq!(
            r.resolve_reference_with_aliases("@/components/Legend", &c, &aliases)
                .as_str(),
            "vision/src/components/Legend"
        );
        // External.
        assert_eq!(
            r.resolve_reference_with_aliases("react", &c, &aliases)
                .as_str(),
            "react"
        );
    }

    #[test]
    fn ts_resolver_trait_method_handles_relative_without_aliases() {
        let r = TypeScriptResolver;
        let c = FileContext {
            relative_path: "vision/src/views/ForceGalaxy.tsx",
            language: Language::TypeScript,
        };
        // Relative imports work even without an alias map.
        assert_eq!(
            r.resolve_reference("./Legend", &c).as_str(),
            "vision/src/views/Legend"
        );
        // Bare aliases without an alias map round-trip unchanged.
        assert_eq!(
            r.resolve_reference("@/components/Foo", &c).as_str(),
            "@/components/Foo"
        );
    }

    #[test]
    fn ts_path_aliases_first_match_wins() {
        let mut aliases = TsPathAliases::new();
        aliases.insert("@/components/*", "vision/src/components/*");
        aliases.insert("@/*", "vision/src/*");
        // The more-specific rule was registered first; we honor
        // insertion order so callers get to express ordering.
        assert_eq!(
            aliases.rewrite("@/components/Foo"),
            Some("vision/src/components/Foo".to_string())
        );
        assert_eq!(
            aliases.rewrite("@/api/graph"),
            Some("vision/src/api/graph".to_string())
        );
    }

    #[test]
    fn python_file_prefix_handles_canonical_paths() {
        // Top-level module file.
        assert_eq!(python_file_prefix("cli.py"), "cli");
        // Package init: the directory IS the module.
        assert_eq!(python_file_prefix("pkg/__init__.py"), "pkg");
        // Nested module.
        assert_eq!(python_file_prefix("pkg/sub/mod.py"), "pkg.sub.mod");
        // Nested package init.
        assert_eq!(python_file_prefix("pkg/sub/__init__.py"), "pkg.sub");
        // `__main__` entry point — a real module name, NOT dropped.
        assert_eq!(python_file_prefix("pkg/__main__.py"), "pkg.__main__");
        // Leading `src/` is dropped (common Python project layout).
        assert_eq!(python_file_prefix("src/pkg/foo.py"), "pkg.foo");
        // Leading `lib/` is also dropped.
        assert_eq!(python_file_prefix("lib/mypkg/util.py"), "mypkg.util");
        // Windows-style path.
        assert_eq!(python_file_prefix(r"pkg\sub\mod.py"), "pkg.sub.mod");
        // Non-`.py` paths are out of scope — return empty.
        assert_eq!(python_file_prefix("README.md"), "");
        assert_eq!(python_file_prefix("pkg/sub"), "");
    }

    #[test]
    fn python_definition_qualifies_with_file_prefix() {
        let r = PythonResolver;
        let c = FileContext {
            relative_path: "pkg/sub/mod.py",
            language: Language::Python,
        };
        assert_eq!(r.resolve_definition("foo", &c).as_str(), "pkg.sub.mod.foo");
        // Class method already has a dot — left as-is.
        assert_eq!(
            r.resolve_definition("MyClass.method", &c).as_str(),
            "MyClass.method"
        );
    }

    #[test]
    fn resolve_python_relative_walks_parents() {
        // 1 dot: anchor at file's own package.
        assert_eq!(resolve_python_relative(".x", "pkg.sub.mod"), "pkg.sub.x");
        // 2 dots: one parent up.
        assert_eq!(resolve_python_relative("..foo", "pkg.sub.mod"), "pkg.foo");
        // 3 dots: two parents up — ends at root, joined with remainder.
        assert_eq!(resolve_python_relative("...top", "pkg.sub.mod"), "top");
        // Bare dot (`.`): the package itself, no remainder.
        assert_eq!(resolve_python_relative(".", "pkg.sub.mod"), "pkg.sub");
        // Surplus dots beyond the available depth — absorbs and emits
        // remainder. Python would raise ImportError; we prefer
        // best-effort to dropping the symbol entirely.
        assert_eq!(resolve_python_relative("....x", "pkg.sub.mod"), "x");
        // Non-relative: passthrough.
        assert_eq!(
            resolve_python_relative("absolute.pkg.mod", "pkg.sub.mod"),
            "absolute.pkg.mod"
        );
    }

    #[test]
    fn rewrite_python_relative_imports_resolve_against_file_prefix() {
        let imports = PythonImportMap::default();
        // Sibling module.
        assert_eq!(
            rewrite_python_path(".sibling", "pkg.sub.mod", &imports),
            "pkg.sub.sibling"
        );
        // Parent package member.
        assert_eq!(
            rewrite_python_path("..foo.bar", "pkg.sub.mod", &imports),
            "pkg.foo.bar"
        );
    }

    #[test]
    fn rewrite_python_absolute_path_passes_through() {
        let imports = PythonImportMap::default();
        // Absolute imports without an alias hit are left verbatim;
        // they're already canonical.
        assert_eq!(
            rewrite_python_path("os.path.join", "pkg.sub.mod", &imports),
            "os.path.join"
        );
        assert_eq!(
            rewrite_python_path("collections.OrderedDict", "pkg.sub.mod", &imports),
            "collections.OrderedDict"
        );
    }

    #[test]
    fn rewrite_python_alias_substitutes_head() {
        let mut imports = PythonImportMap::new();
        // `import os.path as osp` — alias `osp` → `os.path`.
        imports.insert("osp", "os.path");
        assert_eq!(
            rewrite_python_path("osp.join", "pkg.sub.mod", &imports),
            "os.path.join"
        );
        // `from collections import deque` — bare name `deque` →
        // `collections.deque`.
        imports.insert("deque", "collections.deque");
        assert_eq!(
            rewrite_python_path("deque", "pkg.sub.mod", &imports),
            "collections.deque"
        );
        // `from collections import deque as dq` — aliased.
        imports.insert("dq", "collections.deque");
        assert_eq!(
            rewrite_python_path("dq", "pkg.sub.mod", &imports),
            "collections.deque"
        );
    }

    #[test]
    fn rewrite_python_unknown_head_passes_through() {
        let imports = PythonImportMap::default();
        // No alias hit — could be a builtin, local, or stdlib
        // reference. Pass through unchanged.
        assert_eq!(rewrite_python_path("len", "pkg.sub.mod", &imports), "len");
        assert_eq!(
            rewrite_python_path("MyLocalClass", "pkg.sub.mod", &imports),
            "MyLocalClass"
        );
    }

    #[test]
    fn python_resolver_full_algorithm_end_to_end() {
        let r = PythonResolver;
        let c = FileContext {
            relative_path: "pkg/sub/mod.py",
            language: Language::Python,
        };
        let mut imports = PythonImportMap::new();
        imports.insert("osp", "os.path");
        imports.insert("deque", "collections.deque");

        // Relative import (1 dot).
        assert_eq!(
            r.resolve_reference_with_imports(".helper", &c, &imports)
                .as_str(),
            "pkg.sub.helper"
        );
        // Relative import (2 dots).
        assert_eq!(
            r.resolve_reference_with_imports("..foo.bar", &c, &imports)
                .as_str(),
            "pkg.foo.bar"
        );
        // Bare alias — looked up.
        assert_eq!(
            r.resolve_reference_with_imports("deque", &c, &imports)
                .as_str(),
            "collections.deque"
        );
        // Aliased dotted call — head substituted.
        assert_eq!(
            r.resolve_reference_with_imports("osp.join", &c, &imports)
                .as_str(),
            "os.path.join"
        );
        // Already absolute, no alias hit — passthrough.
        assert_eq!(
            r.resolve_reference_with_imports("numpy.array", &c, &imports)
                .as_str(),
            "numpy.array"
        );
    }

    #[test]
    fn python_resolver_trait_method_handles_relative_without_imports() {
        // The plain trait method (no import-map) still resolves
        // relative imports from path alone.
        let r = PythonResolver;
        let c = FileContext {
            relative_path: "pkg/sub/mod.py",
            language: Language::Python,
        };
        assert_eq!(
            r.resolve_reference(".helper", &c).as_str(),
            "pkg.sub.helper"
        );
        assert_eq!(r.resolve_reference("..foo", &c).as_str(), "pkg.foo");
        // Bare names without an import-map are untouched.
        assert_eq!(r.resolve_reference("deque", &c).as_str(), "deque");
    }

    #[test]
    fn python_import_map_last_insert_wins() {
        let mut imports = PythonImportMap::new();
        imports.insert("Foo", "pkg.a.Foo");
        imports.insert("Foo", "pkg.b.Foo");
        assert_eq!(imports.get("Foo"), Some("pkg.b.Foo"));
    }

    #[test]
    fn resolver_for_dispatches_by_language() {
        // Rust → RustResolver. We can't compare TypeId directly (the
        // boxed trait erases it), so we exercise the resolver via its
        // observable behaviour: a Rust-only future invariant (e.g.
        // canonical strings always start with "crate::" or a primitive
        // type name) is checked once each resolver fills in real logic.
        // For now just confirm dispatch returns SOMETHING.
        let _r = resolver_for(Language::Rust);
        let _t = resolver_for(Language::TypeScript);
        let _p = resolver_for(Language::Python);
        let _x = resolver_for(Language::Go); // → passthrough
    }

    #[test]
    fn canonical_symbol_round_trips() {
        let s = CanonicalSymbol::new("crate::manager::WorkerPool::spawn");
        assert_eq!(s.as_str(), "crate::manager::WorkerPool::spawn");
        assert_eq!(s.to_string(), "crate::manager::WorkerPool::spawn");
        assert_eq!(s.into_inner(), "crate::manager::WorkerPool::spawn");
    }
}
