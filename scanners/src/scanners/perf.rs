//! `PerfScanner` — React/JS performance smells.
//!
//! Patterns flagged:
//! - components rendered in tight loops without React.memo
//! - useEffect calls without a dependency array
//! - synchronous I/O in render bodies (`fs.readFileSync`, `XMLHttpRequest` sync)
//! - sequential setState calls within the same handler that could be batched

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

use crate::scanner::{line_col_of, Ast, Finding, Scanner, Severity};

/// `.map(...)` returning a JSX element. Heuristic: `.map(` followed within
/// 200 chars by `<Some/`. We then check whether the wrapping component is
/// memoized (`React.memo` or `memo(`) anywhere in the file.
static JSX_MAP_LOOP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.map\s*\(\s*\([^)]*\)\s*=>").expect("jsx map regex"));

/// `useEffect(() => { ... })` — no dep array (the second argument is missing).
/// Heuristic: matches `useEffect(...)` and we then look at the call's tail.
static USE_EFFECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\buseEffect\s*\(").expect("useEffect regex"));

/// Synchronous filesystem APIs.
static SYNC_IO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:readFileSync|writeFileSync|existsSync|statSync|readdirSync)\s*\(")
        .expect("sync io regex")
});

/// `setState(...)` style hook calls — captures variable.
static SET_STATE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bset[A-Z][A-Za-z0-9_]*\s*\(").expect("setState regex")
});

const PERF_EXTS: &[&str] = &["tsx", "jsx", "ts", "js"];

/// Performance scanner.
pub struct PerfScanner;

impl Default for PerfScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfScanner {
    /// New scanner. Stateless.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Scanner for PerfScanner {
    fn name(&self) -> &str {
        "perf"
    }

    fn applies_to(&self, file: &Path) -> bool {
        file.extension()
            .and_then(|e| e.to_str())
            .map(|e| PERF_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    fn scan(&self, file: &Path, content: &str, _ast: Option<Ast<'_>>) -> Vec<Finding> {
        let file_str = file.to_string_lossy().to_string();
        let mut out = Vec::new();
        let has_memo = content.contains("React.memo") || content.contains("memo(");

        // 1. JSX inside a .map() arrow without React.memo anywhere.
        if !has_memo {
            for m in JSX_MAP_LOOP.find_iter(content) {
                let lookahead = &content[m.end()..(m.end() + 200).min(content.len())];
                if lookahead.contains('<') {
                    let (line, col) = line_col_of(content, m.start());
                    out.push(Finding::new_line(
                        "perf.unmemoized-list-item",
                        Severity::Info,
                        &file_str,
                        line,
                        col,
                        col + (m.end() - m.start()) as u32,
                        "Component rendered in a list without React.memo — consider memoizing the row component.",
                    ));
                }
            }
        }

        // 2. useEffect without a dependency array. Walk parens to find the
        //    matching `)` for each `useEffect(`.
        for m in USE_EFFECT.find_iter(content) {
            if let Some(close) = find_matching_paren(content, m.end() - 1) {
                let body = &content[m.end()..close];
                // The body must contain a "," at depth 0 to indicate a deps arg.
                let has_deps = arg_separator_at_depth_zero(body);
                if !has_deps {
                    let (line, col) = line_col_of(content, m.start());
                    out.push(
                        Finding::new_line(
                            "perf.useeffect-no-deps",
                            Severity::Warning,
                            &file_str,
                            line,
                            col,
                            col + (m.end() - m.start()) as u32,
                            "useEffect missing dependency array — runs after every render.",
                        )
                        .with_fix(", []".to_string()),
                    );
                }
            }
        }

        // 3. Synchronous I/O calls anywhere.
        for m in SYNC_IO.find_iter(content) {
            let (line, col) = line_col_of(content, m.start());
            out.push(Finding::new_line(
                "perf.sync-io",
                Severity::Error,
                &file_str,
                line,
                col,
                col + (m.end() - m.start()) as u32,
                "Synchronous I/O blocks the event loop — use async equivalents.",
            ));
        }

        // 4. Three or more setState-style hook calls within a 200-byte
        //    window suggest unbatched updates.
        let setters: Vec<usize> = SET_STATE.find_iter(content).map(|m| m.start()).collect();
        for window in setters.windows(3) {
            if window[2] - window[0] <= 200 {
                let (line, col) = line_col_of(content, window[0]);
                out.push(Finding::new_line(
                    "perf.unbatched-setstate",
                    Severity::Info,
                    &file_str,
                    line,
                    col,
                    col + 1,
                    "Three or more sequential setState-style calls — consider unstable_batchedUpdates / React 18 auto-batching, or merge state.",
                ));
                break;
            }
        }

        out
    }
}

/// Given `content` and the byte index of an open `(`, return the byte
/// index of the matching `)` honoring nested parens. Returns `None` if
/// unbalanced.
fn find_matching_paren(content: &str, open_idx: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    if bytes.get(open_idx) != Some(&b'(') {
        return None;
    }
    let mut depth: i32 = 0;
    for (i, b) in bytes.iter().enumerate().skip(open_idx) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// True if `body` contains a `,` at paren/bracket/brace depth zero —
/// meaning the call has more than one argument.
fn arg_separator_at_depth_zero(body: &str) -> bool {
    let mut paren = 0i32;
    let mut brack = 0i32;
    let mut brace = 0i32;
    for b in body.bytes() {
        match b {
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => brack += 1,
            b']' => brack -= 1,
            b'{' => brace += 1,
            b'}' => brace -= 1,
            b',' if paren == 0 && brack == 0 && brace == 0 => return true,
            _ => {}
        }
    }
    false
}
