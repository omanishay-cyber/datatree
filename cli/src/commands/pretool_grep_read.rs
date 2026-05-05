//! `mneme pretool-grep-read` — Wave 1E Layer 3 hook entry.
//!
//! Claude Code spawns this binary on every PreToolUse for Grep / Read /
//! Glob. It is the third leg of the v0.4.0 self-ping enforcement stack:
//!
//!   Layer 1 (UserPromptSubmit) — nudge mneme on resume signals.
//!   Layer 2 (PreToolUse Edit/Write) — gate edits on blast_radius.
//!   Layer 3 (PreToolUse Grep/Read) — soft-redirect symbol queries to
//!                                    `mneme find_references` /
//!                                    `mneme blast_radius` instead of
//!                                    text-grep / blind read.
//!
//! ## What this ships in v0.4.0 (Item #122)
//!
//! Soft-redirect only. The hook NEVER blocks — it always approves so
//! the Grep / Read still runs. When the input looks like a symbol
//! query (alphanumeric identifier, dotted module path, PascalCase
//! type name) or a substantive source-file Read, we attach a one-
//! sentence `additionalContext` hint suggesting the equivalent mneme
//! tool. The AI sees the hint alongside the result and adapts on its
//! next call.
//!
//! Why soft, not hard: a hard block on every Read would tank Claude
//! Code's ergonomics — it reads test fixtures, reads config files,
//! reads files the user explicitly mentioned. The hint pattern keeps
//! the muscle memory pointing at mneme without ever stopping a tool
//! call dead.
//!
//! ## Config (read-only at hook fire-time)
//!
//! `~/.mneme/config.toml` `[hooks] enforce_recall_before_grep`
//!   - `true` (default in v0.4.0): emit the soft-redirect hint
//!   - `false`: legacy always-approve passthrough
//!
//! ## Hook output protocol
//!
//! ```json
//! { "hook_specific": { "decision": "approve",
//!                      "additionalContext": "mneme tip: ..." } }
//! ```
//!
//! `additionalContext` is OMITTED when no hint applies (ordinary
//! file path Reads, glob patterns with wildcards, multi-word natural-
//! language Greps, etc.). Exit 0 always — fail-open.

use clap::Args;
use serde_json::{json, Value};
use std::io::{self, Read};
use std::path::PathBuf;

use crate::error::CliResult;

/// Maximum stdin payload size for the hook (SEC-001 fix, 2026-05-05).
/// Claude Code's largest legitimate PreToolUse payload is a tool-input
/// JSON envelope around the size of one source-file path or grep
/// pattern — kilobytes at most. Capping at 1 MiB is generous for the
/// real surface and bounds the worst case at a constant: any sibling
/// process feeding the hook unbounded data can't OOM the hot path.
const MAX_STDIN_BYTES: u64 = 1024 * 1024;

/// Maximum config.toml size we'll read before giving up + falling back
/// to defaults (SEC-007 fix). User configs are tens of bytes; even a
/// generous cap of 64 KiB is 1000× the realistic ceiling. A larger
/// file is either accidental or hostile — either way DEFAULT is safer
/// than parsing a multi-megabyte TOML on every Grep/Read fire.
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// CLI args for `mneme pretool-grep-read`. All optional — payload
/// comes from stdin per Claude Code's hook contract.
#[derive(Debug, Args, Default)]
pub struct PretoolGrepReadArgs {}

/// Entry point — wired into `cli/src/main.rs`.
pub async fn run(_args: PretoolGrepReadArgs) -> CliResult<()> {
    let payload = read_stdin_payload();
    let hint = if hooks_enforce_recall_before_grep() {
        compute_redirect_hint(payload.as_deref().unwrap_or(""))
    } else {
        None
    };
    emit_decision(hint.as_deref());
    Ok(())
}

fn read_stdin_payload() -> Option<String> {
    let mut buf = String::new();
    // SEC-001 fix (2026-05-05): cap reads at MAX_STDIN_BYTES so a
    // sibling process feeding the hook unbounded data can't OOM the
    // session. `take` returns EOF after the cap, which leaves us
    // with a truncated-but-valid (or nearly-valid) JSON payload —
    // worst case the parse fails and we fall through to the silent-
    // approve fallback.
    io::stdin()
        .take(MAX_STDIN_BYTES)
        .read_to_string(&mut buf)
        .ok()?;
    Some(buf)
}

