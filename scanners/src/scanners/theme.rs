//! [`ThemeScanner`] — flags hardcoded color literals that should reference
//! design tokens, and Tailwind classes lacking a `dark:` variant.
//!
//! Patterns flagged:
//! - `#[0-9a-fA-F]{3,8}` hex literals
//! - `rgb(...)` / `rgba(...)` / `hsl(...)` / `hsla(...)` color functions
//! - Tailwind color utilities like `bg-white`, `text-gray-900`, `border-blue-500`
//!   that have NO sibling `dark:` variant on the same element
//!
//! Allowlisted brand-gradient values (per project CLAUDE.md):
//! `#4191E1`, `#41E1B5`, `#22D3EE` are NEVER flagged.

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

use crate::scanner::{line_col_of, Ast, Finding, Scanner, Severity};

/// Hex literal: `#` + 3, 4, 6, or 8 hex digits.
static HEX_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b")
        .expect("hex regex")
});

/// CSS color functions.
static CSS_FN_COLOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:rgb|rgba|hsl|hsla)\s*\([^)]+\)").expect("css fn color regex")
});

/// Tailwind utility class with a color shade. Captures the class so we
/// can check for a sibling `dark:` variant on the same `class="..."`.
static TW_COLOR_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\b(?:bg|text|border|ring|from|to|via|fill|stroke|placeholder|divide|outline|decoration|caret|accent|shadow)-(?:white|black|gray|slate|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)(?:-\d{2,3})?",
    )
    .expect("tailwind color class regex")
});

/// Brand gradient values explicitly allowed by project policy.
const BRAND_ALLOWLIST: &[&str] = &["#4191E1", "#41E1B5", "#22D3EE"];

/// Default extra allowlist (transparent / inherit-style values that show
/// up as 3-digit hex or short hex but aren't really colors of concern).
const DEFAULT_TOKEN_ALLOWLIST: &[&str] = &["#000", "#FFF", "#fff", "#000000", "#FFFFFF"];

/// File extensions the theme scanner runs against.
const THEME_FILE_EXTS: &[&str] = &["tsx", "jsx", "ts", "js", "css", "scss", "html", "vue", "svelte"];

/// Flags hardcoded colors and missing dark variants.
pub struct ThemeScanner {
    /// Optional path to the project's tokens file. When provided we extend
    /// the allowlist by reading hex values from it on construction.
    extra_allowlist: Vec<String>,
}

impl ThemeScanner {
    /// Build a new scanner. If `tokens_path` is `Some`, the file is read
    /// at construction time and every hex value found is added to the
    /// allowlist. Read errors are non-fatal — the scanner falls back to
    /// the built-in allowlist only.
    #[must_use]
    pub fn new(tokens_path: Option<String>) -> Self {
        let mut allow = Vec::new();
        if let Some(path) = tokens_path {
            if let Ok(s) = std::fs::read_to_string(&path) {
                for cap in HEX_COLOR.captures_iter(&s) {
                    if let Some(m) = cap.get(0) {
                        allow.push(m.as_str().to_string());
                    }
                }
            }
        }
        Self {
            extra_allowlist: allow,
        }
    }

    fn is_allowlisted(&self, hex: &str) -> bool {
        BRAND_ALLOWLIST.iter().any(|a| a.eq_ignore_ascii_case(hex))
            || DEFAULT_TOKEN_ALLOWLIST
                .iter()
                .any(|a| a.eq_ignore_ascii_case(hex))
            || self
                .extra_allowlist
                .iter()
                .any(|a| a.eq_ignore_ascii_case(hex))
    }
}

impl Scanner for ThemeScanner {
    fn name(&self) -> &str {
        "theme"
    }

