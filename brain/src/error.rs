//! Crate error type.

use thiserror::Error;

/// Anything the brain crate can fail with.
#[derive(Debug, Error)]
pub enum BrainError {
    #[error("model file missing: {path}")]
    ModelMissing { path: String },

    #[error("tokenizer file missing: {path}")]
    TokenizerMissing { path: String },

    #[error("ONNX runtime error: {0}")]
    Onnx(String),

    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    #[error("embedding store error: {0}")]
    Store(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Leiden error: {0}")]
    Leiden(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("channel closed")]
    ChannelClosed,

    #[error("worker shut down")]
    WorkerDown,

    #[error("other: {0}")]
    Other(#[from] anyhow::Error),
}

/// Convenient `Result` alias.
pub type BrainResult<T> = std::result::Result<T, BrainError>;

impl From<tokenizers::Error> for BrainError {
    fn from(e: tokenizers::Error) -> Self {
        BrainError::Tokenizer(e.to_string())
    }
}