fn emit_decision(additional_context: Option<&str>) {
    let mut spec = json!({ "decision": "approve" });
    if let Some(ctx) = additional_context {
        if let Value::Object(ref mut m) = spec {
            m.insert("additionalContext".into(), Value::String(ctx.to_string()));
        }
    }
    let out = json!({ "hook_specific": spec });
    println!("{out}");
}

// ---------------------------------------------------------------------
// Config — read `[hooks] enforce_recall_before_grep` from
// ~/.mneme/config.toml. Default: true (soft-redirect ON in v0.4.0).
// ---------------------------------------------------------------------

fn hooks_enforce_recall_before_grep() -> bool {
    const DEFAULT: bool = true;
    let path = match config_path() {
        Some(p) => p,
        None => return DEFAULT,
    };
    // SEC-007 fix (2026-05-05): pre-flight size check so a multi-MB
    // config.toml (accidental or hostile) doesn't dominate the hot
    // hook path. We re-read on every invocation since the hook is a
    // short-lived process; the metadata + read are cheap when the
    // file is the typical few hundred bytes.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_CONFIG_BYTES {
            return DEFAULT;
        }
    }
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return DEFAULT,
    };
    let parsed: toml::Value = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DEFAULT,
    };
    parsed
        .get("hooks")
        .and_then(|h| h.get("enforce_recall_before_grep"))
        .and_then(|v| v.as_bool())
        .unwrap_or(DEFAULT)
}

fn config_path() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("MNEME_HOME") {
        return Some(PathBuf::from(home).join("config.toml"));
    }
    // common::paths::PathManager::default_root reads MNEME_HOME or
    // falls back to a platform-specific dir. We can't import that
    // here (would require an async runtime + I/O for a hot hook),
    // so we mirror the env-var contract and let the platform helper
    // synthesize the dir lazily.
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".mneme").join("config.toml"))
}

// ---------------------------------------------------------------------
// Redirect-hint logic.
// ---------------------------------------------------------------------

/// Inspect the Claude Code PreToolUse JSON payload and return a
/// soft-redirect hint when the call looks like something a mneme
/// MCP tool would answer better. Returns `None` for inputs where
/// the hint would be noise (multi-word grep, generic file path
/// Reads, glob wildcards, missing fields).
pub(crate) fn compute_redirect_hint(payload_json: &str) -> Option<String> {
    let payload: Value = serde_json::from_str(payload_json).ok()?;
    let tool = payload.get("tool_name").and_then(|v| v.as_str())?;
    let input = payload.get("tool_input")?;
    match tool {
        "Grep" => grep_hint(input),
        "Read" => read_hint(input),
        // Glob is structural by nature (paths, not symbols) — no hint.
        _ => None,
    }
}

fn grep_hint(input: &Value) -> Option<String> {
    let pattern = input.get("pattern").and_then(|v| v.as_str())?.trim();
    if !is_symbol_shaped(pattern) {
        return None;
    }
    Some(format!(
        "mneme tip: \"{}\" looks like a code symbol — \
         `mcp__mneme__find_references` returns structured (file, line, kind) \
         hits with the symbol resolver applied, typically faster + more \
         precise than text grep. The Grep is approved; prefer mneme on the \
         next symbol query.",
        sanitize_for_message(pattern, 80)
    ))
}

fn read_hint(input: &Value) -> Option<String> {
    let path = input.get("file_path").and_then(|v| v.as_str())?.trim();
    if !is_source_file(path) {
        return None;
    }
    Some(format!(
        "mneme tip: before non-trivial edits to `{}`, \
         `mcp__mneme__blast_radius` returns the (callers, dependents, tests) \
         set in <100 ms — much cheaper than reading the file plus its \
         consumers manually. The Read is approved; consider blast_radius if \
         the next step is an edit.",
        sanitize_for_message(path, 120)
    ))
}

