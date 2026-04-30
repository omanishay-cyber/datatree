//! `MarkdownDriftScanner` — parses `.md` files for path claims (e.g. "the
//! auth flow lives in `src/auth/`") and flags any claim where the
//! referenced path doesn't exist on disk relative to the project root.

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::scanner::{line_col_of, Ast, Finding, Scanner, Severity};

/// Backtick-quoted POSIX-style relative paths (with at least one `/` so we
/// don't match every backtick word). Examples:
///   `src/auth/login.ts`
///   `electron/handlers/auth.ts`
static BACKTICK_PATH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"`([./A-Za-z0-9_\-]+(?:/[A-Za-z0-9_\-./]+)+)`").expect("backtick path regex")
});

/// Markdown link with a relative target: `[text](./foo/bar.md)`.
static MD_LINK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\]\(([./A-Za-z0-9_\-]+(?:/[A-Za-z0-9_\-./]+)+)\)").expect("md link regex")
});

/// Markdown drift scanner.
pub struct MarkdownDriftScanner {
    /// Project root used to resolve the relative claims. When `None` the
    /// scanner reports `applies_to == false`.
    project_root: Option<PathBuf>,
}

impl MarkdownDriftScanner {
    /// Build a scanner. Pass `None` to disable.
    #[must_use]
    pub fn new(project_root: Option<String>) -> Self {
        Self {
            project_root: project_root.map(PathBuf::from),
        }
    }

    fn check_path(&self, claim: &str) -> bool {
        let Some(root) = &self.project_root else {
            return true; // can't verify, treat as fine
        };
        // Strip leading `./`
        let trimmed = claim.trim_start_matches("./");
        let candidate = root.join(trimmed);
        candidate.exists()
    }
}

impl Scanner for MarkdownDriftScanner {
    fn name(&self) -> &str {
        "markdown_drift"
    }

    fn applies_to(&self, file: &Path) -> bool {
        if self.project_root.is_none() {
            return false;
        }
        matches!(
            file.extension().and_then(|e| e.to_str()),
            Some("md") | Some("MD") | Some("markdown")
        )
    }

    fn scan(&self, file: &Path, content: &str, _ast: Option<Ast<'_>>) -> Vec<Finding> {
        let file_str = file.to_string_lossy().to_string();
        let mut out = Vec::new();

        // Skip http(s) and anchor-only targets in MD links. Backtick paths
        // never contain `://` so they're already filtered.
        let report = |re: &Regex, rule: &str| -> Vec<Finding> {
            let mut local = Vec::new();
            for caps in re.captures_iter(content) {
                if let (Some(p), Some(whole)) = (caps.get(1), caps.get(0)) {
                    let claim = p.as_str();
                    if claim.contains("://")
                        || claim.starts_with('#')
                        || claim.starts_with("mailto:")
                    {
                        continue;
                    }
                    if !self.check_path(claim) {
                        let (line, col) = line_col_of(content, whole.start());
                        local.push(Finding::new_line(
                            rule,
                            Severity::Warning,
                            &file_str,
                            line,
                            col,
                            col + (whole.end() - whole.start()) as u32,
                            format!(
                                "Markdown references path '{}' that does not exist in the project.",
                                claim
                            ),
                        ));
                    }
                }
            }
            local
        };

        out.extend(report(&BACKTICK_PATH, "markdown.dead-backtick-path"));
        out.extend(report(&MD_LINK, "markdown.dead-link"));
        out
    }
}
