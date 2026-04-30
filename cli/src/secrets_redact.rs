//! Credential / PII redactor for hook persistence (SEC-2).
//!
//! ## Why this exists
//!
//! Mneme's UserPromptSubmit hook persists every user prompt verbatim into
//! `history.db::turns` so `recall_conversation` and the Step Ledger have a
//! complete record. The flip side is that a user pasting an AWS access key,
//! Slack token, OpenAI key, GitHub PAT, or Anthropic key into a prompt has
//! that secret stored on disk forever.
//!
//! [`redact`] is the single chokepoint that scrubs the most common
//! credential shapes BEFORE they hit the SQLite shard. It is wired in at
//! two layers (defense-in-depth):
//!
//! 1. `commands::inject::run` — the inject hook scrubs the prompt before
//!    handing it to [`crate::hook_writer::HookCtx::write_turn`].
//! 2. `hook_writer::HookCtx::write_turn` — a second pass at the DB write
//!    boundary so any future caller (Stop hook session-end summary,
//!    test fixture, etc.) is also covered.
//!
//! ## Design notes
//!
//! - **Idempotent.** `redact(redact(s)) == redact(s)`. The output marker
//!   `[REDACTED:<kind>]` is shaped so that no source pattern matches it
//!   (no `AKIA`, no `sk-`, no `xox`, no `ghp_` prefix), which means a
//!   second pass is a no-op — verified by a unit test.
//! - **Conservative.** We only redact patterns we are confident about.
//!   False positives in a prompt are still a UX cost (the user's intent
//!   is mangled), so generic 40-char base64 strings are NOT redacted on
//!   their own — we require the surrounding `aws_secret_access_key`
//!   context for the AWS-secret rule.
//! - **No allocation when clean.** When the input contains no matches,
//!   the regex pass returns an unchanged `Cow::Borrowed`, which we then
//!   `into_owned` — still one allocation for the final `String`, but
//!   no per-pattern intermediate strings.
//! - **Compiled once.** All regexes live behind `OnceLock` so the cost
//!   of compilation is paid exactly once per process. Hook processes
//!   are short-lived but the cost still matters when a session fires
//!   thousands of `pre-tool` / `post-tool` events.

use regex::Regex;
use std::sync::OnceLock;

/// Redact common credential / token shapes from `input`.
///
/// Each match is replaced with `[REDACTED:<kind>]` where `<kind>` is one
/// of: `aws_access_key`, `aws_secret_access_key`, `openai_key`,
/// `anthropic_key`, `slack_token`, `github_pat`.
///
/// This function is idempotent: passing already-redacted text through it
/// a second time is a no-op (verified by a unit test).
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    for pat in patterns() {
        out = pat.regex.replace_all(&out, pat.replacement.as_str()).into_owned();
    }
    out
}

struct Pattern {
    regex: Regex,
    replacement: String,
}

