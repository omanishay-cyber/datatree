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

/// Per-language resolver placeholder for Rust (v0.4.1 target).
///
/// The shape this WILL take:
/// 1. Walk up from the source file to the nearest `Cargo.toml`
///    to find the crate root.
/// 2. Build the file's own canonical prefix from the path
///    (`src/foo/bar.rs` → `crate::foo::bar`).
/// 3. For each `use` statement parsed by the extractor, build a
///    `prefix → fully_qualified` rewrite map.
/// 4. For each reference to a symbol, look it up in the rewrite
///    map (try the longest prefix first).
/// 5. Resolve `pub use` re-exports by following the chain.
///
/// Currently inherits the passthrough behaviour from the trait
/// default impls.
#[derive(Debug, Default)]
pub struct RustResolver;

impl SymbolResolver for RustResolver {}

/// Per-language resolver placeholder for TypeScript / JavaScript
/// (v0.4.2 target).
///
/// The shape this WILL take:
/// 1. Find and parse the nearest `tsconfig.json` for `paths` aliases.
/// 2. Track barrel-file re-exports (`export * from './x'`,
///    `export { X } from './y'`).
/// 3. Resolve module specifiers through the configured path-alias
///    map + Node resolution algorithm + ESM `package.json` `exports`.
/// 4. Handle declaration merging (multiple `interface Foo` blocks
///    in different files contributing to the same canonical type).
#[derive(Debug, Default)]
pub struct TypeScriptResolver;

impl SymbolResolver for TypeScriptResolver {}

/// Per-language resolver placeholder for Python (v0.4.3 target).
///
/// The shape this WILL take:
/// 1. jedi-style scoping: walk module-level / class-level / function-level
///    namespaces to figure out which `foo` a reference resolves to.
/// 2. Relative imports: `from .x import y` resolves through the
///    package's `__init__.py` chain.
/// 3. Namespace packages (PEP 420) where `__init__.py` is absent.
/// 4. `__all__` exports as the canonical surface for downstream
///    `from x import *` resolution.
#[derive(Debug, Default)]
pub struct PythonResolver;

impl SymbolResolver for PythonResolver {}

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
    fn rust_resolver_v040_is_still_passthrough() {
        let r = RustResolver;
        let c = ctx(Language::Rust);
        assert_eq!(
            r.resolve_definition("WorkerPool", &c).as_str(),
            "WorkerPool",
            "v0.4.0 ships the skeleton — real resolution lands v0.4.1"
        );
    }

    #[test]
    fn typescript_resolver_v040_is_still_passthrough() {
        let r = TypeScriptResolver;
        let c = ctx(Language::TypeScript);
        assert_eq!(
            r.resolve_reference("@/components/Foo", &c).as_str(),
            "@/components/Foo"
        );
    }

    #[test]
    fn python_resolver_v040_is_still_passthrough() {
        let r = PythonResolver;
        let c = ctx(Language::Python);
        assert_eq!(
            r.resolve_reference("..pkg.mod.foo", &c).as_str(),
            "..pkg.mod.foo"
        );
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