    fn applies_to(&self, file: &Path) -> bool {
        file.extension()
            .and_then(|e| e.to_str())
            .map(|e| THEME_FILE_EXTS.iter().any(|ext| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    fn scan(&self, file: &Path, content: &str, _ast: Option<Ast<'_>>) -> Vec<Finding> {
        let file_str = file.to_string_lossy().to_string();
        let mut out = Vec::new();

        // Pass 1 — hex literals.
        for m in HEX_COLOR.find_iter(content) {
            let hex = m.as_str();
            if self.is_allowlisted(hex) {
                continue;
            }
            let (line, col) = line_col_of(content, m.start());
            let end_col = col + (m.end() - m.start()) as u32;
            out.push(
                Finding::new_line(
                    "theme.hardcoded-hex",
                    Severity::Warning,
                    &file_str,
                    line,
                    col,
                    end_col,
                    format!(
                        "Hardcoded hex color '{}' — replace with var(--color-*) token.",
                        hex
                    ),
                )
                .with_fix("var(--color-TODO)".to_string()),
            );
        }

        // Pass 2 — CSS color functions.
        for m in CSS_FN_COLOR.find_iter(content) {
            let (line, col) = line_col_of(content, m.start());
            let end_col = col + (m.end() - m.start()) as u32;
            out.push(
                Finding::new_line(
                    "theme.hardcoded-css-fn",
                    Severity::Warning,
                    &file_str,
                    line,
                    col,
                    end_col,
                    format!(
                        "Hardcoded CSS color function '{}' — replace with var(--color-*) token.",
                        m.as_str()
                    ),
                )
                .with_fix("var(--color-TODO)".to_string()),
            );
        }

        // Pass 3 — Tailwind utilities without a `dark:` variant on the same
        // class attribute. We scan each `class="..."` / `className="..."`
        // attribute independently.
        for attr in find_class_attrs(content) {
            let has_dark = attr.value.contains("dark:");
            for m in TW_COLOR_CLASS.find_iter(attr.value) {
                let cls = m.as_str();
                if cls.starts_with("dark:") {
                    continue;
                }
                if has_dark {
                    // The element opted into a dark variant somewhere; allow.
                    continue;
                }
                let abs_offset = attr.value_start + m.start();
                let (line, col) = line_col_of(content, abs_offset);
                let end_col = col + (m.end() - m.start()) as u32;
                out.push(Finding::new_line(
                    "theme.missing-dark-variant",
                    Severity::Warning,
                    &file_str,
                    line,
                    col,
                    end_col,
                    format!(
                        "Tailwind class '{}' has no `dark:` sibling. Every color utility must declare a dark variant.",
                        cls
                    ),
                ));
            }
        }

        out
    }
}

/// A single `class=`/`className=` attribute span inside the source text.
struct ClassAttr<'a> {
    value: &'a str,
    /// Byte offset of `value`'s first character in the original `content`.
    value_start: usize,
}

/// Lightweight class-attribute extractor. Walks the bytes once; matches
/// either `class="..."` or `className="..."`, with single or double quotes.
fn find_class_attrs(content: &str) -> Vec<ClassAttr<'_>> {
    let mut out = Vec::new();
    let bytes = content.as_bytes();
    let needles: &[&[u8]] = &[b"class=", b"className="];
    let mut i = 0;
    while i < bytes.len() {
        let mut hit: Option<usize> = None;
        for n in needles {
            if i + n.len() <= bytes.len() && &bytes[i..i + n.len()] == *n {
                hit = Some(n.len());
                break;
            }
        }
        if let Some(name_len) = hit {
            let q_idx = i + name_len;
            if q_idx < bytes.len() && (bytes[q_idx] == b'"' || bytes[q_idx] == b'\'') {
                let quote = bytes[q_idx];
                let val_start = q_idx + 1;
                if let Some(rel_end) = bytes[val_start..].iter().position(|b| *b == quote) {
                    let val_end = val_start + rel_end;
                    if let Ok(s) = std::str::from_utf8(&bytes[val_start..val_end]) {
                        out.push(ClassAttr {
                            value: s,
                            value_start: val_start,
                        });
                    }
                    i = val_end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}
