//! `RefactorScanner` — suggests safe, local refactors.
//!
//! Detects, with line-oriented heuristics:
//!
//! * **Unused imports** — a TS/JS `import { A, B } from "x"` statement where
//!   one of the named specifiers is never referenced again in the same file.
//! * **Unreferenced top-level declarations** — `function foo()`, `const foo =`,
//!   `class Foo`, `type Foo` whose identifier appears only at the definition
//!   site. These are candidates for the project-wide dead-code pass; the
//!   scanner flags them as suggestions, never deletes them.
//! * **Rename candidates** — names whose casing style violates the project
//!   convention. Functions & variables: camelCase; types & classes:
//!   PascalCase; constants: UPPER_SNAKE. Only fires on top-level
//!   declarations (export or module-level) to keep signal high.
//!
//! The scanner does NOT perform cross-file reachability itself (that belongs
//! in the graph reachability pass run by the brain/supervisor). It produces
//! `Finding`s with a `suggestion` field so the refactor-apply MCP tool can
//! turn an approved proposal into an atomic file write.
//!
//! Stateless. Pure. Safe to run in a tokio worker pool.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::scanner::{line_col_of, Ast, Finding, Scanner, Severity};

/// `import { A, B as C } from "x"` — we pull out the brace body.
static IMPORT_NAMED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*import\s*\{\s*([^}]+)\s*\}\s*from\s*['"][^'"]+['"]\s*;?\s*$"#)
        .expect("import named regex")
});

/// `import Default from "x"` — captures default ident.
static IMPORT_DEFAULT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*import\s+([A-Za-z_$][A-Za-z0-9_$]*)\s+from\s*['"][^'"]+['"]\s*;?\s*$"#)
        .expect("import default regex")
});

/// `function foo(` / `async function foo(` at top-level (no leading whitespace
/// beyond `export`). Captures the ident.
static FN_DECL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\("#)
        .expect("fn decl regex")
});

/// `const foo = ` / `let foo = ` at top-level.
static VAR_DECL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^(?:export\s+)?(?:const|let)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*[:=]"#)
        .expect("var decl regex")
});

/// `class Foo` / `interface Foo` / `type Foo =`.
static TYPE_DECL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^(?:export\s+)?(?:class|interface|type)\s+([A-Za-z_$][A-Za-z0-9_$]*)"#)
        .expect("type decl regex")
});

/// Detect identifier-like occurrences to count references.
fn count_occurrences(content: &str, ident: &str) -> usize {
    if ident.is_empty() {
        return 0;
    }
    let escaped = regex::escape(ident);
    let pat = format!(r"\b{}\b", escaped);
    match Regex::new(&pat) {
        Ok(re) => re.find_iter(content).count(),
        Err(_) => 0,
    }
}

/// Is this identifier a valid camelCase (first char lowercase, no underscores
/// other than a leading `_`, no leading uppercase)? Accepts single-letter.
fn is_camel_case(id: &str) -> bool {
    let trimmed = id.trim_start_matches('_');
    if trimmed.is_empty() {
        return true;
    }
    let first = trimmed.chars().next().expect("non-empty");
    if !first.is_ascii_lowercase() {
        return false;
    }
    !trimmed.contains('_')
}

/// Is this identifier PascalCase (leading uppercase, no underscores)?
fn is_pascal_case(id: &str) -> bool {
    if id.is_empty() {
        return true;
    }
    let first = id.chars().next().expect("non-empty");
    if !first.is_ascii_uppercase() {
        return false;
    }
    !id.contains('_')
}

