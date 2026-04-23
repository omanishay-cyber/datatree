//! Compiled `tree_sitter::Query` cache (§21.3, §25.10).
//!
//! Every supported language exposes the same set of [`QueryKind`] patterns
//! (functions, classes, calls, imports, decorators, comments, errors).
//! The compiled queries live in a `OnceCell` per (language, kind) so the
//! first lookup pays the compile cost and every subsequent lookup is O(1).

use crate::error::ParserError;
use crate::language::Language;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tree_sitter::Query;

/// One of the seven canonical query patterns mneme tracks per language.
///
/// Adding a new variant requires updating [`pattern_for`] for every supported
/// language (or returning `""` to opt out).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryKind {
    Functions,
    Classes,
    Calls,
    Imports,
    Decorators,
    Comments,
    Errors,
}

impl QueryKind {
    /// Stable string id — used in tracing and capability advertisement.
    pub fn as_str(self) -> &'static str {
        match self {
            QueryKind::Functions => "functions",
            QueryKind::Classes => "classes",
            QueryKind::Calls => "calls",
            QueryKind::Imports => "imports",
            QueryKind::Decorators => "decorators",
            QueryKind::Comments => "comments",
            QueryKind::Errors => "errors",
        }
    }

    /// Iteration order used by [`crate::extractor::Extractor`].
    pub const ALL: &'static [QueryKind] = &[
        QueryKind::Functions,
        QueryKind::Classes,
        QueryKind::Calls,
        QueryKind::Imports,
        QueryKind::Decorators,
        QueryKind::Comments,
        QueryKind::Errors,
    ];
}

// ---------------------------------------------------------------------------
// Per-(lang, kind) compiled-query cache
// ---------------------------------------------------------------------------

type CacheKey = (Language, QueryKind);

static CACHE: Lazy<DashMap<CacheKey, Arc<Query>>> = Lazy::new(DashMap::new);

/// Get (or compile + cache) the [`Query`] for a `(language, kind)`.
///
/// The first call per key compiles the pattern; later calls return a cheap
/// `Arc::clone`. Returns [`ParserError::QueryCompile`] when the source
/// pattern is malformed for the grammar's node names.
pub fn get_query(lang: Language, kind: QueryKind) -> Result<Arc<Query>, ParserError> {
    if let Some(q) = CACHE.get(&(lang, kind)) {
        return Ok(q.clone());
    }
    let pattern = pattern_for(lang, kind);
    if pattern.is_empty() {
        // No-op pattern — return an empty query so callers don't branch on it.
        let ts_lang = lang.tree_sitter_language()?;
        let q = Query::new(&ts_lang, "").map_err(|e| ParserError::QueryCompile {
            language: lang.as_str().to_string(),
            kind: kind.as_str(),
            source: e,
        })?;
        let arc = Arc::new(q);
        CACHE.insert((lang, kind), arc.clone());
        return Ok(arc);
    }

    let ts_lang = lang.tree_sitter_language()?;
    let query = Query::new(&ts_lang, pattern).map_err(|e| ParserError::QueryCompile {
        language: lang.as_str().to_string(),
        kind: kind.as_str(),
        source: e,
    })?;
    let arc = Arc::new(query);
    CACHE.insert((lang, kind), arc.clone());
    Ok(arc)
}

