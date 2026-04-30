//! Server-Sent Events transport.
//!
//! `GET /events/:topic` opens a long-lived `text/event-stream` response. The
//! `:topic` path segment is a topic *pattern* (it may contain wildcards) and
//! the client is automatically subscribed to it. Additional patterns may be
//! supplied via the optional `?topics=a,b,c` query parameter.
//!
//! Each event is serialized as a single JSON line in the SSE `data:` field.
//! A keepalive comment (`: keepalive`) is sent every 15 seconds so proxies
//! don't kill the stream.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;
use futures::stream::Stream;
use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tracing::{debug, info};

use crate::bus::validate_topic;
use crate::error::LivebusError;
use crate::subscriber::SubscriberManager;

/// Optional query parameters for the SSE endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct SseQuery {
    /// Comma-separated list of *additional* topic patterns to subscribe to.
    pub topics: Option<String>,
}

/// Axum handler for `GET /events/:topic`.
pub async fn sse_handler(
    State(mgr): State<SubscriberManager>,
    Path(topic): Path<String>,
    Query(q): Query<SseQuery>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, LivebusError> {
    // Build the pattern list: always the path-supplied topic, plus any
    // comma-delimited extras from the query string.
    let mut patterns: Vec<String> = Vec::with_capacity(4);
    patterns.push(topic);
    if let Some(extra) = q.topics {
        for p in extra.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            patterns.push(p.to_string());
        }
    }

    // Validate up front so we reject bad requests with 400.
    for p in &patterns {
        validate_topic(p)?;
    }

    let handle = mgr.register(patterns.clone())?;
    let sub_id = handle.id.clone();
    info!(subscriber = %sub_id, ?patterns, "SSE subscriber attached");

    // Convert the bounded mpsc Receiver into a Stream<Item = Event>, then map
    // each Event to an SSE frame. We do *not* fail the whole stream on a
    // single serialization error — we send a comment frame with the error
    // text instead.
    let stream = ReceiverStream::new(handle.rx).map(move |ev| {
        let frame = match ev.to_json_line() {
            Ok(line) => SseEvent::default().event(ev.topic.clone()).data(line),
            Err(e) => {
                debug!(error = %e, "sse: failed to serialize event");
                SseEvent::default().comment(format!("serialize error: {e}"))
            }
        };
        Ok::<_, Infallible>(frame)
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}

/// Convenience handler for `GET /events` (no path param) — subscribes to
/// everything via the bare `#` pattern. Mostly useful for debugging.
pub async fn sse_firehose_handler(
    State(mgr): State<SubscriberManager>,
) -> impl IntoResponse {
    sse_handler(
        State(mgr),
        Path("#".to_string()),
        Query(SseQuery::default()),
    )
    .await
}
