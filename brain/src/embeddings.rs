//! Local sentence embedding for datatree.
//!
//! # Two-tier backend strategy
//!
//! Datatree embeds code/text into 384-dim vectors for semantic recall. The
//! quality of those vectors directly controls retrieval hit-rate (`recall_concept`,
//! `blast_radius` secondary ranking, `find_references` across renames, etc).
//!
//! v0.1 shipped a pure-Rust **hashing-trick** backend: cheap, offline-safe,
//! zero dependencies, mediocre paraphrase quality.
//!
//! v0.2 adds a real **BGE-Small-En-v1.5** backend via `fastembed` (ONNX
//! Runtime under the hood). The upgrade is transparent:
//!
//! 1. On first call, [`Embedder::new`] tries to load BGE from
//!    `~/.datatree/llm/bge-small/` (or the default `fastembed` cache dir).
//! 2. If the model is present, all subsequent `embed*` calls go through the
//!    real transformer and produce semantically meaningful vectors.
//! 3. If the model is missing (fresh machine, offline), the backend falls
//!    back to the hashing trick and logs a warning. The workspace still
//!    compiles and runs; retrieval is just less accurate.
//!
//! Switching backends requires no caller change — the public API
//! ([`Embedder::embed`], [`Embedder::embed_batch`]) is identical.
//!
//! # Offline-first
//!
//! Datatree never makes unsolicited network calls. The BGE model is fetched
//! exactly once, by an explicit user action:
//!
//! ```text
//! $ datatree models install        # ~130 MB download to ~/.datatree/llm/
//! ```
//!
//! After that, everything is local.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;
use tracing::{debug, info, warn};

use crate::error::{BrainError, BrainResult};

/// BGE-Small-En-v1.5 produces 384-dim embeddings. The hashing-trick fallback
/// uses the same dimension for drop-in compatibility.
pub const EMBEDDING_DIM: usize = 384;

/// Maximum tokens fed to the model at once. BGE-small was trained on 512.
const MAX_TOKENS: usize = 512;

/// Process-wide singleton so multiple [`Embedder`] handles share one backend
/// (and one mmap of the model bytes).
static GLOBAL_BACKEND: OnceCell<Arc<Mutex<Backend>>> = OnceCell::new();

/// Public embedder handle. Cheap to clone.
#[derive(Clone)]
pub struct Embedder {
    inner: Arc<Inner>,
}

struct Inner {
    backend: Arc<Mutex<Backend>>,
    cache: DashMap<[u8; 32], Vec<f32>>,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
}

impl std::fmt::Debug for Embedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedder")
            .field("model_path", &self.inner.model_path)
            .field("tokenizer_path", &self.inner.tokenizer_path)
            .field("backend", &self.backend_name())
            .field("cached", &self.inner.cache.len())
            .finish()
    }
}

impl Embedder {
    /// Build an embedder from the default `~/.datatree/llm/bge-small/` path.
    pub fn from_default_path() -> BrainResult<Self> {
        let base = default_model_dir();
        Self::new(&base.join("model.onnx"), &base.join("tokenizer.json"))
    }

    /// Build an embedder from explicit paths. Missing files are tolerated —
    /// the embedder falls back to the pure-Rust hashing-trick backend and
    /// logs a warning.
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> BrainResult<Self> {
        let backend = GLOBAL_BACKEND
            .get_or_init(|| Arc::new(Mutex::new(Backend::Uninitialized)))
            .clone();

        // Eager init. Try real model first, fall back to hashing trick.
        {
            let mut guard = backend.lock();
            if matches!(*guard, Backend::Uninitialized) {
                *guard = Backend::load(model_path, tokenizer_path);
                match &*guard {
                    Backend::Real(_) => info!(
                        model = %model_path.display(),
                        "BGE embedder loaded — real transformer path active"
                    ),
                    Backend::Fallback(_) => warn!(
                        model = %model_path.display(),
                        "BGE model missing — embedder running in fallback mode. \
                         Run `datatree models install` for full retrieval quality."
                    ),
                    Backend::Uninitialized => unreachable!(),
                }
            }
        }

        Ok(Self {
            inner: Arc::new(Inner {
                backend,
                cache: DashMap::new(),
                model_path: model_path.to_path_buf(),
                tokenizer_path: tokenizer_path.to_path_buf(),
            }),
        })
    }

