//! WebSocket transport.
//!
//! `GET /ws` upgrades to a WebSocket. Control messages are JSON objects:
//!
//! ```jsonc
//! // Replace the subscription set
//! { "op": "subscribe",   "topics": ["project.*.file_changed"] }
//!
//! // Drop a topic pattern (no-op if not previously subscribed)
//! { "op": "unsubscribe", "topics": ["project.abc.test_status"] }
//!
//! // Connection liveness probe
//! { "op": "ping" }
//! ```
//!
//! Server-pushed events and replies are JSON objects with an `op` field:
//!
//! ```jsonc
//! { "op": "event", "event": <Event> }
//! { "op": "ack",   "patterns": ["project.*.file_changed"] }
//! { "op": "error", "message": "..." }
//! { "op": "pong" }
//! ```

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::bus::validate_topic;
use crate::subscriber::SubscriberManager;

/// Inbound control frame from the client.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WsClientMsg {
    Subscribe { topics: Vec<String> },
    Unsubscribe { topics: Vec<String> },
    Ping,
}

/// Outbound frame from the server.
#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WsServerMsg<'a> {
    Event { event: &'a crate::event::Event },
    Ack { patterns: &'a [String] },
    Error { message: String },
    Pong,
}

/// Axum handler that performs the WebSocket upgrade.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(mgr): State<SubscriberManager>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, mgr))
}

async fn handle_socket(socket: WebSocket, mgr: SubscriberManager) {
    let (mut sink, mut stream) = socket.split();

    // Start with no patterns; client must send a `subscribe` first.
    let handle = match mgr.register(Vec::new()) {
        Ok(h) => h,
        Err(e) => {
            let _ = sink
                .send(Message::Text(
                    serde_json::to_string(&WsServerMsg::Error {
                        message: e.to_string(),
                    })
                    .unwrap_or_else(|_| "{}".into()),
                ))
                .await;
            return;
        }
    };
    let sub_id = handle.id.clone();
    info!(subscriber = %sub_id, "WebSocket subscriber attached");

    let mut active_patterns: Vec<String> = Vec::new();
    let mut rx = handle.rx;

    loop {
        tokio::select! {
            // ---------- Server -> Client: forward bus events ----------
            maybe_event = rx.recv() => {
                match maybe_event {
                    Some(ev) => {
                        let payload = match serde_json::to_string(
                            &WsServerMsg::Event { event: &ev },
                        ) {
                            Ok(s) => s,
                            Err(e) => {
                                debug!(error = %e, "ws: failed to encode event");
                                continue;
                            }
                        };
                        if sink.send(Message::Text(payload)).await.is_err() {
                            debug!(subscriber = %sub_id, "ws: peer closed during send");
                            break;
                        }
                    }
                    None => {
                        // Bug G-8 (2026-05-01): manager-initiated eviction.
                        // The Close-frame send is intentionally best-effort
                        // (`let _ =`): we're about to `break` regardless,
                        // so a failed send (peer already gone) is fine.
                        // Logged at debug so the eviction is still visible
                        // in supervisor.log when verbose logging is on.
                        if let Err(e) = sink.send(Message::Close(None)).await {
                            debug!(
                                subscriber = %sub_id,
                                error = %e,
                                "ws: failed to send Close frame on manager eviction (peer already gone)"
                            );
                        }
                        break;
                    }
                }
            }

            // ---------- Client -> Server: control frames ----------
            maybe_msg = stream.next() => {
                let Some(msg) = maybe_msg else {
                    debug!(subscriber = %sub_id, "ws: client stream ended");
                    break;
                };
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(subscriber = %sub_id, error = %e, "ws: read error");
                        break;
                    }
                };
                match msg {
                    Message::Text(txt) => {
                        let parsed: Result<WsClientMsg, _> =
                            serde_json::from_str(&txt);
                        match parsed {
                            Ok(WsClientMsg::Subscribe { topics }) => {
                                if let Err(e) = topics.iter().try_for_each(|t| validate_topic(t)) {
                                    let _ = sink.send(Message::Text(
                                        serde_json::to_string(&WsServerMsg::Error {
                                            message: e.to_string(),
                                        }).unwrap_or_default(),
                                    )).await;
                                    continue;
                                }
                                // Merge new patterns into the active set
                                // (subscribe = additive, like AMQP).
                                for t in topics {
                                    if !active_patterns.contains(&t) {
                                        active_patterns.push(t);
                                    }
                                }
                                if let Err(e) = mgr.update_patterns(&sub_id, active_patterns.clone()) {
                                    let _ = sink.send(Message::Text(
                                        serde_json::to_string(&WsServerMsg::Error {
                                            message: e.to_string(),
                                        }).unwrap_or_default(),
                                    )).await;
                                    continue;
                                }
                                let _ = sink.send(Message::Text(
                                    serde_json::to_string(&WsServerMsg::Ack {
                                        patterns: &active_patterns,
                                    }).unwrap_or_default(),
                                )).await;
                            }
                            Ok(WsClientMsg::Unsubscribe { topics }) => {
                                active_patterns.retain(|p| !topics.contains(p));
                                let _ = mgr.update_patterns(&sub_id, active_patterns.clone());
                                let _ = sink.send(Message::Text(
                                    serde_json::to_string(&WsServerMsg::Ack {
                                        patterns: &active_patterns,
                                    }).unwrap_or_default(),
                                )).await;
                            }
                            Ok(WsClientMsg::Ping) => {
                                let _ = sink.send(Message::Text(
                                    serde_json::to_string(&WsServerMsg::Pong).unwrap_or_default(),
                                )).await;
                            }
                            Err(e) => {
                                let _ = sink.send(Message::Text(
                                    serde_json::to_string(&WsServerMsg::Error {
                                        message: format!("bad control frame: {e}"),
                                    }).unwrap_or_default(),
                                )).await;
                            }
                        }
                    }
                    Message::Binary(_) => {
                        // Reserved — we only speak JSON over text frames.
                        let _ = sink.send(Message::Text(
                            serde_json::to_string(&WsServerMsg::Error {
                                message: "binary frames not supported".into(),
                            }).unwrap_or_default(),
                        )).await;
                    }
                    Message::Ping(p) => { let _ = sink.send(Message::Pong(p)).await; }
                    Message::Pong(_) => {}
                    Message::Close(_) => {
                        debug!(subscriber = %sub_id, "ws: client sent close");
                        break;
                    }
                }
            }
        }
    }

    mgr.unregister(&sub_id);
    info!(subscriber = %sub_id, "WebSocket subscriber detached");
}
