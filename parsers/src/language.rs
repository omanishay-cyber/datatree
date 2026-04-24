//! Supported languages and the mapping from filesystem paths to grammars.
//!
//! The enum lists every language mneme v1.0 plans to support (§21.3.1).
//! Variants whose grammars are gated behind cargo features still appear in
//! the enum — but [`Language::tree_sitter_language`] returns
//! [`ParserError::LanguageNotEnabled`] when the matching feature is off.
//! This keeps the enum stable across feature combinations.

use crate::error::ParserError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Every language mneme can parse.
///
/// The variants are ordered by tier (1 = always built, 2 = community / opt-in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    // --- Tier 1 — always available -----------------------------------------
    TypeScript,
    Tsx,
    JavaScript,
    Jsx,
    Python,
    Rust,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Bash,
    Json,

    // --- Lightweight document grammars (default-on but feature-gated) -------
    Lua,
    Toml,
    Yaml,
    Markdown,

    // --- Tier 2 — community grammars (opt-in via features) -----------------
    Swift,
    Kotlin,
    Scala,
    Vue,
    Svelte,
    Solidity,
    Julia,
    Zig,
    Haskell,
}

impl Language {
    /// Returns every supported variant, in declaration order.
    pub const ALL: &'static [Language] = &[
        Language::TypeScript,
        Language::Tsx,
        Language::JavaScript,
        Language::Jsx,
        Language::Python,
        Language::Rust,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Ruby,
        Language::Php,
        Language::Bash,
        Language::Json,
        Language::Lua,
        Language::Toml,
        Language::Yaml,
        Language::Markdown,
        Language::Swift,
        Language::Kotlin,
        Language::Scala,
        Language::Vue,
        Language::Svelte,
        Language::Solidity,
        Language::Julia,
        Language::Zig,
        Language::Haskell,
    ];

    /// Stable string identifier — used in JSON, telemetry, and error messages.
    pub fn as_str(self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::Tsx => "tsx",
            Language::JavaScript => "javascript",
            Language::Jsx => "jsx",
            Language::Python => "python",
            Language::Rust => "rust",
            Language::Go => "go",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Bash => "bash",
            Language::Json => "json",
            Language::Lua => "lua",
            Language::Toml => "toml",
            Language::Yaml => "yaml",
            Language::Markdown => "markdown",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Scala => "scala",
            Language::Vue => "vue",
            Language::Svelte => "svelte",
            Language::Solidity => "solidity",
            Language::Julia => "julia",
            Language::Zig => "zig",
            Language::Haskell => "haskell",
        }
    }

    /// Map a file extension (with or without leading `.`) to a [`Language`].
    ///
    /// Returns `None` for unrecognised or binary extensions.
    pub fn from_extension(ext: &str) -> Option<Language> {
        let ext = ext.strip_prefix('.').unwrap_or(ext).to_ascii_lowercase();
        Some(match ext.as_str() {
            "ts" | "mts" | "cts" => Language::TypeScript,
            "tsx" => Language::Tsx,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "jsx" => Language::Jsx,
            "py" | "pyi" | "pyw" => Language::Python,
            "rs" => Language::Rust,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Language::Cpp,
            "cs" => Language::CSharp,
            "rb" | "rake" => Language::Ruby,
            "php" | "phtml" => Language::Php,
            "sh" | "bash" | "zsh" => Language::Bash,
            "json" | "jsonc" => Language::Json,
            "lua" | "luau" => Language::Lua,
            "toml" => Language::Toml,
            "yml" | "yaml" => Language::Yaml,
            "md" | "mdx" | "markdown" => Language::Markdown,
            "swift" => Language::Swift,
            "kt" | "kts" => Language::Kotlin,
            "scala" | "sc" | "sbt" => Language::Scala,
            "vue" => Language::Vue,
            "svelte" => Language::Svelte,
            "sol" => Language::Solidity,
            "jl" => Language::Julia,
            "zig" => Language::Zig,
            "hs" | "lhs" => Language::Haskell,
            _ => return None,
        })
    }

    /// Map a full path to a [`Language`] using extension + special filenames.
    pub fn from_filename(path: &Path) -> Option<Language> {
        // Special filenames that have no useful extension
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            match name.to_ascii_lowercase().as_str() {
                "dockerfile" | ".bashrc" | ".bash_profile" | ".zshrc" | "makefile" => {
                    // Heuristic: shell-ish things route through Bash for now;
                    // a richer mapping arrives with the scanner crate (§3.1).
                    return Some(Language::Bash);
                }
                "cargo.toml" | "pyproject.toml" => return Some(Language::Toml),
                "package.json" | "tsconfig.json" => return Some(Language::Json),
                _ => {}
            }
        }
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Language::from_extension)
    }

    /// Tier 1 = always built; Tier 2 = feature-gated.
    pub fn is_tier_one(self) -> bool {
        matches!(
            self,
            Language::TypeScript
                | Language::Tsx
                | Language::JavaScript
                | Language::Jsx
                | Language::Python
                | Language::Rust
                | Language::Go
                | Language::Java
                | Language::C
                | Language::Cpp
                | Language::CSharp
                | Language::Ruby
                | Language::Php
                | Language::Bash
                | Language::Json
        )
    }

    /// Returns the tree-sitter `Language` for the grammar, or
    /// [`ParserError::LanguageNotEnabled`] when the cargo feature is off.
    ///
    /// This is the only place that touches the grammar crates directly.
    pub fn tree_sitter_language(self) -> Result<tree_sitter::Language, ParserError> {
        Ok(match self {
            // ---- Tier 1 ------------------------------------------------------
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::Jsx => tree_sitter_javascript::LANGUAGE.into(), // JSX shares JS grammar
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::Java => tree_sitter_java::LANGUAGE.into(),
            Language::C => tree_sitter_c::LANGUAGE.into(),
            Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Language::Bash => tree_sitter_bash::LANGUAGE.into(),
            Language::Json => tree_sitter_json::LANGUAGE.into(),

            // ---- Default-on but feature-gated --------------------------------
            #[cfg(feature = "lua")]
            Language::Lua => tree_sitter_lua::LANGUAGE.into(),
            #[cfg(not(feature = "lua"))]
            Language::Lua => return Err(ParserError::LanguageNotEnabled("lua".into())),

            #[cfg(feature = "toml")]
            Language::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
            #[cfg(not(feature = "toml"))]
            Language::Toml => return Err(ParserError::LanguageNotEnabled("toml".into())),

            #[cfg(feature = "yaml")]
            Language::Yaml => tree_sitter_yaml::LANGUAGE.into(),
            #[cfg(not(feature = "yaml"))]
            Language::Yaml => return Err(ParserError::LanguageNotEnabled("yaml".into())),

            #[cfg(feature = "markdown")]
            Language::Markdown => tree_sitter_md::LANGUAGE.into(),
            #[cfg(not(feature = "markdown"))]
            Language::Markdown => return Err(ParserError::LanguageNotEnabled("markdown".into())),

            // ---- Tier 2 (opt-in only) ----------------------------------------
            #[cfg(feature = "swift")]
            Language::Swift => tree_sitter_swift::LANGUAGE.into(),
            #[cfg(not(feature = "swift"))]
            Language::Swift => return Err(ParserError::LanguageNotEnabled("swift".into())),

            #[cfg(feature = "kotlin")]
            Language::Kotlin => tree_sitter_kotlin_sg::LANGUAGE.into(),
            #[cfg(not(feature = "kotlin"))]
            Language::Kotlin => return Err(ParserError::LanguageNotEnabled("kotlin".into())),

            #[cfg(feature = "scala")]
            Language::Scala => tree_sitter_scala::LANGUAGE.into(),
            #[cfg(not(feature = "scala"))]
            Language::Scala => return Err(ParserError::LanguageNotEnabled("scala".into())),

            // Vue has no working crates.io grammar pinned to our runtime — the
            // only published crate (`tree-sitter-vue` 0.0.3) requires the
            // legacy 0.20 tree-sitter runtime. We keep the Language::Vue
            // variant for file-detection purposes (Vue SFCs are recognised)
            // but report the grammar as unavailable at runtime.
            Language::Vue => return Err(ParserError::LanguageNotEnabled("vue".into())),

            #[cfg(feature = "svelte")]
            Language::Svelte => tree_sitter_svelte_ng::LANGUAGE.into(),
            #[cfg(not(feature = "svelte"))]
            Language::Svelte => return Err(ParserError::LanguageNotEnabled("svelte".into())),

            #[cfg(feature = "solidity")]
            Language::Solidity => tree_sitter_solidity::LANGUAGE.into(),
            #[cfg(not(feature = "solidity"))]
            Language::Solidity => return Err(ParserError::LanguageNotEnabled("solidity".into())),

            #[cfg(feature = "julia")]
            Language::Julia => tree_sitter_julia::LANGUAGE.into(),
            #[cfg(not(feature = "julia"))]
            Language::Julia => return Err(ParserError::LanguageNotEnabled("julia".into())),

            #[cfg(feature = "zig")]
            Language::Zig => tree_sitter_zig::LANGUAGE.into(),
            #[cfg(not(feature = "zig"))]
            Language::Zig => return Err(ParserError::LanguageNotEnabled("zig".into())),

            #[cfg(feature = "haskell")]
            Language::Haskell => tree_sitter_haskell::LANGUAGE.into(),
            #[cfg(not(feature = "haskell"))]
            Language::Haskell => return Err(ParserError::LanguageNotEnabled("haskell".into())),
        })
    }

    /// Returns true when this language's grammar is compiled into this binary.
    ///
    /// Useful when the supervisor advertises capabilities to the MCP server.
    pub fn is_enabled(self) -> bool {
        self.tree_sitter_language().is_ok()
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