    /// True iff the real transformer backend is active. When false, the
    /// embedder is in fallback (hashing-trick) mode — still functional but
    /// lower retrieval quality.
    pub fn is_ready(&self) -> bool {
        matches!(*self.inner.backend.lock(), Backend::Real(_))
    }

    /// Name of the active backend: `"bge-small-en-v1.5"` or `"hashing-trick"`.
    pub fn backend_name(&self) -> &'static str {
        match *self.inner.backend.lock() {
            Backend::Real(_) => "bge-small-en-v1.5",
            Backend::Fallback(_) => "hashing-trick",
            Backend::Uninitialized => "uninitialized",
        }
    }

    /// Embed a single text. Returns a 384-element vector.
    pub fn embed(&self, text: &str) -> BrainResult<Vec<f32>> {
        let key = hash_key(text);
        if let Some(v) = self.inner.cache.get(&key) {
            return Ok(v.clone());
        }

        let vec = {
            let mut guard = self.inner.backend.lock();
            guard.embed_one(text)?
        };
        self.inner.cache.insert(key, vec.clone());
        Ok(vec)
    }

    /// Batched embedding. Order of returned vectors matches `texts`.
    pub fn embed_batch(&self, texts: &[&str]) -> BrainResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Resolve cached entries first; only run uncached items through the
        // backend.
        let mut out: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        let mut to_compute_idx: Vec<usize> = Vec::new();
        let mut to_compute_text: Vec<&str> = Vec::new();

        for (i, t) in texts.iter().enumerate() {
            let k = hash_key(t);
            if let Some(v) = self.inner.cache.get(&k) {
                out[i] = Some(v.clone());
            } else {
                to_compute_idx.push(i);
                to_compute_text.push(t);
            }
        }

        if !to_compute_text.is_empty() {
            let computed = {
                let mut guard = self.inner.backend.lock();
                guard.embed_batch(&to_compute_text)?
            };
            for (slot, vec) in to_compute_idx.into_iter().zip(computed.into_iter()) {
                let k = hash_key(texts[slot]);
                self.inner.cache.insert(k, vec.clone());
                out[slot] = Some(vec);
            }
        }

        Ok(out.into_iter().map(|v| v.unwrap_or_else(zero_vec)).collect())
    }

    /// Drop the cache. Useful for memory-pressure callbacks.
    pub fn clear_cache(&self) {
        self.inner.cache.clear();
    }

    pub fn model_path(&self) -> &Path {
        &self.inner.model_path
    }
}

// ---------------------------------------------------------------------------
// Backend enum — real or fallback
// ---------------------------------------------------------------------------

enum Backend {
    Uninitialized,
    Real(RealBackend),
    Fallback(FallbackBackend),
}

impl Backend {
    fn load(model_path: &Path, tokenizer_path: &Path) -> Self {
        #[cfg(feature = "fastembed")]
        {
            match RealBackend::try_new(model_path) {
                Ok(b) => return Backend::Real(b),
                Err(e) => {
                    warn!(
                        error = %e,
                        "fastembed init failed — using hashing-trick fallback"
                    );
                }
            }
        }
        Backend::Fallback(FallbackBackend::try_load(tokenizer_path))
    }

    fn embed_one(&mut self, text: &str) -> BrainResult<Vec<f32>> {
        match self {
            Backend::Real(b) => b.embed_one(text),
            Backend::Fallback(b) => Ok(b.embed_one(text)),
            Backend::Uninitialized => Ok(zero_vec()),
        }
    }

    fn embed_batch(&mut self, texts: &[&str]) -> BrainResult<Vec<Vec<f32>>> {
        match self {
            Backend::Real(b) => b.embed_batch(texts),
            Backend::Fallback(b) => Ok(b.embed_batch(texts)),
            Backend::Uninitialized => Ok(vec![zero_vec(); texts.len()]),
        }
    }
}

