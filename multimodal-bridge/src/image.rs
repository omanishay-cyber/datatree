//! Image extractor.
//!
//! * Always records pixel dimensions + format via the pure-Rust `image`
//!   crate.
//! * When the `tesseract` feature is enabled, additionally performs OCR
//!   through libtesseract bindings. The trained-data directory defaults
//!   to the path `TESSDATA_PREFIX` env var points at; callers that want
//!   a different dir construct via [`ImageExtractor::with_tessdata`].
//!
//! When `tesseract` is disabled at build time, `extract` still succeeds
//! but returns an [`ExtractedDoc`] with `text` left empty and a log note
//! at WARN level. This matches the "degrade + log, never fail" policy
//! called out in the P2 blueprint.

use std::path::{Path, PathBuf};

use tracing::{debug, warn};

use crate::extractor::{ext_of, Extractor};
use crate::types::{ExtractError, ExtractResult, ExtractedDoc};

/// Image extractor handle. `tessdata_dir` is only consulted when the
/// `tesseract` feature is enabled.
#[derive(Debug, Default, Clone)]
pub struct ImageExtractor {
    #[cfg_attr(not(feature = "tesseract"), allow(dead_code))]
    tessdata_dir: Option<PathBuf>,
    #[cfg_attr(not(feature = "tesseract"), allow(dead_code))]
    language: Option<String>,
}

impl ImageExtractor {
    /// Override the tesseract trained-data directory. Ignored when the
    /// `tesseract` feature is off.
    pub fn with_tessdata(mut self, dir: impl Into<PathBuf>) -> Self {
        self.tessdata_dir = Some(dir.into());
        self
    }

    /// Override the language code (default "eng"). Ignored without
    /// `tesseract`.
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }
}

impl Extractor for ImageExtractor {
    fn kinds(&self) -> &[&'static str] {
        &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"]
    }

    fn extract(&self, path: &Path) -> ExtractResult<ExtractedDoc> {
        let ext = ext_of(path);
        if !self.kinds().contains(&ext.as_str()) {
            return Err(ExtractError::Unsupported {
                path: path.to_path_buf(),
                kind: ext,
            });
        }

        // Pure-Rust dimension/format probe first. This runs without any
        // C FFI dependency.
        let reader = image::ImageReader::open(path).map_err(|source| ExtractError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let format = reader.format();
        let dims = reader.into_dimensions().map_err(|e| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: format!("image dimensions: {e}"),
        })?;

        let mut doc = ExtractedDoc::empty("image", path);
        doc.metadata.insert("width".into(), dims.0.to_string());
        doc.metadata.insert("height".into(), dims.1.to_string());
        if let Some(f) = format {
            doc.metadata
                .insert("format".into(), format!("{:?}", f).to_lowercase());
        }
        doc.elements.push(serde_json::json!({
            "kind": "image",
            "width": dims.0,
            "height": dims.1,
        }));

        self.run_ocr(path, &mut doc)?;

        debug!(
            path = %path.display(),
            w = dims.0,
            h = dims.1,
            ocr_chars = doc.text.len(),
            "image extracted"
        );
        Ok(doc)
    }
}

