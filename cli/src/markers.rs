//! Marker-based idempotent injection for any text-based manifest
//! (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `QODER.md`, …).
//!
//! ## Why markers?
//!
//! From design §25.5 (carried over from graphify): re-running `datatree
//! install` MUST NOT duplicate the datatree section. We solve this with two
//! sentinels:
//!
//! ```text
//! <!-- datatree-start v1.0 sha256=<hex> -->
//! ...generated body...
//! <!-- datatree-end -->
//! ```
//!
//! On install we look for the start sentinel; if present we splice in the
//! new body, otherwise we append. The trailing `sha256=<hex>` covers the
//! body so we can detect user edits between runs and refuse to overwrite
//! without `--force`.
//!
//! The implementation is text-based (not a real Markdown parser) on purpose:
//! it has to round-trip the user's existing CLAUDE.md byte-for-byte outside
//! the marker block. A character-preserving regex pass is the right tool.

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::error::{CliError, CliResult};
use crate::MARKER_VERSION;

/// Opening sentinel prefix. Suffix is `v{MARKER_VERSION} sha256=<hex> -->`.
pub const MARKER_START_PREFIX: &str = "<!-- datatree-start ";
/// Closing sentinel — full literal because there's no version suffix here.
pub const MARKER_END: &str = "<!-- datatree-end -->";

/// One injection block found inside an existing manifest.
#[derive(Debug, Clone)]
pub struct MarkerBlock {
    /// Byte offset of `<` of the start sentinel within the source.
    pub start_byte: usize,
    /// Byte offset just past the `>` of the end sentinel.
    pub end_byte: usize,
    /// Content captured *between* the sentinels (excludes the sentinels
    /// themselves, includes the surrounding newlines).
    pub body: String,
    /// Marker version we found (e.g. `"1.0"`). Differs from
    /// [`crate::MARKER_VERSION`] iff the manifest was written by an older CLI.
    pub version: String,
    /// SHA-256 hex captured from the start sentinel, or `None` if the
    /// sentinel didn't carry one (very old format).
    pub recorded_hash: Option<String>,
}

impl MarkerBlock {
    /// Recompute the SHA-256 of the current body and compare against what
    /// was recorded inside the start sentinel. `Ok(true)` means the body
    /// matches what datatree last wrote (safe to overwrite); `Ok(false)`
    /// means the user edited it.
    pub fn body_matches_recorded_hash(&self) -> bool {
        match &self.recorded_hash {
            None => true, // no hash recorded -> can't tell -> permit
            Some(expected) => &compute_body_hash(&self.body) == expected,
        }
    }
}

/// Stateless helper for marker operations against a manifest's full text.
#[derive(Debug, Default, Clone, Copy)]
pub struct MarkerInjector;

impl MarkerInjector {
    /// Locate the first datatree marker block in `text`. Returns `None` if
    /// neither sentinel is present. Returns `Some` even if the end sentinel
    /// is missing (we'll error in that path during `inject`).
    pub fn find(text: &str) -> Option<MarkerBlock> {
        let start_idx = text.find(MARKER_START_PREFIX)?;

        // Scan forward to end of the start-sentinel comment ("-->").
        let after_prefix = start_idx + MARKER_START_PREFIX.len();
        let close_of_start = text[after_prefix..].find("-->")? + after_prefix + 3;

        let header = &text[start_idx..close_of_start];
        let (version, recorded_hash) = parse_start_header(header);

        // Find the end sentinel after the start-sentinel header.
        let end_idx = text[close_of_start..].find(MARKER_END)? + close_of_start;
        let end_byte = end_idx + MARKER_END.len();

        // Body is everything between the start sentinel close and the end
        // sentinel — including leading/trailing newlines so we can preserve
        // the user's spacing on round-trip.
        let body = text[close_of_start..end_idx].to_string();

        Some(MarkerBlock {
            start_byte: start_idx,
            end_byte,
            body,
            version,
            recorded_hash,
        })
    }

    /// Splice `new_body` into `existing` between datatree sentinels. If no
    /// block exists we append a new one (with two leading newlines for
    /// visual separation). If a block exists and its recorded hash no
    /// longer matches the body inside the file, we refuse unless `force` is
    /// true — that means the user hand-edited the section and we'd lose it.
    ///
    /// `manifest_path` is informational, used only for error messages.
    pub fn inject(
        existing: &str,
        new_body: &str,
        manifest_path: &Path,
        force: bool,
    ) -> CliResult<String> {
        let new_block = render_block(new_body);

        match Self::find(existing) {
            // Existing block; check user edits.
            Some(found) => {
                if !found.body_matches_recorded_hash() && !force {
                    return Err(CliError::MarkerEdited {
                        path: manifest_path.to_path_buf(),
                    });
                }
                let mut out = String::with_capacity(existing.len() + new_block.len());
                out.push_str(&existing[..found.start_byte]);
                out.push_str(&new_block);
                out.push_str(&existing[found.end_byte..]);
                Ok(out)
            }
            // No block; append.
            None => {
                let mut out = String::with_capacity(existing.len() + new_block.len() + 4);
                out.push_str(existing);
                if !existing.ends_with("\n\n") {
                    if existing.ends_with('\n') {
                        out.push('\n');
                    } else if !existing.is_empty() {
                        out.push_str("\n\n");
                    }
                }
                out.push_str(&new_block);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                Ok(out)
            }
        }
    }

    /// Remove the datatree block from `existing`, returning the cleaned
    /// text. If no block is present, returns the input unchanged.
    pub fn remove(existing: &str) -> String {
        let Some(found) = Self::find(existing) else {
            return existing.to_string();
        };
        let mut out = String::with_capacity(existing.len());
        out.push_str(&existing[..found.start_byte]);
        // Trim a single trailing newline that the inject path added so
        // remove() is exact-inverse on a clean install.
        let after = &existing[found.end_byte..];
        out.push_str(after.strip_prefix('\n').unwrap_or(after));
        out
    }
}

