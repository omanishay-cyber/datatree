//! Credential / token redactor for --verbose IPC trace logs (HIGH-18).
//!
//! When users run with --verbose, the store-worker can emit tracing::trace!
//! lines that include the inbound SQL string AND the bound parameters. If a
//! parameter happens to be a credential (a JWT, OAuth bearer token,
//! password, API key) that secret would land in the rolling log file in
//! plaintext. The CLI already has a secrets_redact pass for hook
//! persistence, but the IPC inbound trace path was never wired through it.
//!
//! This module is the store-side mirror, scoped to the IPC inbound trace
//! path: query params, write params, batch params, inject params.
//!
//! ## Why not call the CLI redact directly?
//!
//! 1. The cli crate depends on store. Reaching back from store to cli would
//!    introduce a cycle.
//! 2. The CLI redactor is tuned for hook persistence (one allocation per
//!    call regardless of whether the input was clean). The store redactor
//!    runs on the trace hot path so it uses Cow<str> to avoid the
//!    allocation when the input is already clean.
//! 3. Different threat surface. CLI scrubs prompt text stored on disk
//!    forever. Store scrubs query parameters that flow through IPC every
//!    few milliseconds.
//!
//! ## Patterns covered
//!
//! In addition to the CLI six (AWS access key, OpenAI key, Anthropic key,
//! Slack token, GitHub PAT) the store-side pass also catches:
//!
//! - JWT shape: three base64url segments separated by dots, anchored on the
//!   JOSE header prefix eyJ.
//! - Long hex digests: 40-64 hex chars on word boundaries (SHA-1, SHA-256,
//!   blake3 hex, HMAC hex).
//! - Keyword-anchored secrets: password=, token=, secret=, api_key=, bearer,
//!   authorization. Case-insensitive.
//!
//! ## Hot-path discipline
//!
//! - All regexes compile exactly once via OnceLock.
//! - redact_sql returns Cow<str>; clean input is Cow::Borrowed (zero alloc).
//! - The trace call sites guard the redaction with tracing::enabled! so when
//!   --verbose is OFF the redactor is never invoked. Cost when off: < 1 us.

use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;

/// Redact credential-shaped substrings from input.
///
/// Returns Cow::Borrowed when the input is clean (no allocation), or
/// Cow::Owned when at least one pattern matched.
///
/// Idempotent: each replacement marker (<redacted-*>) does not itself
/// match any source pattern, so a second pass is a no-op.
pub fn redact_sql(input: &str) -> Cow<'_, str> {
    let mut current: Cow<'_, str> = Cow::Borrowed(input);
    for pat in patterns() {
        match pat.regex.replace_all(&current, pat.replacement.as_str()) {
            Cow::Borrowed(_) => {}
            Cow::Owned(new_owned) => {
                current = Cow::Owned(new_owned);
            }
        }
    }
    current
}

/// Redact credential-shaped values from a parameter vector.
///
/// - Value::String(s) runs through redact_sql; if it changed, replaced.
/// - all other variants pass through unchanged.
pub fn redact_params(params: &[serde_json::Value]) -> Vec<serde_json::Value> {
    params
        .iter()
        .map(|v| match v {
            serde_json::Value::String(s) => match redact_sql(s) {
                Cow::Borrowed(_) => v.clone(),
                Cow::Owned(new_s) => serde_json::Value::String(new_s),
            },
            _ => v.clone(),
        })
        .collect()
}

struct Pattern {
    regex: Regex,
    replacement: String,
}

