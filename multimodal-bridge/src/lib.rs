//! Pure-Rust multimodal extraction for mneme.
//!
//! The crate exposes an [`Extractor`](crate::extractor::Extractor) trait
//! plus five built-in implementations:
//!
//! | Module | Kinds | Cargo feature |
//! |---|---|---|
//! | [`pdf`] | `.pdf` | *(always on; pure Rust via `pdf-extract`)* |
//! | [`markdown`] | `.md`, `.markdown`, … | *(always on; pure Rust via `pulldown-cmark`)* |
//! | [`image`] | `.png`, `.jpg`, … | OCR behind `tesseract` |
//! | [`audio`] | `.wav`, … | Transcription behind `whisper` |
//! | [`video`] | `.mp4`, … | Frame sampling behind `ffmpeg` |
//!
//! Callers typically construct [`Registry::default_wired`] and feed it
//! paths from the project walker. Every extractor's failure mode is a
//! typed [`types::ExtractError`]; the CLI path at
//! `cli::commands::graphify` converts these into log-and-skip behaviour.
//!
//! Prior to v0.2 mneme spawned a Python sidecar (`workers/multimodal/`)
//! and proxied length-prefixed msgpack through this crate. That sidecar
//! is gone; this crate is now the whole story.

#![warn(missing_debug_implementations)]

pub mod audio;
pub mod extractor;
pub mod image;
pub mod markdown;
pub mod pdf;
pub mod registry;
pub mod types;
pub mod video;

pub use extractor::Extractor;
pub use registry::Registry;
pub use types::{ExtractError, ExtractResult, ExtractedDoc, PageText, TranscriptSegment};

/// Canonical extractor version. Written to `media.extractor_version`.
pub const VERSION: &str = concat!("mneme-multimodal@", env!("CARGO_PKG_VERSION"));

/// True iff the binary was compiled with the `tesseract` Cargo feature.
/// When `false`, image extractors only emit width/height/EXIF and the
/// per-page text from PDFs/markdown is the only "real" multimodal text
/// the build captures. The CLI's `mneme build` summary uses this to
/// qualify the misleading `pages/sec` figure (audit fix K14): without
/// OCR a 4,000 pages/sec rate is dimensions-only, not real OCR
/// throughput.
///
/// **Bug B-1+ (2026-05-02): prefer [`ocr_runtime_available()`] in
/// new code.** As of v0.3.3 the multimodal worker also tries a
/// runtime shellout to `tesseract.exe` when the compile-time feature
/// is OFF — see `image::locate_tesseract_exe`. So `OCR_ENABLED` is
/// strictly weaker than reality: it's `true` only for FFI-built
/// binaries, but OCR ALSO runs when this is `false` if the user has
/// `tesseract` on PATH or at `C:\Program Files\Tesseract-OCR\`.
pub const OCR_ENABLED: bool = cfg!(feature = "tesseract");

/// Bug B-1+ (2026-05-02): runtime check for OCR availability.
///
/// Returns `true` when EITHER:
///   - the binary was compiled with `--features tesseract` (FFI), OR
///   - `tesseract.exe` is reachable at runtime (PATH probe + the
///     fixed UB-Mannheim Windows install path).
///
/// CLI consumers (`mneme build` summary) should use this instead of
/// the bare [`OCR_ENABLED`] constant so the user-facing summary
/// reflects what mneme will ACTUALLY do, not what it was compiled
/// with. Cheap (~10ms cold, cached). Safe to call from sync code.
pub fn ocr_runtime_available() -> bool {
    if OCR_ENABLED {
        return true;
    }
    crate::image::locate_tesseract_exe().is_some()
}
