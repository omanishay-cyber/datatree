//! Audio transcription extractor (Whisper).
//!
//! Feature-gated behind `whisper`. Uses `whisper-rs` (whisper.cpp
//! bindings) plus `hound` to read PCM WAV input into the `f32` buffer
//! whisper.cpp expects. Other audio containers should first be
//! transcoded via the video extractor's ffmpeg path → WAV.
//!
//! When `whisper` is disabled, `extract` records file dimensions only
//! and emits a WARN log. When the model file is missing we return
//! [`ExtractError::ModelMissing`] so the CLI can skip with a friendly
//! hint.

use std::path::{Path, PathBuf};

#[cfg_attr(not(feature = "whisper"), allow(unused_imports))]
use tracing::{debug, warn};

use crate::extractor::{ext_of, Extractor};
use crate::types::{ExtractError, ExtractResult, ExtractedDoc};

/// Audio extractor handle. The model path is consulted only when the
/// `whisper` feature is enabled.
#[derive(Debug, Default, Clone)]
pub struct AudioExtractor {
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    model_path: Option<PathBuf>,
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    language: Option<String>,
}

impl AudioExtractor {
    /// Override the Whisper GGML model path (for example
    /// `~/.mneme/models/whisper/ggml-base.en.bin`).
    pub fn with_model(mut self, path: impl Into<PathBuf>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    /// Override the language hint passed to Whisper. Defaults to `"en"`.
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }
}

impl Extractor for AudioExtractor {
    fn kinds(&self) -> &[&'static str] {
        &["wav", "flac", "mp3", "m4a", "ogg", "opus"]
    }

    fn extract(&self, path: &Path) -> ExtractResult<ExtractedDoc> {
        let ext = ext_of(path);
        if !self.kinds().contains(&ext.as_str()) {
            return Err(ExtractError::Unsupported {
                path: path.to_path_buf(),
                kind: ext,
            });
        }
        // File-size probe is always valid.
        let meta = std::fs::metadata(path).map_err(|source| ExtractError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut doc = ExtractedDoc::empty("audio", path);
        doc.metadata
            .insert("byte_size".into(), meta.len().to_string());
        doc.metadata.insert("container".into(), ext.clone());

        self.transcribe(path, &mut doc)?;
        Ok(doc)
    }
}

impl AudioExtractor {
    #[cfg(feature = "whisper")]
    fn transcribe(&self, path: &Path, doc: &mut ExtractedDoc) -> ExtractResult<()> {
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

        let ext = ext_of(path);
        if ext != "wav" {
            // Only WAV is decoded natively. Other containers must be
            // converted via the video extractor's ffmpeg path first.
            return Err(ExtractError::FeatureDisabled {
                feature: "whisper",
                what: format!(
                    "container '{ext}' requires ffmpeg transcode to WAV; pipe through VideoExtractor"
                ),
            });
        }

        let model = self
            .model_path
            .as_ref()
            .ok_or_else(|| ExtractError::ModelMissing("no Whisper model path configured".into()))?;
        if !model.exists() {
            return Err(ExtractError::ModelMissing(format!(
                "Whisper model not found: {}",
                model.display()
            )));
        }

        let samples = read_wav_as_f32_mono(path)?;

        let ctx_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model.to_str().ok_or_else(|| {
                ExtractError::Other(format!("non-utf8 whisper model path: {}", model.display()))
            })?,
            ctx_params,
        )
        .map_err(|e| ExtractError::Other(format!("whisper context init: {e}")))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| ExtractError::Other(format!("whisper state init: {e}")))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let lang = self.language.as_deref().unwrap_or("en");
        params.set_language(Some(lang));
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, &samples)
            .map_err(|e| ExtractError::Parse {
                path: path.to_path_buf(),
                reason: format!("whisper decode: {e}"),
            })?;

        let n_segments = state
            .full_n_segments()
            .map_err(|e| ExtractError::Parse {
                path: path.to_path_buf(),
                reason: format!("whisper n_segments: {e}"),
            })?;
        let mut full_text = String::new();
        for i in 0..n_segments {
            let text = state
                .full_get_segment_text(i)
                .map_err(|e| ExtractError::Parse {
                    path: path.to_path_buf(),
                    reason: format!("whisper segment_text[{i}]: {e}"),
                })?;
            let t0 = state.full_get_segment_t0(i).unwrap_or(0);
            let t1 = state.full_get_segment_t1(i).unwrap_or(0);
            // whisper-rs returns centiseconds; convert to ms.
            doc.transcript.push(crate::types::TranscriptSegment {
                start_ms: (t0 * 10).max(0) as u64,
                end_ms: (t1 * 10).max(0) as u64,
                text: text.trim().to_string(),
                speaker: None,
            });
            full_text.push_str(&text);
            full_text.push('\n');
        }
        doc.text = full_text.trim().to_string();
        doc.metadata
            .insert("model_path".into(), model.display().to_string());
        debug!(
            path = %path.display(),
            segments = n_segments,
            chars = doc.text.len(),
            "audio transcribed"
        );
        Ok(())
    }

    #[cfg(not(feature = "whisper"))]
    fn transcribe(&self, path: &Path, _doc: &mut ExtractedDoc) -> ExtractResult<()> {
        warn!(
            path = %path.display(),
            "whisper feature disabled; audio transcription skipped"
        );
        Ok(())
    }
}

#[cfg(feature = "whisper")]
fn read_wav_as_f32_mono(path: &Path) -> ExtractResult<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).map_err(|e| ExtractError::Parse {
        path: path.to_path_buf(),
        reason: format!("hound open: {e}"),
    })?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ExtractError::Parse {
                path: path.to_path_buf(),
                reason: format!("hound f32 samples: {e}"),
            })?,
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / i32::MAX as f32))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ExtractError::Parse {
                path: path.to_path_buf(),
                reason: format!("hound int samples: {e}"),
            })?,
    };
    // Mix down to mono if needed.
    if spec.channels <= 1 {
        return Ok(samples);
    }
    let ch = spec.channels as usize;
    let frames = samples.len() / ch;
    let mut mono = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut sum = 0.0f32;
        for c in 0..ch {
            sum += samples[i * ch + c];
        }
        mono.push(sum / ch as f32);
    }
    Ok(mono)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_extractor_accepts_known_kinds() {
        let e = AudioExtractor::default();
        assert!(e.kinds().contains(&"wav"));
        assert!(e.kinds().contains(&"mp3"));
    }

    #[test]
    fn audio_extractor_rejects_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.pdf");
        std::fs::write(&path, b"%PDF-1.4").unwrap();
        let err = AudioExtractor::default().extract(&path).unwrap_err();
        assert!(matches!(err, ExtractError::Unsupported { .. }));
    }

    #[cfg(not(feature = "whisper"))]
    #[test]
    fn audio_extractor_degrades_without_feature() {
        // Write any bytes to a .wav and confirm we still get an
        // ExtractedDoc with text="" (no panic, no error).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("silent.wav");
        std::fs::write(&path, b"RIFF....WAVEfmt ").unwrap();
        let doc = AudioExtractor::default().extract(&path).expect("ok");
        assert_eq!(doc.kind, "audio");
        assert!(doc.text.is_empty(), "no transcription without feature");
    }
}