/// Pre-compile every pattern for every enabled language.
///
/// Called once at worker startup so the first parse doesn't pay the
/// compile cost. Errors are propagated immediately — a bad pattern is a
/// programmer bug, not a runtime input bug.
pub fn warm_up() -> Result<(), ParserError> {
    for lang in Language::ALL {
        if !lang.is_enabled() {
            continue;
        }
        for kind in QueryKind::ALL {
            // Best-effort: skip any (lang, kind) pair the grammar can't satisfy.
            // We log via tracing; the supervisor's drift detector picks it up.
            if let Err(e) = get_query(*lang, *kind) {
                tracing::warn!(language = %lang, kind = ?kind, error = %e, "skipping query");
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Patterns — per-language source strings
//
// Patterns are intentionally MINIMAL — they capture the named scopes the
// extractor cares about (functions, classes, calls, imports, decorators,
// comments) and nothing more. Richer patterns belong to the scanner crate.
// ---------------------------------------------------------------------------

/// Per-(lang, kind) S-expression pattern. Empty string = "not implemented".
fn pattern_for(lang: Language, kind: QueryKind) -> &'static str {
    match (lang, kind) {
        // ---------------- TypeScript / TSX ----------------------------------
        (Language::TypeScript | Language::Tsx, QueryKind::Functions) => {
            r#"
            (function_declaration name: (identifier) @name) @function
            (method_definition name: (property_identifier) @name) @function
            (function_expression name: (identifier)? @name) @function
            (arrow_function) @function
            "#
        }
        (Language::TypeScript | Language::Tsx, QueryKind::Classes) => {
            r#"
            (class_declaration name: (type_identifier) @name) @class
            (interface_declaration name: (type_identifier) @name) @class
            "#
        }
        (Language::TypeScript | Language::Tsx, QueryKind::Calls) => {
            r#"(call_expression function: (_) @callee) @call"#
        }
        (Language::TypeScript | Language::Tsx, QueryKind::Imports) => {
            r#"(import_statement source: (string) @source) @import"#
        }
        (Language::TypeScript | Language::Tsx, QueryKind::Decorators) => {
            r#"(decorator (_) @decorator_value) @decorator"#
        }
        (Language::TypeScript | Language::Tsx, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- JavaScript / JSX ----------------------------------
        (Language::JavaScript | Language::Jsx, QueryKind::Functions) => {
            r#"
            (function_declaration name: (identifier) @name) @function
            (method_definition name: (property_identifier) @name) @function
            (function_expression name: (identifier)? @name) @function
            (arrow_function) @function
            "#
        }
        (Language::JavaScript | Language::Jsx, QueryKind::Classes) => {
            r#"(class_declaration name: (identifier) @name) @class"#
        }
        (Language::JavaScript | Language::Jsx, QueryKind::Calls) => {
            r#"(call_expression function: (_) @callee) @call"#
        }
        (Language::JavaScript | Language::Jsx, QueryKind::Imports) => {
            r#"(import_statement source: (string) @source) @import"#
        }
        (Language::JavaScript | Language::Jsx, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Python --------------------------------------------
        (Language::Python, QueryKind::Functions) => {
            r#"
            (function_definition name: (identifier) @name) @function
            "#
        }
        (Language::Python, QueryKind::Classes) => {
            r#"(class_definition name: (identifier) @name) @class"#
        }
        (Language::Python, QueryKind::Calls) => r#"(call function: (_) @callee) @call"#,
        (Language::Python, QueryKind::Imports) => {
            r#"
            (import_statement) @import
            (import_from_statement) @import
            "#
        }
        (Language::Python, QueryKind::Decorators) => {
            r#"(decorator (_) @decorator_value) @decorator"#
        }
        (Language::Python, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Rust ----------------------------------------------
        (Language::Rust, QueryKind::Functions) => {
            r#"
            (function_item name: (identifier) @name) @function
            (function_signature_item name: (identifier) @name) @function
            "#
        }
        (Language::Rust, QueryKind::Classes) => {
            r#"
            (struct_item name: (type_identifier) @name) @class
            (enum_item name: (type_identifier) @name) @class
            (trait_item name: (type_identifier) @name) @class
            (impl_item) @class
            "#
        }
        (Language::Rust, QueryKind::Calls) => {
            r#"
            (call_expression function: (_) @callee) @call
            (macro_invocation macro: (_) @callee) @call
            "#
        }
        (Language::Rust, QueryKind::Imports) => r#"(use_declaration) @import"#,
        (Language::Rust, QueryKind::Decorators) => {
            r#"(attribute_item) @decorator"#
        }
        (Language::Rust, QueryKind::Comments) => {
            r#"
            (line_comment) @comment
            (block_comment) @comment
            "#
        }

        // ---------------- Go ------------------------------------------------
        (Language::Go, QueryKind::Functions) => {
            r#"
            (function_declaration name: (identifier) @name) @function
            (method_declaration name: (field_identifier) @name) @function
            "#
        }
        (Language::Go, QueryKind::Classes) => {
            r#"(type_declaration (type_spec name: (type_identifier) @name)) @class"#
        }
        (Language::Go, QueryKind::Calls) => {
            r#"(call_expression function: (_) @callee) @call"#
        }
        (Language::Go, QueryKind::Imports) => r#"(import_declaration) @import"#,
        (Language::Go, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Java ----------------------------------------------
        (Language::Java, QueryKind::Functions) => {
            r#"(method_declaration name: (identifier) @name) @function"#
        }
        (Language::Java, QueryKind::Classes) => {
            r#"
            (class_declaration name: (identifier) @name) @class
            (interface_declaration name: (identifier) @name) @class
            "#
        }
        (Language::Java, QueryKind::Calls) => {
            r#"(method_invocation name: (identifier) @callee) @call"#
        }
        (Language::Java, QueryKind::Imports) => r#"(import_declaration) @import"#,
        (Language::Java, QueryKind::Decorators) => {
            r#"(annotation name: (identifier) @decorator_value) @decorator"#
        }
        (Language::Java, QueryKind::Comments) => {
            r#"
            (line_comment) @comment
            (block_comment) @comment
            "#
        }

        // ---------------- C / C++ -------------------------------------------
        (Language::C | Language::Cpp, QueryKind::Functions) => {
            r#"(function_definition declarator: (_) @name) @function"#
        }
        (Language::Cpp, QueryKind::Classes) => {
            r#"
            (class_specifier name: (type_identifier) @name) @class
            (struct_specifier name: (type_identifier) @name) @class
            "#
        }
        (Language::C, QueryKind::Classes) => {
            r#"(struct_specifier name: (type_identifier) @name) @class"#
        }
        (Language::C | Language::Cpp, QueryKind::Calls) => {
            r#"(call_expression function: (_) @callee) @call"#
        }
        (Language::C | Language::Cpp, QueryKind::Imports) => r#"(preproc_include) @import"#,
        (Language::C | Language::Cpp, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- C# ------------------------------------------------
        (Language::CSharp, QueryKind::Functions) => {
            r#"(method_declaration name: (identifier) @name) @function"#
        }
        (Language::CSharp, QueryKind::Classes) => {
            r#"
            (class_declaration name: (identifier) @name) @class
            (interface_declaration name: (identifier) @name) @class
            "#
        }
        (Language::CSharp, QueryKind::Calls) => {
            r#"(invocation_expression function: (_) @callee) @call"#
        }
        (Language::CSharp, QueryKind::Imports) => r#"(using_directive) @import"#,
        (Language::CSharp, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Ruby ----------------------------------------------
        (Language::Ruby, QueryKind::Functions) => {
            r#"(method name: (identifier) @name) @function"#
        }
        (Language::Ruby, QueryKind::Classes) => {
            r#"
            (class name: (constant) @name) @class
            (module name: (constant) @name) @class
            "#
        }
        (Language::Ruby, QueryKind::Calls) => r#"(call) @call"#,
        (Language::Ruby, QueryKind::Imports) => r#"(call) @import"#,
        (Language::Ruby, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- PHP -----------------------------------------------
        (Language::Php, QueryKind::Functions) => {
            r#"
            (function_definition name: (name) @name) @function
            (method_declaration name: (name) @name) @function
            "#
        }
        (Language::Php, QueryKind::Classes) => {
            r#"
            (class_declaration name: (name) @name) @class
            (interface_declaration name: (name) @name) @class
            "#
        }
        (Language::Php, QueryKind::Calls) => {
            r#"(function_call_expression function: (_) @callee) @call"#
        }
        (Language::Php, QueryKind::Imports) => r#"(namespace_use_declaration) @import"#,
        (Language::Php, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Bash ----------------------------------------------
        (Language::Bash, QueryKind::Functions) => {
            r#"(function_definition name: (word) @name) @function"#
        }
        (Language::Bash, QueryKind::Calls) => {
            r#"(command name: (command_name) @callee) @call"#
        }
        (Language::Bash, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Documentary grammars ------------------------------
        (Language::Json, QueryKind::Comments) => "",
        #[cfg(feature = "toml")]
        (Language::Toml, QueryKind::Comments) => r#"(comment) @comment"#,
        #[cfg(feature = "yaml")]
        (Language::Yaml, QueryKind::Comments) => r#"(comment) @comment"#,
        #[cfg(feature = "markdown")]
        (Language::Markdown, QueryKind::Functions) => "",
        #[cfg(feature = "lua")]
        (Language::Lua, QueryKind::Functions) => {
            r#"(function_declaration name: (_) @name) @function"#
        }
        #[cfg(feature = "lua")]
        (Language::Lua, QueryKind::Calls) => {
            r#"(function_call) @call"#
        }
        #[cfg(feature = "lua")]
        (Language::Lua, QueryKind::Comments) => r#"(comment) @comment"#,

        // ---------------- Tier 2 community grammars -------------------------
        // Patterns deliberately conservative so the extractor degrades
        // gracefully on grammar version drift. Richer queries land per
        // language as the scanner crate matures.
        #[cfg(feature = "swift")]
        (Language::Swift, QueryKind::Functions) => {
            r#"(function_declaration name: (simple_identifier) @name) @function"#
        }
        #[cfg(feature = "kotlin")]
        (Language::Kotlin, QueryKind::Functions) => {
            r#"(function_declaration (simple_identifier) @name) @function"#
        }
        #[cfg(feature = "scala")]
        (Language::Scala, QueryKind::Functions) => {
            r#"(function_definition name: (_) @name) @function"#
        }
        #[cfg(feature = "vue")]
        (Language::Vue, QueryKind::Comments) => r#"(comment) @comment"#,
        #[cfg(feature = "svelte")]
        (Language::Svelte, QueryKind::Comments) => r#"(comment) @comment"#,
        #[cfg(feature = "solidity")]
        (Language::Solidity, QueryKind::Functions) => {
            r#"(function_definition name: (_) @name) @function"#
        }
        #[cfg(feature = "julia")]
        (Language::Julia, QueryKind::Functions) => {
            r#"(function_definition name: (_) @name) @function"#
        }
        #[cfg(feature = "zig")]
        (Language::Zig, QueryKind::Functions) => {
            r#"(function_declaration name: (_) @name) @function"#
        }
        #[cfg(feature = "haskell")]
        (Language::Haskell, QueryKind::Functions) => {
            r#"(function) @function"#
        }

        // ----- ERROR/MISSING — same query across every grammar ---------------
        // tree-sitter exposes ERROR as a built-in node name regardless of
        // grammar; MISSING is queried via the predicate `(MISSING)`.
        // Tree-sitter 0.23 accepts `(ERROR)` as a built-in node query but
        // rejects `(MISSING)` (that's a query-predicate concept, not a
        // node). ERROR alone is enough for our confidence-downgrade logic.
        (_, QueryKind::Errors) => r#"(ERROR) @error"#,

        // Anything not explicitly listed above gets an empty pattern, which
        // [`get_query`] turns into a harmless no-op Query.
        _ => "",
    }
}