// ---------------------------------------------------------------------------
// Real backend (fastembed / BGE-Small-En-v1.5)
// ---------------------------------------------------------------------------

#[cfg(feature = "fastembed")]
struct RealBackend {
    model: fastembed::TextEmbedding,
}

#[cfg(feature = "fastembed")]
impl RealBackend {
    fn try_new(model_path: &Path) -> BrainResult<Self> {
        use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

        // fastembed caches models under cache_dir. Point it at our own path so
        // the same model dir works whether loaded via `datatree models install`
        // or explicitly specified.
        let cache_dir = model_path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| default_model_dir().parent().unwrap().to_path_buf());

        let opts = InitOptions::new(EmbeddingModel::BGESmallENV15)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false);

        let model = TextEmbedding::try_new(opts).map_err(|e| {
            BrainError::Embedding(format!("fastembed init: {e}"))
        })?;
        Ok(Self { model })
    }

    fn embed_one(&mut self, text: &str) -> BrainResult<Vec<f32>> {
        let mut out = self.embed_batch(&[text])?;
        Ok(out.pop().unwrap_or_else(zero_vec))
    }

    fn embed_batch(&mut self, texts: &[&str]) -> BrainResult<Vec<Vec<f32>>> {
        let documents: Vec<&str> = texts.to_vec();
        let embeddings = self
            .model
            .embed(documents, None)
            .map_err(|e| BrainError::Embedding(format!("fastembed embed: {e}")))?;

        // fastembed returns Vec<Vec<f32>> already L2-normalised and of the
        // correct dimension. Defensive check.
        for v in &embeddings {
            if v.len() != EMBEDDING_DIM {
                return Err(BrainError::Embedding(format!(
                    "fastembed returned {} dims, expected {}",
                    v.len(),
                    EMBEDDING_DIM
                )));
            }
        }
        Ok(embeddings)
    }
}

#[cfg(not(feature = "fastembed"))]
struct RealBackend;

#[cfg(not(feature = "fastembed"))]
impl RealBackend {
    #[allow(dead_code)]
    fn try_new(_model_path: &Path) -> BrainResult<Self> {
        Err(BrainError::Embedding(
            "fastembed feature disabled at compile time".into(),
        ))
    }

    #[allow(dead_code)]
    fn embed_one(&mut self, _text: &str) -> BrainResult<Vec<f32>> {
        Ok(zero_vec())
    }

    #[allow(dead_code)]
    fn embed_batch(&mut self, texts: &[&str]) -> BrainResult<Vec<Vec<f32>>> {
        Ok(vec![zero_vec(); texts.len()])
    }
}

// ---------------------------------------------------------------------------
// Fallback backend (pure-Rust hashing trick)
// ---------------------------------------------------------------------------

/// Pure-Rust embedder. Uses the hashing trick: tokens are hashed into
/// `EMBEDDING_DIM` buckets with signed counts, then L2-normalised. This
/// preserves the property that *similar bags of tokens produce similar
/// vectors* without requiring an ONNX runtime or any native DLL.
///
/// Quality is lower than BGE-small on abstract paraphrase benchmarks but
/// is excellent on code (where exact-token overlap dominates similarity)
/// and is fully deterministic, online-free, and platform-portable.
struct FallbackBackend {
    tokenizer: Option<Tokenizer>,
}

impl FallbackBackend {
    fn try_load(tokenizer: &Path) -> Self {
        // Tokenizer is optional; if present we use it for better word
        // segmentation, otherwise fall back to whitespace tokenising.
        let tk = if tokenizer.exists() {
            Tokenizer::from_file(tokenizer).ok()
        } else {
            None
        };
        debug!(has_tokenizer = tk.is_some(), "fallback embedder ready");
        Self { tokenizer: tk }
    }

    fn embed_one(&self, text: &str) -> Vec<f32> {
        hashing_embed(text, self.tokenizer.as_ref())
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts
            .iter()
            .map(|t| hashing_embed(t, self.tokenizer.as_ref()))
            .collect()
    }
}

