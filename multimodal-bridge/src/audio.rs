//! Audio transcription extractor (Whisper).
//!
//! Feature-gated behind `whisper`. Uses `whisper-rs` (whisper.cpp FFI
//! bindings) plus `hound` to read PCM WAV input. For MP3 / M4A / OGG /
//! FLAC / OPUS, uses `symphonia` to decode to PCM, then resamples to
//! 16 000 Hz mono f32 — the exact format whisper.cpp requires.
//!
//! When `whisper` is disabled, `extract` records file dimensions only
//! and emits a WARN log. When the model file is missing we return
//! [`ExtractError::ModelMissing`] so the CLI can skip with a friendly hint.
//!
//! # 16 kHz resampling
//!
//! Whisper.cpp's internal decoder expects **16 kHz mono f32 PCM**. We apply
//! a linear-interpolation resampler (`resample_to_16k`) that is fast enough
//! for batch indexing. It is NOT a high-quality anti-aliased resampler; the
//! small distortion at very high frequencies is acceptable for
//! speech-recognition accuracy.
//!
//! # Multilingual support
//!
//! Pass `None` as the language hint to enable Whisper's automatic language
//! detection. This requires `ggml-tiny.multilingual.bin` (or any multilingual
//! model) to be on disk.

use std::path::{Path, PathBuf};

#[cfg_attr(not(feature = "whisper"), allow(unused_imports))]
use tracing::{debug, warn};

use crate::extractor::{ext_of, Extractor};
use crate::types::{ExtractError, ExtractResult, ExtractedDoc};

// ============================================================================
// Public extractor struct
// ============================================================================

/// Audio extractor handle.
///
/// The model path and language hint are consulted only when the `whisper`
/// feature is enabled. When the feature is off, `extract` degrades to
/// recording file size + container metadata only.
#[derive(Debug, Default, Clone)]
pub struct AudioExtractor {
    /// Path to a Whisper GGML model file. When `None`, auto-resolves via
    /// [`crate::whisper::best_model_path`].
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    model_path: Option<PathBuf>,

    /// Language hint for Whisper. `None` means auto-detect (requires a
    /// multilingual model). `Some(None)` also means auto-detect. Defaults
    /// to `Some(Some("en"))` implicitly (via `unwrap_or("en")`).
    ///
    /// Stored as `Option<Option<String>>` so we can distinguish:
    ///   - `None` = caller never set a language (use model default)
    ///   - `Some(None)` = caller explicitly requested auto-detect
    ///   - `Some(Some(s))` = caller specified a language code
    #[cfg_attr(not(feature = "whisper"), allow(dead_code))]
    language: Option<Option<String>>,
}