/// Heuristic: does this look like a code symbol the resolver could
/// answer for? Accept identifiers, dotted module paths, `::`-paths,
/// and PascalCase names. Reject regex metacharacters (those signal
/// the user wants pattern matching, not symbol lookup), shell glob
/// wildcards, and multi-word natural-language phrases.
pub(crate) fn is_symbol_shaped(pattern: &str) -> bool {
    let s = pattern.trim();
    if s.is_empty() || s.len() > 200 {
        return false;
    }
    // Multi-word phrases are not symbols.
    if s.split_whitespace().count() > 1 {
        return false;
    }
    // Regex metacharacters disqualify — the user wants regex matching.
    // We allow `::` (Rust path separator) and `.` (Python path) by
    // not listing them here, but we still reject `\`, `[`, `]`, `(`,
    // `)`, `*`, `+`, `?`, `|`, `^`, `$`, `{`, `}`.
    const REGEX_META: &[char] = &[
        '\\', '[', ']', '(', ')', '*', '+', '?', '|', '^', '$', '{', '}',
    ];
    if s.chars().any(|c| REGEX_META.contains(&c)) {
        return false;
    }
    // Path-like shapes (`/foo/bar`) are file references, not symbols.
    if s.contains('/') {
        return false;
    }
    // Every remaining char must be ASCII alnum, `_`, `:`, or `.` —
    // covers `foo_bar`, `WorkerPool`, `crate::manager::spawn`, and
    // `pkg.sub.mod`. SEC-006 fix (2026-05-05): was `is_alphanumeric`
    // which accepts Unicode RTL-override marks, zero-width joiners,
    // and homoglyphs (e.g. Cyrillic 'а' for Latin 'a'). Those would
    // pass the symbol-shape gate and reach `additionalContext`,
    // letting a crafted file path or grep pattern smuggle prompt-
    // injection into the AI's context. ASCII-only kicks out the
    // entire homoglyph + bidi class. Real non-ASCII identifier
    // codebases (Cyrillic, Han) lose the soft-redirect hint, but
    // the Grep itself still runs — trade-off accepted.
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '.')
}

/// Heuristic: does this path point at human-written source code that
/// would benefit from a blast-radius preview? Source-code extensions
/// only (no JSON, no images, no Markdown — those are usually data the
/// user actually wants to read).
pub(crate) fn is_source_file(path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return false;
    }
    // Forward-slash normalization for the suffix check.
    let lower = p.to_ascii_lowercase().replace('\\', "/");
    const EXTS: &[&str] = &[
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".swift", ".cpp", ".cc",
        ".c", ".h", ".hpp", ".rb", ".php", ".cs",
    ];
    EXTS.iter().any(|ext| lower.ends_with(ext))
}

