//! Skill-matcher — the minimal port of the TS `suggest_skill` algorithm
//! into the CLI so the inject hook can surface a recommendation without
//! requiring the MCP server to be up.
//!
//! Responsibilities:
//!
//!   1. Locate every `SKILL.md` under `~/.mneme/plugin/skills/` (with a
//!      dev fallback to the repo's `plugin/skills/` when the binary is
//!      run from a source checkout).
//!   2. Extract `name`, `triggers`, and `tags` from the `---` delimited
//!      YAML frontmatter using a tiny hand-rolled parser — we do NOT
//!      pull in a full YAML dep here. SKILL.md frontmatter is a
//!      scoped dialect (scalar strings + inline arrays + block arrays).
//!   3. Score each skill against the user's prompt: triggers weigh 2x,
//!      tags weigh 1x. Return the top `limit` hits with confidence
//!      buckets (`high >= 3`, `medium >= 1`, `low > 0`).
//!   4. Always exit cleanly. A parse failure on one SKILL.md logs and
//!      continues. A missing directory returns an empty vector. The
//!      hook's promise is "never block Claude Code" and this module
//!      upholds it from the bottom of the call stack.
//!
//! The TS side's richer behaviour (e.g. dedup across roots, the
//! codeword nudge) is preserved here so both paths produce consistent
//! recommendations.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Confidence bucket assigned to a scored match. Mirrors the TS
/// `ConfidenceEnum`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }
}

/// A scored skill suggestion.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub skill: String,
    pub triggers_matched: Vec<String>,
    pub tags_matched: Vec<String>,
    pub confidence: Confidence,
    pub source_path: PathBuf,
    pub score: u32,
}

/// A parsed SKILL.md descriptor. Only the fields we need for matching.
#[derive(Debug, Clone)]
struct ParsedSkill {
    name: String,
    triggers: Vec<String>,
    tags: Vec<String>,
    source_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Stopwords — kept deliberately small.
// ---------------------------------------------------------------------------

const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "of", "to", "for", "with", "in", "on", "at", "is", "it",
    "this", "that", "my", "i", "me", "we", "our", "please", "help", "need", "want", "can",
    "could", "you", "your", "do", "does", "did", "have", "has", "had", "be", "been", "am",
    "are", "was", "were", "if", "then", "so", "but", "not", "no", "yes",
];

const CODEWORDS: &[&str] = &["coldstart", "hotstart", "firestart", "chs"];

// ---------------------------------------------------------------------------
// Directory resolution.
// ---------------------------------------------------------------------------

/// Return the ordered list of directories that might hold `SKILL.md`
/// files. The installed plugin wins over the in-repo fallback so a
/// production binary always prefers `~/.mneme/plugin/skills/`.
fn candidate_skill_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".mneme").join("plugin").join("skills"));
    }
    // Dev fallback: walk up from the binary location looking for a
    // `plugin/skills/` directory. Covers `cargo run` and the
    // `target/release/mneme.exe` test layout.
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor = exe.as_path();
        for _ in 0..6 {
            if let Some(parent) = cursor.parent() {
                let candidate = parent.join("plugin").join("skills");
                if candidate.is_dir() {
                    out.push(candidate);
                }
                cursor = parent;
            } else {
                break;
            }
        }
    }
    // `cargo run` from the workspace root.
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("plugin").join("skills");
        if candidate.is_dir() {
            out.push(candidate);
        }
    }
    // Dedup while preserving first-seen order.
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

