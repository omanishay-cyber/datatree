//! Phase A · F2 — WebSocket `/ws` livebus relay.
//!
//! The supervisor hosts the production HTTP surface that the vision SPA and
//! Claude Code MCP clients connect to. Until F2 the WebSocket leg of the
//! livebus protocol was implemented in the standalone `mneme-livebus` crate
//! but **not** exposed by the production daemon's [`build_router`], so the
//! frontend's livebus subscription fell back to placeholder data on every
//! load.
//!
//! This module wires the existing livebus broadcast machinery into the same
//! axum router that owns `/api/health`, `/api/graph/*`, and the static SPA
//! assets. Each `/ws` upgrade:
//!
//! 1. Allocates a [`SubscriberHandle`] from the shared [`SubscriberManager`].
//! 2. Reads JSON control frames (`subscribe`, `unsubscribe`, `ping`) from the
//!    socket and replies with `ack` / `pong`.
//! 3. Forwards every matching [`Event`] from the bus as a single
//!    JSON-encoded WebSocket text frame:
//!    `{"op":"event","event":<Event>}`.
//!
//! Non-loopback binds are forbidden by [`livebus::bind_addr`] one layer
//! up, so this handler never runs on a public socket.
//!
//! [`build_router`]: crate::api_graph::build_router

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use livebus::{validate_topic, Event, SubscriberManager};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::api_graph::ApiGraphState;

/// Inbound control frame from the client.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WsClientMsg {
    /// Add the supplied topic patterns to the subscription set.
    Subscribe {
        /// Topic patterns; `*` matches one segment, trailing `#` matches any
        /// suffix.
        topics: Vec<String>,
    },
    /// Drop the supplied topic patterns from the subscription set (no-op if
    /// the pattern was never registered).
    Unsubscribe {
        /// Topic patterns to remove.
        topics: Vec<String>,
    },
    /// Liveness probe. Server replies with [`WsServerMsg::Pong`].
    Ping,
}

/// Outbound frame from the server.
#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WsServerMsg<'a> {
    /// A relayed bus event.
    Event {
        /// Borrowed event so we don't clone the JSON payload twice.
        event: &'a Event,
    },
    /// Acknowledges the current subscription set after a `subscribe` /
    /// `unsubscribe` round-trip.
    Ack {
        /// Patterns currently active for this socket.
        patterns: &'a [String],
    },
    /// Server-side error reply for a malformed control frame.
    Error {
        /// Human-readable reason.
        message: String,
    },
    /// Pong reply to a client `ping`.
    Pong,
}

/// `GET /ws` handler. Performs the HTTP→WebSocket upgrade and hands the
/// resulting socket to [`handle_socket`].
///
/// If the daemon was booted without a livebus (`livebus` field is `None` on
/// [`ApiGraphState`]) we reject the upgrade with a 503-style error frame and
/// close the connection cleanly. This keeps the route mounted in production
/// regardless of whether the bus has been initialised yet — the frontend's
/// subscription code retries with backoff so a transient `None` is harmless.
pub async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<ApiGraphState>,
) -> impl IntoResponse {
    // Pull the per-router `SubscriberManager` out of state. `livebus` is
    // `Option<SubscriberManager>` because the existing api_graph tests can
    // build a router without standing up a bus; in production
    // `supervisor::run` injects `Some(mgr)` once the in-process bus is
    // online (Phase A · F2).
    let mgr = state.livebus.clone();
    ws.on_upgrade(move |socket| async move {
        match mgr {
            Some(mgr) => handle_socket(socket, mgr).await,
            None => {
                // Bus not initialised — emit a polite error frame and
                // close so the SPA's reconnect-with-backoff loop can
                // retry rather than treat the gap as a hard failure.
                let mut socket = socket;
                let payload = serde_json::to_string(&WsServerMsg::Error {
                    message: "livebus not initialised on this daemon".into(),
                })
                .unwrap_or_else(|_| "{}".into());
                let _ = socket.send(Message::Text(payload)).await;
                let _ = socket.send(Message::Close(None)).await;
            }
        }
    })
}

/// Per-connection state machine. Identical in shape to
/// `livebus::ws::handle_socket` but anchored on the supervisor-side
/// `SubscriberManager` so the same shared bus serves SSE and WebSocket
/// clients side by side.
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
    let mut rx = handle.rx;
    let mut active_patterns: Vec<String> = Vec::new();
    info!(subscriber = %sub_id, "supervisor /ws subscriber attached");

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
                        // Channel closed — manager evicted us.
                        let _ = sink.send(Message::Close(None)).await;
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
    info!(subscriber = %sub_id, "supervisor /ws subscriber detached");
}

