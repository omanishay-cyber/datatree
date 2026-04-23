//! `A11yScanner` — accessibility smells in JSX / HTML.
//!
//! Patterns flagged:
//! - icon-only buttons (no text child) without `aria-label`
//! - `<img>` without `alt`
//! - `<button>` without text *and* without role/label
//! - `<a>` without `href`
//! - interactive elements missing `focus-visible:` Tailwind class

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

use crate::scanner::{line_col_of, Ast, Finding, Scanner, Severity};

/// `<button ...>...</button>` opening tag matcher (single-line, attrs captured).
static BUTTON_OPEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<button\b([^>]*)>").expect("button regex"));

/// `<img ...>` (self-closing or not).
static IMG_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<img\b([^>]*)/?>").expect("img regex"));

/// `<a ...>` opening tag.
static ANCHOR_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<a\b([^>]*)>").expect("a regex"));

/// `aria-label="..."` attribute.
static ARIA_LABEL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\baria-label\s*=\s*["'][^"']+["']"#).expect("aria-label regex"));

/// `alt="..."` attribute.
static ALT_ATTR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\balt\s*=\s*["'][^"']*["']"#).expect("alt regex"));

/// `href="..."` attribute.
static HREF_ATTR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\bhref\s*=\s*["'][^"']*["']"#).expect("href regex"));

/// Tailwind `focus-visible:` class on the element. Heuristic.
static FOCUS_VISIBLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"focus-visible:").expect("focus-visible regex"));

const A11Y_EXTS: &[&str] = &["tsx", "jsx", "html", "vue", "svelte"];

/// Accessibility scanner.
pub struct A11yScanner;

impl Default for A11yScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl A11yScanner {
    /// New scanner. Stateless.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Scanner for A11yScanner {
    fn name(&self) -> &str {
        "a11y"
    }

    fn applies_to(&self, file: &Path) -> bool {
        file.extension()
            .and_then(|e| e.to_str())
            .map(|e| A11Y_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    fn scan(&self, file: &Path, content: &str, _ast: Option<Ast<'_>>) -> Vec<Finding> {
        let file_str = file.to_string_lossy().to_string();
        let mut out = Vec::new();

        // <button> — needs either text content OR aria-label.
        for caps in BUTTON_OPEN.captures_iter(content) {
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let open = caps.get(0).expect("group 0");
            let close_idx = content[open.end()..]
                .find("</button>")
                .map(|i| open.end() + i)
                .unwrap_or(content.len());
            let inner = &content[open.end()..close_idx];
            let inner_text = strip_tags(inner);
            let has_text = !inner_text.trim().is_empty();
            let has_aria = ARIA_LABEL.is_match(attrs);
            if !has_text && !has_aria {
                let (line, col) = line_col_of(content, open.start());
                out.push(
                    Finding::new_line(
                        "a11y.icon-button-no-label",
                        Severity::Error,
                        &file_str,
                        line,
                        col,
                        col + (open.end() - open.start()) as u32,
                        "Icon-only <button> must include an aria-label.",
                    )
                    .with_fix(r#"<button aria-label="TODO" "#.to_string()),
                );
            }
            if !FOCUS_VISIBLE.is_match(attrs) {
                let (line, col) = line_col_of(content, open.start());
                out.push(Finding::new_line(
                    "a11y.button-missing-focus-visible",
                    Severity::Warning,
                    &file_str,
                    line,
                    col,
                    col + (open.end() - open.start()) as u32,
                    "<button> missing `focus-visible:` Tailwind class — keyboard focus must be visible.",
                ));
            }
        }

        // <img> — needs alt.
        for caps in IMG_TAG.captures_iter(content) {
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if !ALT_ATTR.is_match(attrs) {
                let m = caps.get(0).expect("group 0");
                let (line, col) = line_col_of(content, m.start());
                out.push(
                    Finding::new_line(
                        "a11y.img-no-alt",
                        Severity::Error,
                        &file_str,
                        line,
                        col,
                        col + (m.end() - m.start()) as u32,
                        "<img> missing `alt` attribute (use empty alt for decorative images).",
                    )
                    .with_fix(r#"alt="" "#.to_string()),
                );
            }
        }

        // <a> — needs href (otherwise it's not a link, should be a button).
        for caps in ANCHOR_TAG.captures_iter(content) {
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if !HREF_ATTR.is_match(attrs) {
                let m = caps.get(0).expect("group 0");
                let (line, col) = line_col_of(content, m.start());
                out.push(Finding::new_line(
                    "a11y.anchor-no-href",
                    Severity::Error,
                    &file_str,
                    line,
                    col,
                    col + (m.end() - m.start()) as u32,
                    "<a> without href is not a link — use <button> for interactive controls.",
                ));
            }
        }

        out
    }
}

/// Strip HTML/JSX tags from a fragment so we can check for visible text.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            ch if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}