fn walk_skill_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.is_file() {
                out.push(skill_md);
            }
        } else if ft.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.eq_ignore_ascii_case("SKILL.md") {
                    out.push(path);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tiny YAML frontmatter parser.
//
// We intentionally do not depend on `serde_yaml` (not in the workspace
// dep tree). SKILL.md frontmatter is a tightly scoped dialect and a
// small hand-rolled parser is both cheap and bounded.
// ---------------------------------------------------------------------------

/// Extract the text between the opening and closing `---` fences. Returns
/// `None` when the file doesn't begin with a frontmatter block.
fn slice_frontmatter(text: &str) -> Option<&str> {
    // Drop UTF-8 BOM if present.
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    // Normalise CRLF for the byte-index math below. We borrow a slice
    // of the original, so `replace` would allocate — cheaper to just
    // locate the delimiter with `find` and accept both `\n---` and
    // `\r\n---` as terminators.
    if !(body.starts_with("---\n") || body.starts_with("---\r\n")) {
        return None;
    }
    let after_open = if body.starts_with("---\r\n") {
        &body[5..]
    } else {
        &body[4..]
    };
    // Find the next `---` that sits at the start of a line.
    for (idx, _) in after_open.match_indices("---") {
        let at_line_start = idx == 0
            || after_open.as_bytes().get(idx.saturating_sub(1)) == Some(&b'\n')
            || (idx >= 2
                && after_open.as_bytes().get(idx - 2) == Some(&b'\n')
                && after_open.as_bytes().get(idx - 1) == Some(&b'\r'));
        if at_line_start {
            return Some(&after_open[..idx]);
        }
    }
    None
}

#[derive(Debug)]
enum YamlValue {
    Scalar(String),
    Array(Vec<String>),
}

fn strip_yaml_scalar(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn split_inline_array(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                buf.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == ',' {
            let t = buf.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
            buf.clear();
            continue;
        }
        buf.push(ch);
    }
    let tail = buf.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn parse_tiny_yaml(block: &str) -> Vec<(String, YamlValue)> {
    let lines: Vec<&str> = block.split('\n').collect();
    let mut out: Vec<(String, YamlValue)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i].trim_end_matches(['\r', '\t', ' ']);
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        // Top-level key only — SKILL.md frontmatter doesn't nest beyond
        // arrays of strings.
        if raw.starts_with(|c: char| c.is_whitespace()) {
            i += 1;
            continue;
        }
        let colon = match raw.find(':') {
            Some(c) => c,
            None => {
                i += 1;
                continue;
            }
        };
        let key_part = &raw[..colon];
        if !key_part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            || key_part.is_empty()
        {
            i += 1;
            continue;
        }
        let key = key_part.to_string();
        let rest = raw[colon + 1..].trim().to_string();
        if rest.is_empty() || rest == ">" || rest == "|" {
            // Block scalar or block array starts on the next non-empty
            // indented line.
            let mut items: Vec<String> = Vec::new();
            let mut is_array = false;
            let mut folded: Vec<String> = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let next = lines[j];
                if next.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let leading_ws = next.starts_with(|c: char| c.is_whitespace());
                if !leading_ws {
                    break;
                }
                let inner = next.trim_start();
                if let Some(item_body) = inner.strip_prefix("- ") {
                    is_array = true;
                    items.push(strip_yaml_scalar(item_body));
                    j += 1;
                    continue;
                }
                if inner.starts_with('-') && inner.len() == 1 {
                    // `-` alone — treat as an empty array entry.
                    is_array = true;
                    items.push(String::new());
                    j += 1;
                    continue;
                }
                if !is_array {
                    folded.push(inner.to_string());
                    j += 1;
                    continue;
                }
                break;
            }
            if is_array {
                out.push((key, YamlValue::Array(items)));
            } else if !folded.is_empty() {
                out.push((key, YamlValue::Scalar(folded.join(" "))));
            } else {
                out.push((key, YamlValue::Scalar(String::new())));
            }
            i = j;
            continue;
        }
        if rest.starts_with('[') && rest.ends_with(']') {
            let inner = &rest[1..rest.len() - 1];
            out.push((key, YamlValue::Array(split_inline_array(inner))));
            i += 1;
            continue;
        }
        out.push((key, YamlValue::Scalar(strip_yaml_scalar(&rest))));
        i += 1;
    }
    out
}

fn value_as_array(v: &YamlValue) -> Vec<String> {
    match v {
        YamlValue::Array(a) => a.iter().filter(|s| !s.is_empty()).cloned().collect(),
        YamlValue::Scalar(s) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => Vec::new(),
    }
}

fn value_as_scalar(v: &YamlValue) -> String {
    match v {
        YamlValue::Scalar(s) => s.trim().to_string(),
        YamlValue::Array(a) => a.join(" "),
    }
}