/// Is this identifier UPPER_SNAKE_CASE?
fn is_upper_snake(id: &str) -> bool {
    if id.is_empty() {
        return true;
    }
    id.chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Convert a snake_case or PascalCase or kebab ident to camelCase best-effort.
fn to_camel_case(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut upper_next = false;
    let mut first = true;
    for c in id.chars() {
        if c == '_' || c == '-' {
            upper_next = true;
            continue;
        }
        if first {
            out.push(c.to_ascii_lowercase());
            first = false;
        } else if upper_next {
            out.push(c.to_ascii_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Convert any ident to PascalCase best-effort.
fn to_pascal_case(id: &str) -> String {
    let camel = to_camel_case(id);
    let mut chars = camel.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

const REFACTOR_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// Refactor suggestion scanner.
pub struct RefactorScanner;

impl Default for RefactorScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl RefactorScanner {
    /// New scanner. Stateless.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Scanner for RefactorScanner {
    fn name(&self) -> &str {
        "refactor"
    }

    fn applies_to(&self, file: &Path) -> bool {
        file.extension()
            .and_then(|e| e.to_str())
            .map(|e| REFACTOR_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    fn scan(&self, file: &Path, content: &str, _ast: Option<Ast<'_>>) -> Vec<Finding> {
        let file_str = file.to_string_lossy().to_string();
        let mut out = Vec::new();

        // --- 1. Unused named imports --------------------------------------
        for caps in IMPORT_NAMED.captures_iter(content) {
            let full = caps.get(0).expect("full match");
            let body = caps.get(1).expect("named body").as_str();
            let stmt_start = full.start();
            let after_stmt = &content[full.end()..];
            for raw in body.split(',') {
                let piece = raw.trim();
                if piece.is_empty() {
                    continue;
                }
                // `A as B` → usage ident is `B`, detection ident is `A`.
                let local_ident = match piece.split(" as ").map(str::trim).nth(1) {
                    Some(rename) => rename,
                    None => piece,
                };
                if !local_ident
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                {
                    continue;
                }
                let refs_after = count_occurrences(after_stmt, local_ident);
                if refs_after == 0 {
                    let (line, col) = line_col_of(content, stmt_start);
                    out.push(
                        Finding::new_line(
                            "refactor.unused-import",
                            Severity::Info,
                            &file_str,
                            line,
                            col,
                            col + (full.end() - full.start()) as u32,
                            format!("Unused named import `{}` — safe to remove.", local_ident),
                        )
                        .with_fix(""),
                    );
                }
            }
        }

        // --- 2. Unused default imports ------------------------------------
        for caps in IMPORT_DEFAULT.captures_iter(content) {
            let full = caps.get(0).expect("full match");
            let ident = caps.get(1).expect("ident").as_str();
            let after_stmt = &content[full.end()..];
            if count_occurrences(after_stmt, ident) == 0 {
                let (line, col) = line_col_of(content, full.start());
                out.push(
                    Finding::new_line(
                        "refactor.unused-import",
                        Severity::Info,
                        &file_str,
                        line,
                        col,
                        col + (full.end() - full.start()) as u32,
                        format!("Unused default import `{}` — safe to remove.", ident),
                    )
                    .with_fix(""),
                );
            }
        }

        // --- 3. Unreferenced top-level declarations -----------------------
        for caps in FN_DECL.captures_iter(content) {
            let ident_m = caps.get(1).expect("ident");
            let ident = ident_m.as_str();
            // References in the whole file, minus the definition site.
            let total = count_occurrences(content, ident);
            if total <= 1 {
                let (line, col) = line_col_of(content, ident_m.start());
                out.push(Finding::new_line(
                    "refactor.unreachable-function",
                    Severity::Info,
                    &file_str,
                    line,
                    col,
                    col + ident.len() as u32,
                    format!(
                        "Function `{}` has no in-file references — candidate for dead-code removal.",
                        ident
                    ),
                ));
            }
            if !is_camel_case(ident) {
                let (line, col) = line_col_of(content, ident_m.start());
                let suggestion = to_camel_case(ident);
                out.push(
                    Finding::new_line(
                        "refactor.rename-function",
                        Severity::Info,
                        &file_str,
                        line,
                        col,
                        col + ident.len() as u32,
                        format!(
                            "Function `{}` violates camelCase convention — suggest `{}`.",
                            ident, suggestion
                        ),
                    )
                    .with_fix(suggestion),
                );
            }
        }

        for caps in VAR_DECL.captures_iter(content) {
            let ident_m = caps.get(1).expect("ident");
            let ident = ident_m.as_str();
            // Constant-like: UPPER_SNAKE is acceptable for top-level const.
            if !is_camel_case(ident) && !is_upper_snake(ident) {
                let (line, col) = line_col_of(content, ident_m.start());
                let suggestion = to_camel_case(ident);
                out.push(
                    Finding::new_line(
                        "refactor.rename-variable",
                        Severity::Info,
                        &file_str,
                        line,
                        col,
                        col + ident.len() as u32,
                        format!(
                            "Top-level binding `{}` violates camelCase convention — suggest `{}`.",
                            ident, suggestion
                        ),
                    )
                    .with_fix(suggestion),
                );
            }
        }

        for caps in TYPE_DECL.captures_iter(content) {
            let ident_m = caps.get(1).expect("ident");
            let ident = ident_m.as_str();
            if !is_pascal_case(ident) {
                let (line, col) = line_col_of(content, ident_m.start());
                let suggestion = to_pascal_case(ident);
                out.push(
                    Finding::new_line(
                        "refactor.rename-type",
                        Severity::Info,
                        &file_str,
                        line,
                        col,
                        col + ident.len() as u32,
                        format!(
                            "Type `{}` violates PascalCase convention — suggest `{}`.",
                            ident, suggestion
                        ),
                    )
                    .with_fix(suggestion),
                );
            }
            let total = count_occurrences(content, ident);
            if total <= 1 {
                let (line, col) = line_col_of(content, ident_m.start());
                out.push(Finding::new_line(
                    "refactor.unreferenced-type",
                    Severity::Info,
                    &file_str,
                    line,
                    col,
                    col + ident.len() as u32,
                    format!(
                        "Type `{}` has no in-file references — candidate for dead-code removal.",
                        ident
                    ),
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_unused_named_import() {
        let s = RefactorScanner::new();
        let src = "import { Used, Unused } from \"x\";\nconsole.log(Used);\n";
        let f = PathBuf::from("a.ts");
        let findings = s.scan(&f, src, None);
        assert!(
            findings
                .iter()
                .any(|f| f.rule_id == "refactor.unused-import" && f.message.contains("Unused")),
            "findings = {:?}",
            findings
        );
    }

    #[test]
    fn detects_unreachable_function() {
        let s = RefactorScanner::new();
        let src = "function orphan() { return 1; }\n";
        let f = PathBuf::from("a.ts");
        let findings = s.scan(&f, src, None);
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "refactor.unreachable-function"));
    }

    #[test]
    fn detects_rename_candidate_function() {
        let s = RefactorScanner::new();
        let src = "export function Bad_Name() { Bad_Name(); }\n";
        let f = PathBuf::from("a.ts");
        let findings = s.scan(&f, src, None);
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "refactor.rename-function"));
    }
}