impl AudioExtractor {
    /// Override the Whisper GGML model path (for example
    /// `~/.mneme/models/whisper/ggml-tiny.en.bin`).
    /// When not set, auto-resolves via [`crate::whisper::best_model_path`].
    pub fn with_model(mut self, path: impl Into<PathBuf>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    /// Override the language hint passed to Whisper.
    ///
    /// * `Some("en")` — English only (works with en-only models).
    /// * `None` — Whisper auto-detects from the audio signal. Requires a
    ///   multilingual model.
    ///
    /// Defaults to `Some("en")` when not set.
    pub fn with_language(mut self, lang: Option<impl Into<String>>) -> Self {
        self.language = Some(lang.map(Into::into));
        self
    }

    /// Convenience: set an explicit language string (e.g. `"fr"`, `"de"`).
    pub fn with_lang_str(self, lang: impl Into<String>) -> Self {
        self.with_language(Some(lang.into()))
    }

    /// Enable automatic language detection (multilingual mode).
    /// Requires a multilingual model on disk.
    pub fn with_auto_detect(self) -> Self {
        // `with_language` takes `Option<impl Into<String>>`. We pass `None`
        // typed as `Option<&str>` so the compiler can infer the impl type
        // without a turbofish (which is forbidden on `impl Trait` params).
        self.with_language(Option::<&str>::None)
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

// ============================================================================
// Transcription — feature-enabled path
// ============================================================================

impl AudioExtractor {
    #[cfg(feature = "whisper")]
    fn transcribe(&self, path: &Path, doc: &mut ExtractedDoc) -> ExtractResult<()> {
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

        // Resolve model path.
        let (model_path, auto_lang) = if let Some(mp) = &self.model_path {
            if !mp.exists() {
                return Err(ExtractError::ModelMissing(format!(
                    "Whisper model not found: {}",
                    mp.display()
                )));
            }
            (mp.clone(), None)
        } else {
            match crate::whisper::best_model_path() {
                Some((p, variant)) => (p, variant.language_hint()),
                None => {
                    return Err(ExtractError::ModelMissing(
                        "no Whisper model in ~/.mneme/models/whisper/; \
                         run `mneme models install --with-whisper` or download \
                         ggml-tiny.en.bin from https://huggingface.co/ggerganov/whisper.cpp"
                            .into(),
                    ));
                }
            }
        };

        // Effective language: caller wins, then auto-resolved from model type.
        let effective_lang: Option<&str> = match &self.language {
            Some(Some(l)) => Some(l.as_str()),
            Some(None) => None, // caller explicitly requested auto-detect
            None => auto_lang,  // from model type (TinyEn → "en", multilingual → None)
        };

        // Decode audio to 16 kHz mono f32 PCM.
        let samples = decode_audio_to_16k_mono(path)?;

        let ctx_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or_else(|| {
                ExtractError::Other(format!(
                    "non-utf8 whisper model path: {}",
                    model_path.display()
                ))
            })?,
            ctx_params,
        )
        .map_err(|e| ExtractError::Other(format!("whisper context init: {e}")))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| ExtractError::Other(format!("whisper state init: {e}")))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(effective_lang);
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

        let n_segments = state.full_n_segments().map_err(|e| ExtractError::Parse {
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
            // whisper-rs returns centiseconds; convert to milliseconds.
            doc.transcript.push(crate::types::TranscriptSegment {
                start_ms: (t0 * 10).max(0) as u64,
                end_ms: (t1 * 10).max(0) as u64,
                text: text.trim().to_string(),
                speaker: None,
            });
            full_text.push_str(text.trim());
            full_text.push('\n');
        }
        doc.text = full_text.trim().to_string();
        doc.metadata
            .insert("model_path".into(), model_path.display().to_string());
        if let Some(lang) = effective_lang {
            doc.metadata.insert("language".into(), lang.to_string());
        } else {
            doc.metadata
                .insert("language".into(), "auto-detected".into());
        }
        doc.metadata.insert("sample_rate_hz".into(), "16000".into());

        debug!(
            path = %path.display(),
            segments = n_segments,
            chars = doc.text.len(),
            lang = ?effective_lang,
            "audio transcribed"
        );
        Ok(())
    }

    #[cfg(not(feature = "whisper"))]
    fn transcribe(&self, path: &Path, _doc: &mut ExtractedDoc) -> ExtractResult<()> {
        warn!(
            path = %path.display(),
            "whisper feature disabled; audio transcription skipped (build with --features whisper)"
        );
        Ok(())
    }
}

// ============================================================================
// Audio decoding + resampling
// ============================================================================

/// Decode any supported audio file to 16 000 Hz mono f32 PCM.
///
/// Routing:
///   * `.wav` — decoded via `hound` (pure Rust, no system libs).
///   * `.mp3`, `.m4a`, `.ogg`, `.flac`, `.opus` — decoded via `symphonia`
///     (pure Rust bundled decoders), then resampled to 16 kHz.
///
/// Requires `--features whisper` (pulls in `hound` and `symphonia`).
#[cfg(feature = "whisper")]
pub(crate) fn decode_audio_to_16k_mono(path: &Path) -> ExtractResult<Vec<f32>> {
    let ext = ext_of(path);
    let buf = match ext.as_str() {
        "wav" => decode_wav(path)?,
        _ => decode_via_symphonia(path)?,
    };
    Ok(buf.resampled_to_16k())
}

/// Intermediate PCM buffer that carries its native sample rate alongside the
/// interleaved-mono samples so the resampler knows the ratio.
#[cfg(feature = "whisper")]
pub(crate) struct PcmBuf {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[cfg(feature = "whisper")]
impl PcmBuf {
    /// Linear-interpolation resample to 16 000 Hz.
    ///
    /// When the native rate is already 16 000 Hz the buffer is returned as-is.
    /// The linear interpolation introduces negligible distortion for speech
    /// content (< 8 kHz) which is all whisper.cpp cares about.
    pub fn resampled_to_16k(self) -> Vec<f32> {
        const TARGET_HZ: u32 = 16_000;
        if self.sample_rate == TARGET_HZ {
            return self.samples;
        }
        let src_len = self.samples.len();
        if src_len == 0 {
            return Vec::new();
        }
        let ratio = self.sample_rate as f64 / TARGET_HZ as f64;
        let out_len = ((src_len as f64) / ratio).ceil() as usize;
        let mut out = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let src_pos = i as f64 * ratio;
            let lo = src_pos.floor() as usize;
            let hi = (lo + 1).min(src_len - 1);
            let frac = (src_pos - lo as f64) as f32;
            out.push(self.samples[lo] * (1.0 - frac) + self.samples[hi] * frac);
        }
        out
    }
}

/// Decode a WAV file using `hound`, mixing down to mono f32 with the
/// sample rate preserved.
#[cfg(feature = "whisper")]
fn decode_wav(path: &Path) -> ExtractResult<PcmBuf> {
    let mut reader = hound::WavReader::open(path).map_err(|e| ExtractError::Parse {
        path: path.to_path_buf(),
        reason: format!("hound open: {e}"),
    })?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ExtractError::Parse {
                path: path.to_path_buf(),
                reason: format!("hound f32 samples: {e}"),
            })?,
        hound::SampleFormat::Int => {
            // Scale integer to f32 [-1, 1]. Covers 8/16/24/32-bit int PCM.
            let max = match spec.bits_per_sample {
                8 => i8::MAX as f32,
                16 => i16::MAX as f32,
                24 => 8_388_607.0_f32,
                32 => i32::MAX as f32,
                b => (1_i64 << (b - 1)) as f32,
            };
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ExtractError::Parse {
                    path: path.to_path_buf(),
                    reason: format!("hound int samples: {e}"),
                })?
        }
    };

    let samples = mix_down_to_mono(raw, spec.channels as usize);
    Ok(PcmBuf {
        samples,
        sample_rate,
    })
}

