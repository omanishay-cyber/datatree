//! Local LLM wrapper around llama.cpp (Phi-3-mini-4k Q4_K_M).
//!
//! Only compiled when the `llm` cargo feature is enabled. The crate works
//! perfectly well without it — concept extraction & summarisation simply
//! skip the LLM stage and fall back to deterministic output.
//!
//! Model layout:
//! ```text
//! ~/.datatree/llm/phi-3-mini-4k/
//!   model.gguf      # Phi-3-mini-4k-instruct, Q4_K_M
//! ```
//!
//! Threading: `LocalLlm` is `Send + Sync`, but inference itself is wrapped
//! in a `Mutex` because llama.cpp contexts are not internally synchronised.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{info, warn};

use crate::error::{BrainError, BrainResult};

/// Public handle. Cheap to clone.
#[derive(Clone)]
pub struct LocalLlm {
    inner: Arc<Mutex<Inner>>,
    model_path: PathBuf,
}

impl std::fmt::Debug for LocalLlm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalLlm")
            .field("model_path", &self.model_path)
            .field("ready", &self.is_ready())
            .finish()
    }
}

struct Inner {
    backend: Option<Backend>,
}

// We deliberately keep the llama-cpp-2 surface area inside this module so
// that flipping the feature on/off does not require editing call-sites.
struct Backend {
    // Held opaquely; only used through the helper methods below. The exact
    // type is intentionally not part of our public API since llama-cpp-2's
    // session/context types are evolving rapidly.
    backend: llama_cpp_2::llama_backend::LlamaBackend,
    model: llama_cpp_2::model::LlamaModel,
}

impl LocalLlm {
    /// Convenience: load Phi-3 from the default `~/.datatree/llm/phi-3-mini-4k/model.gguf`.
    pub fn from_default_path() -> Self {
        Self::new(&default_model_path())
    }