/// Trim long inputs so the injected message stays under control. We
/// inject this into the AI's context window — a 5-KB pattern would
/// make the hint counterproductive.
fn sanitize_for_message(s: &str, max: usize) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).collect();
    if cleaned.len() <= max {
        cleaned
    } else {
        let mut out: String = cleaned.chars().take(max).collect();
        out.push('…');
        out
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_symbol_shaped --------------------------------------------

    #[test]
    fn symbol_shape_accepts_plain_identifier() {
        assert!(is_symbol_shaped("WorkerPool"));
        assert!(is_symbol_shaped("spawn"));
        assert!(is_symbol_shaped("snake_case_name"));
        assert!(is_symbol_shaped("camelCase"));
        assert!(is_symbol_shaped("CONST_VALUE"));
    }

    #[test]
    fn symbol_shape_accepts_dotted_and_double_colon_paths() {
        assert!(is_symbol_shaped("crate::manager::spawn"));
        assert!(is_symbol_shaped("pkg.sub.mod"));
        assert!(is_symbol_shaped("std::collections::HashMap"));
    }

    #[test]
    fn symbol_shape_rejects_regex_metacharacters() {
        assert!(!is_symbol_shaped("foo.*bar"));
        assert!(!is_symbol_shaped("(WorkerPool|JobQueue)"));
        assert!(!is_symbol_shaped("^impl "));
        assert!(!is_symbol_shaped("foo\\s+bar"));
        assert!(!is_symbol_shaped("foo[A-Z]"));
    }

    #[test]
    fn symbol_shape_rejects_paths_and_multiword_phrases() {
        assert!(!is_symbol_shaped("src/manager.rs"));
        assert!(!is_symbol_shaped("how does spawn work"));
        assert!(!is_symbol_shaped("fn spawn"));
        assert!(!is_symbol_shaped("/home/anish/code"));
    }

    #[test]
    fn symbol_shape_rejects_empty_and_oversize() {
        assert!(!is_symbol_shaped(""));
        assert!(!is_symbol_shaped("   "));
        let long: String = "a".repeat(300);
        assert!(!is_symbol_shaped(&long));
    }

    // ---- is_source_file ----------------------------------------------

    #[test]
    fn source_file_recognises_common_extensions() {
        assert!(is_source_file("src/main.rs"));
        assert!(is_source_file("vision/src/views/Foo.tsx"));
        assert!(is_source_file("pkg/sub/mod.py"));
        assert!(is_source_file("cmd/main.go"));
        assert!(is_source_file(r"cli\src\commands\build.rs"));
    }

    #[test]
    fn source_file_rejects_non_code() {
        assert!(!is_source_file("README.md"));
        assert!(!is_source_file("docs/index.html"));
        assert!(!is_source_file("config.toml"));
        assert!(!is_source_file("data.json"));
        assert!(!is_source_file("image.png"));
        assert!(!is_source_file(""));
    }

    // ---- compute_redirect_hint (integration) -------------------------

    #[test]
    fn hint_for_symbol_grep_includes_find_references() {
        let payload = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Grep",
            "tool_input": { "pattern": "WorkerPool" }
        })
        .to_string();
        let h = compute_redirect_hint(&payload).expect("hint expected");
        assert!(h.contains("WorkerPool"));
        assert!(h.contains("mcp__mneme__find_references"));
        assert!(h.contains("approved"));
    }

    #[test]
    fn hint_skipped_for_regex_grep() {
        let payload = json!({
            "tool_name": "Grep",
            "tool_input": { "pattern": "fn\\s+\\w+\\(" }
        })
        .to_string();
        assert!(compute_redirect_hint(&payload).is_none());
    }

    #[test]
    fn hint_for_source_read_includes_blast_radius() {
        let payload = json!({
            "tool_name": "Read",
            "tool_input": { "file_path": "supervisor/src/manager.rs" }
        })
        .to_string();
        let h = compute_redirect_hint(&payload).expect("hint expected");
        assert!(h.contains("supervisor/src/manager.rs"));
        assert!(h.contains("mcp__mneme__blast_radius"));
        assert!(h.contains("approved"));
    }

    #[test]
    fn hint_skipped_for_markdown_read() {
        let payload = json!({
            "tool_name": "Read",
            "tool_input": { "file_path": "README.md" }
        })
        .to_string();
        assert!(compute_redirect_hint(&payload).is_none());
    }

    #[test]
    fn hint_skipped_for_glob_tool() {
        let payload = json!({
            "tool_name": "Glob",
            "tool_input": { "pattern": "**/*.rs" }
        })
        .to_string();
        assert!(compute_redirect_hint(&payload).is_none());
    }

    #[test]
    fn hint_skipped_for_unknown_tool() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls" }
        })
        .to_string();
        assert!(compute_redirect_hint(&payload).is_none());
    }

    #[test]
    fn hint_skipped_for_malformed_payload() {
        assert!(compute_redirect_hint("not json").is_none());
        assert!(compute_redirect_hint("{}").is_none());
        // Missing tool_input.
        let p = json!({ "tool_name": "Grep" }).to_string();
        assert!(compute_redirect_hint(&p).is_none());
    }

    #[test]
    fn sanitize_truncates_with_ellipsis() {
        let s = "x".repeat(500);
        let out = sanitize_for_message(&s, 100);
        // 100 chars + 1 ellipsis char.
        assert_eq!(out.chars().count(), 101);
        assert!(out.ends_with('…'));
    }
}