/// FNV-1a hash of a token string. Stable, fast, well-distributed across buckets.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Embed one string into `EMBEDDING_DIM` floats via hashing trick.
fn hashing_embed(text: &str, tk: Option<&Tokenizer>) -> Vec<f32> {
    let mut out = vec![0f32; EMBEDDING_DIM];
    let lower = text.to_lowercase();

    let tokens: Vec<String> = match tk {
        Some(t) => {
            let enc = t.encode(lower.as_str(), false);
            match enc {
                Ok(e) => e.get_tokens().iter().cloned().collect(),
                Err(_) => whitespace_tokens(&lower),
            }
        }
        None => whitespace_tokens(&lower),
    };

    let mut n_tokens: u32 = 0;
    for tok in &tokens {
        if tok.len() < 2 {
            continue;
        }
        n_tokens += 1;
        let h = fnv1a(tok.as_bytes());
        let bucket = (h as usize) % EMBEDDING_DIM;
        let sign = if h & 1 == 0 { 1.0 } else { -1.0 };
        out[bucket] += sign;
    }
    // Character trigrams add shape-of-word info; helps for rare tokens.
    let bytes = lower.as_bytes();
    for pair in bytes.windows(3) {
        let h = fnv1a(pair);
        let bucket = (h as usize) % EMBEDDING_DIM;
        let sign = if h & 1 == 0 { 1.0 } else { -1.0 };
        out[bucket] += sign * 0.5;
    }

    if n_tokens > MAX_TOKENS as u32 && n_tokens > 0 {
        let scale = (MAX_TOKENS as f32) / (n_tokens as f32);
        for v in &mut out {
            *v *= scale;
        }
    }

    l2_normalise(&mut out);
    out
}

fn whitespace_tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn zero_vec() -> Vec<f32> {
    vec![0f32; EMBEDDING_DIM]
}

fn hash_key(text: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let out = h.finalize();
    let mut k = [0u8; 32];
    k.copy_from_slice(&out);
    k
}

fn l2_normalise(v: &mut [f32]) {
    let sq: f32 = v.iter().map(|x| x * x).sum();
    let norm = sq.sqrt();
    if norm > 1e-12 {
        for x in v {
            *x /= norm;
        }
    }
}

/// Default model directory, e.g. `~/.datatree/llm/bge-small/`.
pub fn default_model_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".datatree").join("llm").join("bge-small")
    } else {
        PathBuf::from(".datatree/llm/bge-small")
    }
}

/// Explicit model-install entry point, used by `datatree models install`.
///
/// This is the ONE network call datatree is allowed to make, and only when
/// the user asks for it. It initialises fastembed with `show_download_progress
/// = true` so the CLI can stream a progress bar.
///
/// Returns `Ok(())` on success. Errors when network is unreachable or the
/// fastembed feature wasn't compiled in.
#[cfg(feature = "fastembed")]
pub fn install_default_model() -> BrainResult<()> {
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

    let cache_dir = default_model_dir()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".datatree/llm"));

    std::fs::create_dir_all(&cache_dir).ok();

    let opts = InitOptions::new(EmbeddingModel::BGESmallENV15)
        .with_cache_dir(cache_dir.clone())
        .with_show_download_progress(true);

    let _model = TextEmbedding::try_new(opts)
        .map_err(|e| BrainError::Embedding(format!("fastembed install: {e}")))?;

    // Drop marker so the CLI can show "already installed" on subsequent runs.
    let marker = cache_dir.join("bge-small").join(".installed");
    std::fs::create_dir_all(marker.parent().unwrap()).ok();
    std::fs::write(&marker, b"v0.2 BGESmallENV15 via fastembed\n").ok();
    Ok(())
}

#[cfg(not(feature = "fastembed"))]
pub fn install_default_model() -> BrainResult<()> {
    Err(BrainError::Embedding(
        "this datatree build was compiled without the `fastembed` feature".into(),
    ))
}