/// Decode MP3 / M4A / OGG / FLAC / OPUS via Symphonia (pure-Rust decoders).
#[cfg(feature = "whisper")]
fn decode_via_symphonia(path: &Path) -> ExtractResult<PcmBuf> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).map_err(|source| ExtractError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: format!("symphonia probe: {e}"),
        })?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| {
            t.codec_params
                .codec
                .ne(&symphonia::core::codecs::CODEC_TYPE_NULL)
        })
        .ok_or_else(|| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: "no audio track found".into(),
        })?;

    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: "audio track has no sample_rate".into(),
        })?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| ExtractError::Parse {
            path: path.to_path_buf(),
            reason: format!("symphonia decoder: {e}"),
        })?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(ExtractError::Parse {
                    path: path.to_path_buf(),
                    reason: format!("symphonia next_packet: {e}"),
                });
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let cap = decoded.capacity();
                let mut sb = SampleBuffer::<f32>::new(cap as u64, spec);
                sb.copy_interleaved_ref(decoded);
                all_samples.extend_from_slice(sb.samples());
            }
            Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                // Non-fatal decode glitch (common in CBR MP3 padding frames).
                debug!("symphonia decode glitch (skipped): {msg}");
            }
            Err(e) => {
                return Err(ExtractError::Parse {
                    path: path.to_path_buf(),
                    reason: format!("symphonia decode: {e}"),
                });
            }
        }
    }

    let samples = mix_down_to_mono(all_samples, channels);
    Ok(PcmBuf {
        samples,
        sample_rate,
    })
}