fn patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        // Pattern order matters. Most specific prefixes go first so a
        // less-specific rule (e.g. the bare `sk-` OpenAI rule) does NOT
        // swallow a more specific token (e.g. `sk-ant-`) before its own
        // rule runs.
        vec![
            // 1) Anthropic: literal sk-ant- prefix + 20+ [A-Za-z0-9_-].
            //    Must run before the OpenAI rule since both start with
            //    `sk-`.
            Pattern {
                regex: Regex::new(r"\bsk-ant-[A-Za-z0-9_-]{20,}\b").unwrap(),
                replacement: "[REDACTED:anthropic_key]".to_string(),
            },
            // 2) AWS access key: literal AKIA prefix + exactly 16 [A-Z0-9].
            //    Anchor on word boundaries so we don't chop the prefix
            //    off a longer alphanumeric blob.
            Pattern {
                regex: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
                replacement: "[REDACTED:aws_access_key]".to_string(),
            },
            // 3) AWS secret access key: 40 chars of [A-Za-z0-9/+], but
            //    ONLY when surrounded by the assignment context. A naked
            //    40-char blob is too prone to false positives (a sha1
            //    hex digest is 40 chars, so are many base64 chunks).
            //
            //    We capture the prefix (key name + separator) and re-emit
            //    it via $1 so the redacted line still reads sensibly:
            //
            //      aws_secret_access_key=[REDACTED:aws_secret_access_key]
            Pattern {
                regex: Regex::new(
                    r#"(?i)(aws_secret_access_key\s*[:=]\s*"?)[A-Za-z0-9/+]{40}"#,
                )
                .unwrap(),
                replacement: "${1}[REDACTED:aws_secret_access_key]".to_string(),
            },
            // 4) OpenAI: literal sk- prefix + 32 or more [A-Za-z0-9_-].
            //    Real OpenAI keys are 48+ today but older ones were ~32,
            //    so 32 is the safe lower bound.
            //
            //    Runs AFTER the Anthropic rule above so `sk-ant-...`
            //    never matches here.
            Pattern {
                regex: Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}\b").unwrap(),
                replacement: "[REDACTED:openai_key]".to_string(),
            },
            // 5) Slack tokens: xoxb / xoxa / xoxp / xoxr / xoxs prefix.
            Pattern {
                regex: Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").unwrap(),
                replacement: "[REDACTED:slack_token]".to_string(),
            },
            // 6) GitHub personal-access-token / OAuth / app / refresh /
            //    server tokens. Always exactly 36 [A-Za-z0-9] following
            //    the ghX_ prefix.
            Pattern {
                regex: Regex::new(r"\b(?:ghp_|gho_|ghu_|ghs_|ghr_)[A-Za-z0-9]{36}\b").unwrap(),
                replacement: "[REDACTED:github_pat]".to_string(),
            },
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_aws_access_key() {
        let s = "my key is AKIAIOSFODNN7EXAMPLE in the config";
        let out = redact(s);
        assert!(out.contains("[REDACTED:aws_access_key]"));
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn redacts_aws_secret_access_key_in_context() {
        let s = "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let out = redact(s);
        assert!(
            out.contains("[REDACTED:aws_secret_access_key]"),
            "expected redaction marker in: {out}"
        );
        assert!(!out.contains("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"));
    }

    #[test]
    fn redacts_openai_key() {
        let s = "OPENAI_API_KEY=sk-abcdef0123456789abcdef0123456789abcd";
        let out = redact(s);
        assert!(out.contains("[REDACTED:openai_key]"), "got: {out}");
        assert!(!out.contains("sk-abcdef0123456789abcdef0123456789abcd"));
    }

    #[test]
    fn redacts_anthropic_key() {
        let s = "export ANTHROPIC_API_KEY=sk-ant-api03-AAAA_BBBB-CCCC_DDDD-1234567890";
        let out = redact(s);
        assert!(out.contains("[REDACTED:anthropic_key]"), "got: {out}");
        assert!(!out.contains("sk-ant-api03-AAAA_BBBB-CCCC_DDDD-1234567890"));
    }

    #[test]
    fn redacts_slack_bot_token() {
        let s = "slack token: xoxb-1234567890-0987654321-AbCdEfGhIjKlMnOpQrStUvWx";
        let out = redact(s);
        assert!(out.contains("[REDACTED:slack_token]"), "got: {out}");
        assert!(!out.contains("xoxb-1234567890-0987654321-AbCdEfGhIjKlMnOpQrStUvWx"));
    }

    #[test]
    fn redacts_github_pat() {
        let s = "GITHUB_TOKEN=ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789";
        let out = redact(s);
        assert!(out.contains("[REDACTED:github_pat]"), "got: {out}");
        assert!(!out.contains("ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789"));
    }

    #[test]
    fn is_idempotent() {
        let s = "AKIAIOSFODNN7EXAMPLE and sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaa and ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789";
        let once = redact(s);
        let twice = redact(&once);
        assert_eq!(once, twice, "redact() must be idempotent");
    }

    #[test]
    fn leaves_clean_input_alone() {
        let s = "this is just a normal prompt with no secrets in it at all";
        let out = redact(s);
        assert_eq!(out, s);
    }

    #[test]
    fn redacts_multiple_in_one_pass() {
        let s = "key1=AKIAIOSFODNN7EXAMPLE and key2=ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789";
        let out = redact(s);
        assert!(out.contains("[REDACTED:aws_access_key]"));
        assert!(out.contains("[REDACTED:github_pat]"));
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!out.contains("ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789"));
    }
}
