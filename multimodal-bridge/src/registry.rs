//! Pluggable [`Registry`] that dispatches a path to the right
//! [`Extractor`] by its file extension.

use std::path::Path;
use std::sync::Arc;

use tracing::warn;

use crate::extractor::{ext_of, Extractor};
use crate::types::{ExtractError, ExtractResult, ExtractedDoc};

/// Ordered collection of extractors. First match wins.
#[derive(Clone)]
pub struct Registry {
    extractors: Vec<Arc<dyn Extractor>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("extractors", &self.extractors.len())
            .finish()
    }
}

impl Registry {
    /// Empty registry — every `extract` call returns `Unsupported`.
    pub fn empty() -> Self {
        Self {
            extractors: Vec::new(),
        }
    }

    /// The default wiring for mneme:
    ///
    /// * `PdfExtractor` (always on — pure Rust)
    /// * `MarkdownExtractor` (always on — pure Rust)
    /// * `ImageExtractor` (always registered; OCR is feature-gated inside)
    /// * `AudioExtractor` (always registered; transcription feature-gated)
    /// * `VideoExtractor` (always registered; frame sampling feature-gated)
    pub fn default_wired() -> Self {
        let mut r = Self::empty();
        r.push(crate::pdf::PdfExtractor);
        r.push(crate::markdown::MarkdownExtractor);
        r.push(crate::image::ImageExtractor::default());
        r.push(crate::audio::AudioExtractor::default());
        r.push(crate::video::VideoExtractor::default());
        r
    }

    /// Push an extractor onto the tail of the dispatch list.
    pub fn push<E: Extractor + 'static>(&mut self, e: E) {
        self.extractors.push(Arc::new(e));
    }

    /// Find the first extractor whose `kinds()` match `path`. Returns
    /// `None` when no extractor claims the extension.
    pub fn find(&self, path: &Path) -> Option<&Arc<dyn Extractor>> {
        let ext = ext_of(path);
        if ext.is_empty() {
            return None;
        }
        self.extractors
            .iter()
            .find(|e| e.kinds().iter().any(|k| *k == ext))
    }

    /// Dispatch extraction. If no extractor matches we return
    /// [`ExtractError::Unsupported`]; callers should treat this as
    /// "skip, not fatal".
    pub fn extract(&self, path: &Path) -> ExtractResult<ExtractedDoc> {
        match self.find(path) {
            Some(e) => e.extract(path),
            None => Err(ExtractError::Unsupported {
                path: path.to_path_buf(),
                kind: ext_of(path),
            }),
        }
    }

    /// Like [`extract`] but logs-and-skips on failure instead of
    /// bubbling. Returns `None` for any non-fatal case; use this in the
    /// `mneme graphify` walker loop.
    pub fn try_extract(&self, path: &Path) -> Option<ExtractedDoc> {
        match self.extract(path) {
            Ok(d) => Some(d),
            Err(ExtractError::Unsupported { .. }) => None,
            Err(e) => {
                warn!(path = %path.display(), error = %e, "extract skipped");
                None
            }
        }
    }

    /// Every registered extension across every extractor (deduped).
    pub fn known_kinds(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self
            .extractors
            .iter()
            .flat_map(|e| e.kinds().iter().copied())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::default_wired()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_finds_pdf_and_markdown() {
        let r = Registry::default_wired();
        assert!(r.find(Path::new("a.pdf")).is_some());
        assert!(r.find(Path::new("a.md")).is_some());
        assert!(r.find(Path::new("a.png")).is_some());
        assert!(r.find(Path::new("a.mp4")).is_some());
        assert!(r.find(Path::new("a.wav")).is_some());
        assert!(r.find(Path::new("a.unknown")).is_none());
    }

    #[test]
    fn known_kinds_dedupes() {
        let r = Registry::default_wired();
        let k = r.known_kinds();
        let dedup: std::collections::HashSet<_> = k.iter().copied().collect();
        assert_eq!(k.len(), dedup.len());
    }

    #[test]
    fn extract_returns_unsupported_for_unknown_ext() {
        let r = Registry::default_wired();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.xyz");
        std::fs::write(&path, "hi").unwrap();
        let err = r.extract(&path).unwrap_err();
        assert!(matches!(err, ExtractError::Unsupported { .. }));
    }
}