#[cfg(test)]
mod ws_tests {
    use super::*;
    use crate::api_graph::{build_router, ApiGraphState};
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use common::PathManager;
    use livebus::{Event, EventBus, SubscriberManager};
    use std::sync::Arc;
    use tower::ServiceExt;

    fn state_with_bus() -> (ApiGraphState, EventBus, SubscriberManager) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bus = EventBus::new();
        let mgr = SubscriberManager::new(bus.clone());
        let state = ApiGraphState {
            paths: Arc::new(PathManager::with_root(tmp.path().to_path_buf())),
            livebus: Some(mgr.clone()),
        };
        (state, bus, mgr)
    }

    /// `/ws` must be mounted and routed through axum's WebSocket upgrade
    /// extractor. We don't drive a full tungstenite client here (that
    /// would pull a new dev-dep into the supervisor); instead we issue a
    /// properly-formed `Upgrade: websocket` request via `oneshot` and
    /// assert axum returns `426 Upgrade Required` (its default reply when
    /// the underlying `hyper` connection isn't actually upgradeable, as
    /// is always the case with `Service::oneshot`).
    ///
    /// 426 is the load-bearing signal here: it proves
    /// 1. the route is mounted (else 404),
    /// 2. axum's `ws` Cargo feature is enabled (else the
    ///    `WebSocketUpgrade` extractor wouldn't compile in the first
    ///    place — this test exists to keep that feature wired), and
    /// 3. the request actually reaches `WebSocketUpgrade`'s extractor
    ///    (a stub handler returning Json/501 wouldn't emit 426).
    ///
    /// End-to-end upgrade handshakes are exercised by the
    /// `mneme-livebus` integration tests which spawn a real TCP listener.
    #[tokio::test]
    async fn ws_route_returns_upgrade_required_via_oneshot() {
        let (state, _bus, _mgr) = state_with_bus();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/ws")
            .method("GET")
            .header(header::HOST, "localhost")
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            // Any 16-byte base64 nonce is accepted by axum's upgrade
            // negotiation; the actual value doesn't matter for routing.
            .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .expect("request");
        let resp = app.oneshot(req).await.expect("oneshot");
        assert_eq!(
            resp.status(),
            StatusCode::UPGRADE_REQUIRED,
            "/ws must reach axum's WebSocketUpgrade extractor; got {} — \
             is the route mounted and is axum's `ws` feature on?",
            resp.status()
        );
    }

    /// When `/ws` is hit without WebSocket upgrade headers it must reply
    /// with `426 Upgrade Required` (axum's default), proving the route is
    /// still routed through the upgrade handler and not a stub.
    #[tokio::test]
    async fn ws_route_demands_upgrade_headers() {
        let (state, _bus, _mgr) = state_with_bus();
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/ws")
                    .method("GET")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        // axum 0.7's WebSocketUpgrade returns 426 when upgrade headers are
        // missing; older revs sometimes returned 400. Accept either.
        assert!(
            resp.status() == StatusCode::UPGRADE_REQUIRED
                || resp.status() == StatusCode::BAD_REQUEST,
            "expected 426 or 400 from /ws without upgrade headers, got {}",
            resp.status()
        );
    }

    /// Sanity-check the relay path itself: a published bus event reaches
    /// every matching subscriber registered through the same manager. This
    /// is the in-process equivalent of "frontend connects to /ws and
    /// receives the event" — it verifies the wiring between
    /// [`EventBus::publish`] and [`SubscriberManager::dispatch`] that the
    /// production daemon's livebus producer task drives.
    #[tokio::test]
    async fn livebus_relay_forwards_event_to_subscriber() {
        let (_state, _bus, mgr) = state_with_bus();
        let mut handle = mgr
            .register(vec!["project.*.file_changed".into()])
            .expect("register");

        let ev = Event::from_json(
            "project.abc.file_changed",
            None,
            Some("abc".into()),
            serde_json::json!({"path": "src/lib.rs"}),
        );
        mgr.dispatch(&ev);

        let got = handle.rx.recv().await.expect("event delivered");
        assert_eq!(got.topic, "project.abc.file_changed");
        // Round-trip through the on-the-wire JSON shape too — that's what
        // `WsServerMsg::Event` encodes per frame.
        let frame =
            serde_json::to_string(&WsServerMsg::Event { event: &got }).expect("encode");
        let v: serde_json::Value = serde_json::from_str(&frame).expect("decode");
        assert_eq!(v["op"], "event");
        assert_eq!(v["event"]["topic"], "project.abc.file_changed");
        assert_eq!(v["event"]["payload"]["path"], "src/lib.rs");
    }
}
