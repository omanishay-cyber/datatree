//! The [`Extractor`] trait and dispatch helpers.
//!
//! Every extractor implementation lives in its own sibling module. The
//! trait here is the seam; `lib.rs` wires the default registry.

use std::path::Path;

use crate::types::{ExtractResult, ExtractedDoc};

/// Single-method trait implemented by PDF, Image, Audio, Video, and
/// Markdown extractors. Impls must be thread-safe (`Send + Sync`) so
/// `cli graphify` can fan out over a project with tokio tasks.
pub trait Extractor: Send + Sync {
    /// File-extension suffixes (lowercase, no leading dot) this extractor
    /// handles. Checked by [`crate::Registry::find`].
    fn kinds(&self) -> &[&'static str];

    /// Run extraction. On error the caller logs + skips the file; there
    /// is no retry policy inside the trait.
    fn extract(&self, path: &Path) -> ExtractResult<ExtractedDoc>;
}

/// Small helper: return the lowercase extension of `path` without the
/// leading dot, or `""` if the path has no extension.
pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}
