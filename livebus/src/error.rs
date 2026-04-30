//! Crate-wide error type.

use thiserror::Error;

/// Errors emitted by the livebus crate.
#[derive(Debug, Error)]
pub enum LivebusError {
    /// Refused to bind, e.g. caller asked for a non-loopback host.
    #[error("bind error: {0}")]
    Bind(String),

    /// I/O error from the HTTP listener, IPC socket, or filesystem.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON encode/decode failure (subscribe message, event payload, etc.).
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid topic pattern (empty segment, control character, etc.).
    #[error("invalid topic: {0}")]
    InvalidTopic(String),

    /// Subscriber dropped because its queue overflowed the backpressure window.
    #[error("subscriber {0} evicted: {1}")]
    SubscriberEvicted(String, String),

    /// Inbound channel closed while we were trying to publish.
    #[error("bus channel closed")]
    BusClosed,

    /// HTTP/Axum error.
    #[error("http error: {0}")]
    Http(String),

    /// IPC frame too large.
    #[error("ipc frame too large: {0} bytes (max {1})")]
    FrameTooLarge(usize, usize),

    /// Catch-all for unexpected internal failures.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for LivebusError {
    fn from(err: anyhow::Error) -> Self {
        LivebusError::Internal(err.to_string())
    }
}

impl axum::response::IntoResponse for LivebusError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        let status = match self {
            LivebusError::InvalidTopic(_) => StatusCode::BAD_REQUEST,
            LivebusError::Bind(_) => StatusCode::INTERNAL_SERVER_ERROR,
            LivebusError::SubscriberEvicted(_, _) => StatusCode::GONE,
            LivebusError::FrameTooLarge(_, _) => StatusCode::PAYLOAD_TOO_LARGE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