impl ImageExtractor {
    #[cfg(feature = "tesseract")]
    fn run_ocr(&self, path: &Path, doc: &mut ExtractedDoc) -> ExtractResult<()> {
        use tesseract::Tesseract;

        let lang: &str = self.language.as_deref().unwrap_or("eng");
        let tessdata: Option<&str> = self.tessdata_dir.as_deref().and_then(|p| p.to_str());

        let tess = match Tesseract::new(tessdata, Some(lang)) {
            Ok(t) => t,
            Err(e) => {
                return Err(ExtractError::ModelMissing(format!(
                    "tesseract init failed (lang={lang}): {e}"
                )));
            }
        };

        let path_str = path.to_str().ok_or_else(|| {
            ExtractError::Other(format!("non-utf8 image path {}", path.display()))
        })?;
        let with_image = tess.set_image(path_str).map_err(|e| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: format!("tesseract set_image: {e}"),
        })?;
        let text = with_image.get_text().map_err(|e| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: format!("tesseract get_text: {e}"),
        })?;
        doc.text = text.trim().to_string();
        if !doc.text.is_empty() {
            doc.pages.push(crate::types::PageText {
                index: 1,
                heading: None,
                text: doc.text.clone(),
            });
        }
        Ok(())
    }

    /// Bug B-1 (2026-05-01): runtime fallback to the system `tesseract`
    /// CLI when the compile-time `tesseract` C-FFI feature is OFF.
    ///
    /// The release pipeline historically built every multimodal binary
    /// without `--features tesseract` (cargo workspace builds do NOT
    /// activate optional features on members), so `OCR_ENABLED` was
    /// always false on shipped binaries even though the installer
    /// winget-installed `UB-Mannheim.TesseractOCR` and put
    /// `tesseract.exe` on PATH. The C-FFI crate links libtesseract at
    /// compile time — having `tesseract.exe` at runtime is irrelevant
    /// to it. So 100% of installs shipped OCR-disabled.
    ///
    /// This fallback shells out to whatever `tesseract` is on PATH:
    ///     `tesseract <image> stdout -l <lang>`
    /// reads stdout as the OCR text, and stuffs it into `doc.text`.
    /// No build-time deps, no C++ toolchain required, works anywhere
    /// the user has run `winget install UB-Mannheim.TesseractOCR` (or
    /// equivalent on macOS/Linux). The compile-time `tesseract`
    /// feature path is preserved above for power users who want the
    /// faster in-process FFI.
    #[cfg(not(feature = "tesseract"))]
    fn run_ocr(&self, path: &Path, doc: &mut ExtractedDoc) -> ExtractResult<()> {
        use std::process::Command;

        let lang: &str = self.language.as_deref().unwrap_or("eng");
        let path_str = path.to_str().ok_or_else(|| {
            ExtractError::Other(format!("non-utf8 image path {}", path.display()))
        })?;

        // First probe: is `tesseract` on PATH at all? Use `--version`
        // because it's cheap and exits 0 quickly. If this fails we
        // fall back to the historical "dimensions only" warning so
        // builds stay green even on machines without OCR.
        let probe = Command::new("tesseract").arg("--version").output();
        let probe_ok = match &probe {
            Ok(out) => out.status.success(),
            Err(_) => false,
        };
        if !probe_ok {
            warn!(
                path = %path.display(),
                "tesseract not on PATH; image OCR skipped (dimensions only). Install via `winget install UB-Mannheim.TesseractOCR` (Windows), `brew install tesseract` (macOS), or `apt-get install tesseract-ocr` (Linux)."
            );
            return Ok(());
        }

        // Real run: `tesseract <image> stdout -l <lang>` writes the
        // OCR text to stdout. We swallow stderr (which tesseract uses
        // for progress) and parse stdout as UTF-8.
        let output = match Command::new("tesseract")
            .arg(path_str)
            .arg("stdout")
            .arg("-l")
            .arg(lang)
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                return Err(ExtractError::Parse {
                    path: path.to_path_buf(),
                    reason: format!("tesseract spawn failed: {e}"),
                });
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ExtractError::Parse {
                path: path.to_path_buf(),
                reason: format!("tesseract exit {:?}: {}", output.status.code(), stderr.trim()),
            });
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        doc.text = text;
        if !doc.text.is_empty() {
            doc.pages.push(crate::types::PageText {
                index: 1,
                heading: None,
                text: doc.text.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smallest valid PNG: 1x1, fully transparent. 67 bytes.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn image_extractor_reads_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.png");
        std::fs::write(&path, TINY_PNG).unwrap();

        let doc = ImageExtractor::default().extract(&path).expect("extract");
        assert_eq!(doc.kind, "image");
        assert_eq!(doc.metadata.get("width").map(String::as_str), Some("1"));
        assert_eq!(doc.metadata.get("height").map(String::as_str), Some("1"));
    }

    #[test]
    fn image_extractor_rejects_non_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not.txt");
        std::fs::write(&path, "nope").unwrap();
        let err = ImageExtractor::default().extract(&path).unwrap_err();
        assert!(matches!(err, ExtractError::Unsupported { .. }));
    }
}