/// Render the full sentinel-wrapped block, including the SHA-256 of the
/// *body* (without sentinels) so we can later detect user edits.
fn render_block(body: &str) -> String {
    let normalized = ensure_surrounding_newlines(body);
    let hash = compute_body_hash(&normalized);
    format!(
        "{prefix}v{ver} sha256={hash} -->{body}{end}",
        prefix = MARKER_START_PREFIX,
        ver = MARKER_VERSION,
        hash = hash,
        body = normalized,
        end = MARKER_END,
    )
}

/// Ensures the body starts AND ends with exactly one `\n` so the sentinels
/// always sit on their own lines for readability.
fn ensure_surrounding_newlines(body: &str) -> String {
    let mut s = String::with_capacity(body.len() + 2);
    if !body.starts_with('\n') {
        s.push('\n');
    }
    s.push_str(body);
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// SHA-256 hex of the body, as recorded in the start sentinel.
fn compute_body_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    let digest = hasher.finalize();
    hex_lower(&digest)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Parse the start-sentinel header `<!-- datatree-start v1.0 sha256=abc... -->`.
/// Tolerates missing sha (older format).
fn parse_start_header(header: &str) -> (String, Option<String>) {
    // header looks like: "<!-- datatree-start v1.0 sha256=<hex> -->"
    let inner = header
        .trim_start_matches("<!--")
        .trim_end_matches("-->")
        .trim();
    // Split on whitespace: ["datatree-start", "v1.0", "sha256=<hex>"]
    let mut tokens = inner.split_whitespace();
    let _ = tokens.next(); // "datatree-start"
    let version = tokens
        .next()
        .and_then(|t| t.strip_prefix('v'))
        .unwrap_or(MARKER_VERSION)
        .to_string();
    let mut recorded_hash = None;
    for tok in tokens {
        if let Some(rest) = tok.strip_prefix("sha256=") {
            recorded_hash = Some(rest.to_string());
        }
    }
    (version, recorded_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_path() -> PathBuf {
        PathBuf::from("/tmp/CLAUDE.md")
    }

    #[test]
    fn append_when_no_marker_present() {
        let original = "# My Project\n\nSome existing notes.\n";
        let injected =
            MarkerInjector::inject(original, "Generated by datatree.", &fake_path(), false)
                .unwrap();
        assert!(injected.starts_with("# My Project"));
        assert!(injected.contains(MARKER_START_PREFIX));
        assert!(injected.contains(MARKER_END));
        assert!(injected.contains("Generated by datatree."));
    }

    #[test]
    fn idempotent_double_injection() {
        let original = "# Doc\n";
        let once =
            MarkerInjector::inject(original, "BODY-A", &fake_path(), false).unwrap();
        let twice =
            MarkerInjector::inject(&once, "BODY-A", &fake_path(), false).unwrap();
        // Same body re-applied -> file should be byte-identical.
        assert_eq!(once, twice, "double-inject of same body must be a no-op");
    }

    #[test]
    fn replaces_body_in_place() {
        let original = "# Doc\n";
        let v1 = MarkerInjector::inject(original, "BODY-A", &fake_path(), false).unwrap();
        let v2 = MarkerInjector::inject(&v1, "BODY-B", &fake_path(), false).unwrap();
        assert!(v2.contains("BODY-B"));
        assert!(!v2.contains("BODY-A"));
        // No accidental duplication of sentinels.
        assert_eq!(
            v2.matches(MARKER_START_PREFIX).count(),
            1,
            "exactly one start sentinel must remain"
        );
        assert_eq!(
            v2.matches(MARKER_END).count(),
            1,
            "exactly one end sentinel must remain"
        );
    }

    #[test]
    fn refuses_overwrite_when_user_edited_block() {
        let original = "# Doc\n";
        let mut v1 =
            MarkerInjector::inject(original, "BODY-A", &fake_path(), false).unwrap();
        // Simulate a user editing INSIDE the block.
        v1 = v1.replace("BODY-A", "BODY-A-USER-EDIT");
        let err = MarkerInjector::inject(&v1, "BODY-B", &fake_path(), false);
        assert!(matches!(err, Err(CliError::MarkerEdited { .. })));
    }

    #[test]
    fn force_overrides_user_edits() {
        let original = "# Doc\n";
        let mut v1 =
            MarkerInjector::inject(original, "BODY-A", &fake_path(), false).unwrap();
        v1 = v1.replace("BODY-A", "BODY-A-USER-EDIT");
        let v2 = MarkerInjector::inject(&v1, "BODY-B", &fake_path(), true).unwrap();
        assert!(v2.contains("BODY-B"));
    }

    #[test]
    fn remove_is_inverse_of_clean_install() {
        let original = "# Doc\n\nNotes\n";
        let injected =
            MarkerInjector::inject(original, "BODY", &fake_path(), false).unwrap();
        let removed = MarkerInjector::remove(&injected);
        // remove() should leave us with at least the original content prefix.
        assert!(removed.starts_with("# Doc"));
        assert!(!removed.contains(MARKER_START_PREFIX));
        assert!(!removed.contains(MARKER_END));
    }

    #[test]
    fn parse_header_extracts_version_and_hash() {
        let (v, h) =
            parse_start_header("<!-- datatree-start v1.0 sha256=deadbeef -->");
        assert_eq!(v, "1.0");
        assert_eq!(h.as_deref(), Some("deadbeef"));
    }
}