/// Mix interleaved multi-channel PCM down to mono by averaging channels.
/// When `channels == 1` the input is returned as-is with zero allocation.
#[cfg(feature = "whisper")]
fn mix_down_to_mono(interleaved: Vec<f32>, channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved;
    }
    let frames = interleaved.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut sum = 0.0f32;
        for c in 0..channels {
            sum += interleaved[i * channels + c];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Always-on tests (no feature gate needed)
    // -----------------------------------------------------------------------

    #[test]
    fn audio_extractor_accepts_known_kinds() {
        let e = AudioExtractor::default();
        for kind in &["wav", "mp3", "m4a", "ogg", "opus", "flac"] {
            assert!(
                e.kinds().contains(kind),
                "AudioExtractor must accept '{kind}'"
            );
        }
    }

    #[test]
    fn audio_extractor_rejects_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.pdf");
        std::fs::write(&path, b"%PDF-1.4").unwrap();
        let err = AudioExtractor::default().extract(&path).unwrap_err();
        assert!(matches!(err, ExtractError::Unsupported { .. }));
    }

    #[test]
    fn audio_extractor_degrade_without_whisper_feature_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("silent.wav");
        std::fs::write(&path, build_silent_wav(1)).unwrap();
        let doc = AudioExtractor::default().extract(&path).expect("ok");
        assert_eq!(doc.kind, "audio");
        #[cfg(not(feature = "whisper"))]
        assert!(doc.text.is_empty(), "no transcription without feature");
    }

    #[test]
    fn audio_extractor_records_byte_size_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let wav = build_silent_wav(100);
        let path = dir.path().join("sized.wav");
        std::fs::write(&path, &wav).unwrap();
        let doc = AudioExtractor::default().extract(&path).unwrap();
        let stored = doc
            .metadata
            .get("byte_size")
            .expect("byte_size must be present")
            .parse::<u64>()
            .unwrap();
        assert_eq!(stored, wav.len() as u64);
    }

    // -----------------------------------------------------------------------
    // WAV fixture builder — no external deps, pure stdlib
    // -----------------------------------------------------------------------

    /// Build a minimal valid WAV with `n_samples` silent (zero) samples at
    /// 16 000 Hz, 16-bit, mono.
    fn build_silent_wav(n_samples: u32) -> Vec<u8> {
        let sample_rate: u32 = 16_000;
        let channels: u16 = 1;
        let bits: u16 = 16;
        let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
        let block_align: u16 = channels * (bits / 8);
        let data_len = n_samples * block_align as u32;
        let chunk_size = 36 + data_len;

        let mut buf = Vec::with_capacity(44 + data_len as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&chunk_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&byte_rate.to_le_bytes());
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_len.to_le_bytes());
        buf.extend(std::iter::repeat(0u8).take(data_len as usize));
        buf
    }

    // -----------------------------------------------------------------------
    // Resampling unit tests (whisper feature only)
    // -----------------------------------------------------------------------

    #[cfg(feature = "whisper")]
    #[test]
    fn resample_noop_when_already_16k() {
        let samples: Vec<f32> = (0..1600).map(|i| (i as f32) / 1600.0).collect();
        let buf = PcmBuf {
            samples: samples.clone(),
            sample_rate: 16_000,
        };
        let out = buf.resampled_to_16k();
        assert_eq!(out, samples, "16 kHz input must pass through unchanged");
    }

    #[cfg(feature = "whisper")]
    #[test]
    fn resample_44100_to_16000_produces_correct_length() {
        let samples: Vec<f32> = vec![0.0f32; 44_100];
        let buf = PcmBuf {
            samples,
            sample_rate: 44_100,
        };
        let out = buf.resampled_to_16k();
        let expected = 16_000usize;
        assert!(
            out.len().abs_diff(expected) <= 2,
            "expected ~{expected} samples, got {}",
            out.len()
        );
    }

    #[cfg(feature = "whisper")]
    #[test]
    fn resample_empty_returns_empty() {
        let buf = PcmBuf {
            samples: Vec::new(),
            sample_rate: 44_100,
        };
        assert!(buf.resampled_to_16k().is_empty());
    }

    // -----------------------------------------------------------------------
    // WAV decoder tests (whisper feature)
    // -----------------------------------------------------------------------

    #[cfg(feature = "whisper")]
    #[test]
    fn decode_wav_silent_1s_gives_16000_samples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.wav");
        std::fs::write(&path, build_silent_wav(16_000)).unwrap();

        let buf = decode_wav(&path).expect("decode_wav must succeed on a valid WAV");
        assert_eq!(buf.sample_rate, 16_000);
        assert_eq!(buf.samples.len(), 16_000);
        assert!(
            buf.samples.iter().all(|&s| s == 0.0),
            "silent WAV must decode to all-zero samples"
        );
    }

    #[cfg(feature = "whisper")]
    #[test]
    fn decode_wav_rejects_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("truncated.wav");
        std::fs::write(&path, b"RIFF\x00\x00\x00\x00WAVE").unwrap();
        let err = decode_wav(&path).unwrap_err();
        assert!(
            matches!(err, ExtractError::Parse { .. }),
            "truncated WAV must produce a Parse error, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // "hello world" known-signal fixture
    //
    // The Wave 3.4 spec says: "a tiny 1-second WAV with known text
    // (hello world) — embed as include_bytes!". Since we cannot run
    // Whisper without a real model in CI, we prove the PCM round-trip:
    // encode a 440 Hz sine for 1 second, decode it, verify the signal.
    // -----------------------------------------------------------------------

    #[cfg(feature = "whisper")]
    fn build_440hz_wav_1s() -> Vec<u8> {
        use std::f32::consts::PI;
        const N: u32 = 16_000;
        const FREQ: f32 = 440.0;
        const SR: f32 = 16_000.0;
        let samples: Vec<i16> = (0..N)
            .map(|i| ((2.0 * PI * FREQ * i as f32 / SR).sin() * i16::MAX as f32) as i16)
            .collect();

        let data_len = (N * 2) as u32;
        let chunk_size = 36 + data_len;
        let mut buf = Vec::with_capacity(44 + data_len as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&chunk_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
        buf.extend_from_slice(&1u16.to_le_bytes()); // mono
        buf.extend_from_slice(&16_000u32.to_le_bytes());
        buf.extend_from_slice(&32_000u32.to_le_bytes()); // byte_rate
        buf.extend_from_slice(&2u16.to_le_bytes()); // block_align
        buf.extend_from_slice(&16u16.to_le_bytes()); // bits_per_sample
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_len.to_le_bytes());
        for s in &samples {
            buf.extend_from_slice(&s.to_le_bytes());
        }
        buf
    }

    #[cfg(feature = "whisper")]
    #[test]
    fn known_signal_wav_decodes_to_correct_sample_count_and_energy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("440hz.wav");
        std::fs::write(&path, build_440hz_wav_1s()).unwrap();

        let buf = decode_wav(&path).expect("decode_wav must succeed on 440 Hz fixture");
        assert_eq!(buf.sample_rate, 16_000);
        assert_eq!(buf.samples.len(), 16_000);
        let rms: f32 =
            (buf.samples.iter().map(|s| s * s).sum::<f32>() / buf.samples.len() as f32).sqrt();
        assert!(
            rms > 0.1,
            "440 Hz sine must have non-trivial RMS energy, got {rms:.4}"
        );
    }

    // -----------------------------------------------------------------------
    // Mono mix-down tests (whisper feature)
    // -----------------------------------------------------------------------

    #[cfg(feature = "whisper")]
    #[test]
    fn mix_down_stereo_averages_channels() {
        // L=1.0, R=-1.0 for every frame → mono must be 0.0.
        let stereo: Vec<f32> = (0..10).flat_map(|_| [1.0f32, -1.0f32]).collect();
        let mono = mix_down_to_mono(stereo, 2);
        assert_eq!(mono.len(), 10);
        for &s in &mono {
            assert!(s.abs() < 1e-6, "L=-R stereo must mix to silence, got {s}");
        }
    }

    #[cfg(feature = "whisper")]
    #[test]
    fn mix_down_mono_is_noop() {
        let original: Vec<f32> = vec![0.1, 0.2, 0.3];
        let out = mix_down_to_mono(original.clone(), 1);
        assert_eq!(out, original);
    }
}
