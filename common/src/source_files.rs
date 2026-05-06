//! Canonical "is this a source file?" predicate.
//!
//! MAINT-8 fix (2026-05-06 audit): two `is_source_file` helpers
//! existed in the CLI crate with DIFFERENT extension lists:
//!
//!   cli/src/commands/federated.rs           — 24 exts (incl. mjs/cjs/scala/sh/bash/zsh/ps1)
//!   cli/src/commands/pretool_grep_read.rs   — 18 exts (no shell scripts, no scala, no Apple variants)
//!
//! Two predicates with the same name + drifting truth tables is the
//! shape of a real bug waiting to happen — a contributor adds an
//! extension to one, ships, and the other one silently disagrees on
//! the same call. Lift the canonical truth table here so both
//! callers stay in sync, and provide str + Path overloads so each
//! site uses the form it already has.
//!
//! The canonical list is the UNION of the two prior lists (24 exts).
//! The pretool_grep_read site was the one missing entries; expanding
//! its coverage means soft-redirect hints fire for shell scripts +
//! Scala too (correct, since the resolver is being wired for those).

use std::path::Path;

/// Source-file extensions Mneme treats as "human-written code"
/// (excludes data, docs, configs, generated artifacts). Lowercased,
/// no leading dot. Sorted by language family for diff readability.
pub const SOURCE_FILE_EXTENSIONS: &[&str] = &[
    // Rust
    "rs",
    // TypeScript / JavaScript family (incl. ESM/CJS variants)
    "ts", "tsx", "js", "jsx", "mjs", "cjs",
    // Python
    "py",
    // Go
    "go",
    // JVM
    "java", "kt", "scala",
    // Apple
    "swift",
    // C / C++
    "c", "cc", "cpp", "h", "hpp",
    // Other dynamic
    "rb", "php",
    // .NET
    "cs",
    // Shells
    "sh", "bash", "zsh", "ps1",
];

/// True if `path`'s extension matches a known source-code file type.
///
/// Path-form: lowercases the extension via `to_ascii_lowercase` so
/// `.RS` / `.Rs` / `.rs` all match equally. Returns `false` for any
/// path with no extension or a non-UTF-8 extension.
pub fn is_source_file_path(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let lower = ext.to_ascii_lowercase();
    SOURCE_FILE_EXTENSIONS.iter().any(|known| *known == lower)
}

/// True if `path` (string form) ends with a known source-code
/// extension. Forward-slash and back-slash both supported. Empty /
/// whitespace strings return `false`.
pub fn is_source_file_str(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Find the LAST dot after the LAST path separator.
    let last_sep = trimmed
        .rfind(|c: char| c == '/' || c == '\\')
        .map(|i| i + 1)
        .unwrap_or(0);
    let basename = &trimmed[last_sep..];
    let Some(dot_pos) = basename.rfind('.') else {
        return false;
    };
    let ext = &basename[dot_pos + 1..];
    if ext.is_empty() {
        return false;
    }
    let lower = ext.to_ascii_lowercase();
    SOURCE_FILE_EXTENSIONS.iter().any(|known| *known == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rs_path_is_source() {
        assert!(is_source_file_path(Path::new("src/lib.rs")));
        assert!(is_source_file_path(Path::new("src/lib.RS")));
    }

    #[test]
    fn shell_scripts_count() {
        assert!(is_source_file_str("install.sh"));
        assert!(is_source_file_str("script.bash"));
        assert!(is_source_file_str("setup.ps1"));
        assert!(is_source_file_str("config.zsh"));
    }

    #[test]
    fn scala_counts() {
        assert!(is_source_file_path(Path::new("Main.scala")));
    }

    #[test]
    fn data_files_skip() {
        assert!(!is_source_file_path(Path::new("README.md")));
        assert!(!is_source_file_path(Path::new("config.json")));
        assert!(!is_source_file_path(Path::new("photo.png")));
        assert!(!is_source_file_str("README.md"));
        assert!(!is_source_file_str("config.toml"));
    }

    #[test]
    fn no_extension_skips() {
        assert!(!is_source_file_path(Path::new("Makefile")));
        assert!(!is_source_file_str("Dockerfile"));
        assert!(!is_source_file_str(""));
        assert!(!is_source_file_str("   "));
    }

    #[test]
    fn windows_paths_work_in_str_form() {
        assert!(is_source_file_str("C:\\src\\lib.rs"));
        assert!(is_source_file_str("vision\\src\\App.tsx"));
        assert!(!is_source_file_str("C:\\photos\\img.png"));
    }

    #[test]
    fn extension_case_insensitive() {
        assert!(is_source_file_str("App.TSX"));
        assert!(is_source_file_str("Main.JAVA"));
    }

    #[test]
    fn dotfiles_without_extension_skip() {
        assert!(!is_source_file_str(".gitignore"));
        assert!(!is_source_file_path(Path::new(".env")));
    }
}