    /// Try to load a GGUF model. On failure the LLM enters degraded mode
    /// (every method returns the deterministic fallback) — this is **not**
    /// an error to the caller.
    pub fn new(path: &Path) -> Self {
        let backend = if path.exists() {
            match try_load(path) {
                Ok(b) => {
                    info!(path = %path.display(), "Phi-3 loaded");
                    Some(b)
                }
                Err(e) => {
                    warn!(error = %e, path = %path.display(), "Phi-3 load failed — degraded LLM");
                    None
                }
            }
        } else {
            warn!(path = %path.display(), "Phi-3 model missing — degraded LLM");
            None
        };
        Self {
            inner: Arc::new(Mutex::new(Inner { backend })),
            model_path: path.to_path_buf(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.inner.lock().backend.is_some()
    }

    /// Run a free-form prompt; cap output at `max_tokens` (default 256).
    /// In degraded mode this returns `Ok(String::new())`.
    pub fn complete(&self, prompt: &str, max_tokens: usize) -> BrainResult<String> {
        let mut g = self.inner.lock();
        let Some(backend) = g.backend.as_mut() else {
            return Ok(String::new());
        };
        backend
            .complete(prompt, max_tokens)
            .map_err(|e| BrainError::Llm(e))
    }

    /// One-sentence summary helper. Routes through [`Self::complete`] with
    /// an instruction template tuned for code chunks.
    pub fn summarize_function(&self, signature: &str, body: &str) -> BrainResult<String> {
        if !self.is_ready() {
            return Ok(String::new());
        }
        let prompt = format!(
            "Summarise the following function in ONE concise sentence (<=20 words).\n\
             Signature:\n{signature}\n\nBody:\n{snippet}\n\nSummary:",
            snippet = trim_for_prompt(body, 1500),
        );
        self.complete(&prompt, 64)
    }

    /// Concept extraction helper. Returns up to 8 candidate noun-phrases.
    pub fn extract_concepts(&self, text: &str) -> BrainResult<Vec<String>> {
        if !self.is_ready() {
            return Ok(Vec::new());
        }
        let prompt = format!(
            "Extract up to 8 high-level CONCEPTS from the text below.\n\
             Reply as a comma-separated list of noun phrases, lower case, no numbering.\n\n\
             Text:\n{}\n\nConcepts:",
            trim_for_prompt(text, 2000)
        );
        let raw = self.complete(&prompt, 96)?;
        let concepts: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .take(8)
            .collect();
        Ok(concepts)
    }

    /// Hint-only routing helper used by the query layer. Returns one of
    /// `"semantic"`, `"keyword"`, or `"hybrid"`. Falls back to `"hybrid"` in
    /// degraded mode.
    pub fn route_query(&self, query: &str) -> BrainResult<String> {
        if !self.is_ready() {
            return Ok("hybrid".to_string());
        }
        let prompt = format!(
            "Classify the following query as exactly one of: semantic, keyword, hybrid.\n\n\
             Query: {query}\n\nAnswer (one word):",
        );
        let raw = self.complete(&prompt, 8)?;
        let lc = raw.trim().to_ascii_lowercase();
        let route = if lc.contains("semantic") {
            "semantic"
        } else if lc.contains("keyword") {
            "keyword"
        } else {
            "hybrid"
        };
        Ok(route.to_string())
    }
}

fn try_load(path: &Path) -> Result<Backend, String> {
    let backend =
        llama_cpp_2::llama_backend::LlamaBackend::init().map_err(|e| format!("backend: {e}"))?;
    let params = llama_cpp_2::model::params::LlamaModelParams::default();
    let model = llama_cpp_2::model::LlamaModel::load_from_file(&backend, path, &params)
        .map_err(|e| format!("model load: {e}"))?;
    Ok(Backend { backend, model })
}

impl Backend {
    fn complete(&mut self, prompt: &str, max_tokens: usize) -> Result<String, String> {
        // Note: the llama-cpp-2 surface for sampling has churned across
        // versions; rather than pin to one specific API, we wrap a single
        // safe entry point so consumers always get a `String` back.
        //
        // The heavy lifting (token-by-token decode, logits sampling) is
        // delegated to the model's helper. If the helper is absent at
        // compile time the build fails loudly and we know to bump the
        // version pin in Cargo.toml.
        use llama_cpp_2::context::params::LlamaContextParams;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(2048));
        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| format!("context: {e}"))?;
        let tokens = self
            .model
            .str_to_token(prompt, llama_cpp_2::model::AddBos::Always)
            .map_err(|e| format!("tokenise: {e}"))?;
        let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(2048, 1);
        for (i, t) in tokens.iter().enumerate() {
            let last = i == tokens.len() - 1;
            batch
                .add(*t, i as i32, &[0], last)
                .map_err(|e| format!("batch add: {e}"))?;
        }
        ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;

        let mut out = String::new();
        let mut cur_pos = tokens.len() as i32;
        let mut sampler = llama_cpp_2::sampling::LlamaSampler::greedy();
        for _ in 0..max_tokens {
            let token = sampler.sample(&ctx, -1);
            sampler.accept(token);
            if self.model.is_eog_token(token) {
                break;
            }
            let piece = self
                .model
                .token_to_str(token, llama_cpp_2::model::Special::Tokenize)
                .unwrap_or_default();
            out.push_str(&piece);
            batch.clear();
            batch
                .add(token, cur_pos, &[0], true)
                .map_err(|e| format!("batch add: {e}"))?;
            cur_pos += 1;
            ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;
        }
        Ok(out)
    }
}

fn trim_for_prompt(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        s
    } else {
        // Avoid splitting in the middle of a UTF-8 char.
        let mut end = max_chars;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        &s[..end]
    }
}

/// Default GGUF path: `~/.datatree/llm/phi-3-mini-4k/model.gguf`.
pub fn default_model_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".datatree")
            .join("llm")
            .join("phi-3-mini-4k")
            .join("model.gguf")
    } else {
        PathBuf::from(".datatree/llm/phi-3-mini-4k/model.gguf")
    }
}