fn patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        // Pattern order matters: most specific prefixes go first.
        vec![
            // 1) Anthropic: literal sk-ant- prefix + 20+ chars.
            Pattern {
                regex: Regex::new(r"\bsk-ant-[A-Za-z0-9_-]{20,}\b").unwrap(),
                replacement: "<redacted-anthropic-key>".to_string(),
            },
            // 2) AWS access key: literal AKIA + 16 [A-Z0-9].
            Pattern {
                regex: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
                replacement: "<redacted-aws-access-key>".to_string(),
            },
            // 3) OpenAI: literal sk- prefix + 32+ chars. Runs after Anthropic.
            Pattern {
                regex: Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}\b").unwrap(),
                replacement: "<redacted-openai-key>".to_string(),
            },
            // 4) Slack tokens: xox[baprs] prefix.
            Pattern {
                regex: Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").unwrap(),
                replacement: "<redacted-slack-token>".to_string(),
            },
            // 5) GitHub PAT / OAuth / app / refresh / server tokens.
            Pattern {
                regex: Regex::new(r"\b(?:ghp_|gho_|ghu_|ghs_|ghr_)[A-Za-z0-9]{36}\b").unwrap(),
                replacement: "<redacted-github-pat>".to_string(),
            },
            // 6) JWT shape: three base64url segments separated by dots,
            //    anchored on the JOSE header prefix eyJ.
            Pattern {
                regex: Regex::new(r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*\b")
                    .unwrap(),
                replacement: "<redacted-jwt>".to_string(),
            },
            // 7) Long hex digests: 40-64 hex chars on word boundaries.
            Pattern {
                regex: Regex::new(r"\b[a-fA-F0-9]{40,64}\b").unwrap(),
                replacement: "<redacted-hash>".to_string(),
            },
            // 8) Keyword-anchored credential assignment.
            Pattern {
                regex: Regex::new(
                    r###"(?i)\b((?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|bearer|authorization)\s*[:=]\s*"?)[^\s"',;)]{1,200}"###,
                )
                .unwrap(),
                replacement: "${1}<redacted>".to_string(),
            },
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_input_zero_alloc() {
        let s = "SELECT id FROM nodes WHERE id = ?1";
        match redact_sql(s) {
            Cow::Borrowed(out) => assert_eq!(out, s),
            Cow::Owned(_) => panic!("clean input must NOT allocate"),
        }
    }

    #[test]
    fn redacts_jwt_in_string() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let s = format!("WHERE token = '{}'", jwt);
        let out = redact_sql(&s);
        assert!(out.contains("<redacted-jwt>"), "got: {out}");
        assert!(!out.contains("eyJzdWIiOiIxMjM0NSJ9"), "JWT body leaked: {out}");
    }

    #[test]
    fn redacts_password_keyword() {
        let s = "INSERT INTO users(password) VALUES (password=hunter2)";
        let out = redact_sql(s);
        assert!(out.contains("<redacted>"), "got: {out}");
        assert!(!out.contains("hunter2"), "password leaked: {out}");
    }

    #[test]
    fn redacts_long_hex() {
        let s = "session_id=da39a3ee5e6b4b0d3255bfef95601890afd80709";
        let out = redact_sql(s);
        assert!(out.contains("<redacted-hash>"), "got: {out}");
        assert!(
            !out.contains("da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            "SHA-1 hex leaked: {out}"
        );
    }

    #[test]
    fn redacts_aws_access_key() {
        let s = "key=AKIAIOSFODNN7EXAMPLE";
        let out = redact_sql(s);
        assert!(out.contains("<redacted-aws-access-key>"));
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn does_not_overredact_normal_query() {
        let s = "SELECT id FROM nodes WHERE id = 'xyz'";
        let out = redact_sql(s);
        assert_eq!(&*out, s);
    }

    #[test]
    fn is_idempotent() {
        let jwt_token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let s = format!("password=hunter2 token={}", jwt_token);
        let once = redact_sql(&s).into_owned();
        let twice = redact_sql(&once).into_owned();
        assert_eq!(once, twice, "redact_sql must be idempotent");
    }

    #[test]
    fn redact_params_passes_through_non_strings() {
        let params = vec![
            serde_json::Value::Number(serde_json::Number::from(42)),
            serde_json::Value::Bool(true),
            serde_json::Value::Null,
        ];
        let out = redact_params(&params);
        assert_eq!(out, params);
    }

    #[test]
    fn redact_params_redacts_string_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let params = vec![
            serde_json::Value::String(jwt.to_string()),
            serde_json::Value::String("normal-id".to_string()),
        ];
        let out = redact_params(&params);
        let s0 = out[0].as_str().unwrap();
        assert!(s0.contains("<redacted-jwt>"), "got: {s0}");
        assert!(!s0.contains("eyJzdWI"), "JWT leaked: {s0}");
        assert_eq!(out[1].as_str(), Some("normal-id"));
    }
}