fn parse_skill_file(path: &Path) -> Option<ParsedSkill> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let fm = slice_frontmatter(&text)?;
    let kv = parse_tiny_yaml(fm);
    let mut name = String::new();
    let mut triggers: Vec<String> = Vec::new();
    let mut tags: Vec<String> = Vec::new();
    for (k, v) in kv {
        match k.as_str() {
            "name" => name = value_as_scalar(&v),
            "triggers" => {
                triggers = value_as_array(&v)
                    .into_iter()
                    .map(|s| s.to_lowercase())
                    .collect();
            }
            "tags" => {
                tags = value_as_array(&v)
                    .into_iter()
                    .map(|s| s.to_lowercase())
                    .collect();
            }
            _ => {}
        }
    }
    if name.is_empty() {
        return None;
    }
    Some(ParsedSkill {
        name,
        triggers,
        tags,
        source_path: path.to_path_buf(),
    })
}

fn load_skills() -> Vec<ParsedSkill> {
    let mut out: Vec<ParsedSkill> = Vec::new();
    let mut seen = HashSet::new();
    for dir in candidate_skill_dirs() {
        if !dir.is_dir() {
            continue;
        }
        for file in walk_skill_files(&dir) {
            if let Some(skill) = parse_skill_file(&file) {
                if seen.insert(skill.name.clone()) {
                    out.push(skill);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Matching.
// ---------------------------------------------------------------------------

fn tokenize(task: &str) -> HashSet<String> {
    let lowered = task.to_lowercase();
    let mut out = HashSet::new();
    for raw in lowered.split(|c: char| {
        !(c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '.' || c == '-')
    }) {
        if raw.len() < 2 {
            continue;
        }
        if STOPWORDS.contains(&raw) {
            continue;
        }
        out.insert(raw.to_string());
    }
    out
}

fn trigger_matches(trigger: &str, lowered_task: &str, tokens: &HashSet<String>) -> bool {
    let t = trigger.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains(' ') {
        return lowered_task.contains(t);
    }
    if tokens.contains(t) {
        return true;
    }
    // Allow substring match for tokens with special chars.
    if t.chars()
        .any(|c| c == '-' || c == '_' || c == '+' || c == '.' || c.is_ascii_digit())
        && lowered_task.contains(t)
    {
        return true;
    }
    false
}

fn confidence_for(score: u32) -> Confidence {
    if score >= 3 {
        Confidence::High
    } else if score >= 1 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

fn codeword_hit(task: &str) -> Option<&'static str> {
    let lowered = task.to_lowercase();
    for cw in CODEWORDS {
        // Require word boundaries so "coldstart" hits but
        // "coldstartup" doesn't.
        let mut search_start = 0;
        while let Some(idx) = lowered[search_start..].find(cw) {
            let abs = search_start + idx;
            let before_ok = abs == 0
                || !matches!(lowered.as_bytes()[abs - 1], b'a'..=b'z' | b'0'..=b'9');
            let end = abs + cw.len();
            let after_ok = end >= lowered.len()
                || !matches!(lowered.as_bytes()[end], b'a'..=b'z' | b'0'..=b'9');
            if before_ok && after_ok {
                return Some(cw);
            }
            search_start = abs + cw.len();
        }
    }
    None
}

/// Return the top `limit` skill suggestions for `task`. Always
/// succeeds — a missing skills directory yields an empty `Vec`.
pub fn suggest(task: &str, limit: usize) -> Vec<Suggestion> {
    let skills = load_skills();
    let lowered_task = task.to_lowercase();
    let tokens = tokenize(task);
    let mut scored: Vec<Suggestion> = Vec::new();

    for skill in &skills {
        let mut triggers_matched = Vec::new();
        let mut tags_matched = Vec::new();
        for trig in &skill.triggers {
            if trigger_matches(trig, &lowered_task, &tokens) {
                triggers_matched.push(trig.clone());
            }
        }
        // De-dup tags against triggers: if a tag string also appears as a
        // trigger on the same skill, don't count it twice. Otherwise skills
        // whose authors mirrored their triggers into tags (e.g. fireworks-
        // test: `tags: [test, tdd, ...]` + `triggers: [test, tdd, ...]`)
        // get an unfair double-score for a single keyword hit.
        let trigger_set: HashSet<&str> =
            skill.triggers.iter().map(String::as_str).collect();
        for tag in &skill.tags {
            if trigger_set.contains(tag.as_str()) {
                continue;
            }
            if trigger_matches(tag, &lowered_task, &tokens) {
                tags_matched.push(tag.clone());
            }
        }
        let score = (triggers_matched.len() as u32) * 2 + tags_matched.len() as u32;
        if score > 0 {
            scored.push(Suggestion {
                skill: skill.name.clone(),
                triggers_matched,
                tags_matched,
                confidence: confidence_for(score),
                source_path: skill.source_path.clone(),
                score,
            });
        }
    }

    scored.sort_by(|a, b| b.score.cmp(&a.score));

    if let Some(cw) = codeword_hit(task) {
        let already = scored.iter().any(|s| s.skill == "mneme-codewords");
        if !already {
            if let Some(cw_skill) = skills.iter().find(|s| s.name == "mneme-codewords") {
                scored.push(Suggestion {
                    skill: "mneme-codewords".to_string(),
                    triggers_matched: vec![cw.to_string()],
                    tags_matched: Vec::new(),
                    confidence: Confidence::Low,
                    source_path: cw_skill.source_path.clone(),
                    score: 1,
                });
            }
        }
    }

    scored.truncate(limit);
    scored
}

/// Short human-readable "why this skill?" string.
pub fn reason_for(s: &Suggestion) -> String {
    if !s.triggers_matched.is_empty() {
        format!("matched trigger(s): {}", s.triggers_matched.join(", "))
    } else if !s.tags_matched.is_empty() {
        format!("tag match: {}", s.tags_matched.join(", "))
    } else {
        "codeword match".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_slice_handles_lf_and_crlf() {
        let lf = "---\nname: foo\n---\n# body\n";
        assert_eq!(slice_frontmatter(lf), Some("name: foo\n"));
        let crlf = "---\r\nname: foo\r\n---\r\n# body\r\n";
        assert_eq!(slice_frontmatter(crlf), Some("name: foo\r\n"));
    }

    #[test]
    fn parses_inline_array() {
        let block = "name: demo\ntriggers: [a, \"b c\", d]\n";
        let kv = parse_tiny_yaml(block);
        let (_, v) = kv.iter().find(|(k, _)| k == "triggers").unwrap();
        assert_eq!(value_as_array(v), vec!["a", "b c", "d"]);
    }

    #[test]
    fn parses_block_array() {
        let block = "name: demo\ntriggers:\n  - a\n  - b\n  - c\n";
        let kv = parse_tiny_yaml(block);
        let (_, v) = kv.iter().find(|(k, _)| k == "triggers").unwrap();
        assert_eq!(value_as_array(v), vec!["a", "b", "c"]);
    }

    #[test]
    fn tokenize_drops_stopwords_and_short() {
        let t = tokenize("I need to debug a test");
        assert!(t.contains("debug"));
        assert!(t.contains("test"));
        assert!(!t.contains("a"));
        assert!(!t.contains("to"));
    }

    #[test]
    fn trigger_matches_whole_token() {
        let tokens = tokenize("fix a bug in the parser");
        assert!(trigger_matches("bug", "fix a bug in the parser", &tokens));
        assert!(!trigger_matches("art", "fix a bug in the parser", &tokens));
    }

    #[test]
    fn trigger_matches_multi_word_substring() {
        let tokens = tokenize("help me with system design");
        assert!(trigger_matches(
            "system design",
            "help me with system design",
            &tokens,
        ));
    }

    #[test]
    fn codeword_hit_respects_boundaries() {
        assert_eq!(codeword_hit("let's do a coldstart please"), Some("coldstart"));
        assert_eq!(codeword_hit("coldstartup procedure"), None);
    }

    #[test]
    fn confidence_buckets() {
        assert_eq!(confidence_for(0), Confidence::Low);
        assert_eq!(confidence_for(1), Confidence::Medium);
        assert_eq!(confidence_for(2), Confidence::Medium);
        assert_eq!(confidence_for(3), Confidence::High);
        assert_eq!(confidence_for(10), Confidence::High);
    }
}
