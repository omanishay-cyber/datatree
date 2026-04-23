//! End-to-end style tests for the in-process bus and HTTP transport.
//!
//! These tests deliberately avoid the IPC ingest path so they can run on any
//! platform without socket-cleanup concerns. The IPC layer is exercised
//! manually via the `mneme-livebus` binary.

use std::time::Duration;

use crate::bus::{topic_matches, validate_topic, EventBus};
use crate::event::{Event, EventPayload, FileChangeKind, FileChanged};
use crate::subscriber::{SubscriberManager, BACKPRESSURE_WINDOW};
use crate::{bind_addr, DEFAULT_HOST, DEFAULT_PORT};

// ----------------------------- bind safety -----------------------------------

#[test]
fn bind_refuses_non_loopback() {
    assert!(bind_addr(DEFAULT_HOST, DEFAULT_PORT).is_ok());
    assert!(bind_addr("127.0.0.1", 7778).is_ok());
    assert!(bind_addr("::1", 7778).is_ok());
    assert!(bind_addr("0.0.0.0", 7778).is_err());
    assert!(bind_addr("192.168.1.10", 7778).is_err());
    assert!(bind_addr("not-an-ip", 7778).is_err());
}

// ----------------------------- topic patterns --------------------------------

#[test]
fn wildcard_matches_design_examples() {
    // From design §11.2 / §11.3
    assert!(topic_matches(
        "project.*.file_changed",
        "project.abc123.file_changed"
    ));
    assert!(topic_matches(
        "session.current.compaction_detected",
        "session.current.compaction_detected"
    ));
    assert!(!topic_matches(
        "project.*.file_changed",
        "session.x.file_changed"
    ));
    assert!(topic_matches(
        "project.abc.#",
        "project.abc.subagent_event"
    ));
}

#[test]
fn rejects_invalid_topics() {
    assert!(validate_topic("").is_err());
    assert!(validate_topic("project..file_changed").is_err());
    assert!(validate_topic("project.\nabc.file_changed").is_err());
}

// ----------------------------- in-process fan-out ----------------------------

#[tokio::test]
async fn subscribe_and_receive_typed_event() {
    let bus = EventBus::new();
    let mgr = SubscriberManager::new(bus.clone());
    let mut handle = mgr
        .register(vec!["project.*.file_changed".into()])
        .expect("register");

    let payload = EventPayload::FileChanged(FileChanged {
        path: "src/lib.rs".into(),
        change_kind: FileChangeKind::Modified,
        bytes: Some(128),
        content_hash: None,
    });
    let ev = Event::from_typed(
        "project.abc123.file_changed",
        None,
        Some("abc123".into()),
        payload,
    );
    mgr.dispatch(&ev);

    let received = tokio::time::timeout(Duration::from_millis(100), handle.rx.recv())
        .await
        .expect("recv timeout")
        .expect("channel closed");
    assert_eq!(received.topic, "project.abc123.file_changed");
    assert_eq!(received.project_hash.as_deref(), Some("abc123"));
}

#[tokio::test]
async fn non_matching_topic_is_filtered_out() {
    let bus = EventBus::new();
    let mgr = SubscriberManager::new(bus);
    let mut handle = mgr
        .register(vec!["project.specific.file_changed".into()])
        .unwrap();
    mgr.dispatch(&Event::from_json(
        "session.x.compaction_detected",
        Some("x".into()),
        None,
        serde_json::Value::Null,
    ));
    assert!(handle.rx.try_recv().is_err());
}

#[tokio::test]
async fn slow_subscriber_is_evicted_after_window() {
    let bus = EventBus::new();
    let mgr = SubscriberManager::new(bus.clone());
    // Hold the receiver but never drain it.
    let _slow = mgr.register(vec!["#".into()]).unwrap();

    // Fire well past the BACKPRESSURE_WINDOW so the eviction must trigger.
    for i in 0..(BACKPRESSURE_WINDOW * 5) {
        mgr.dispatch(&Event::from_json(
            "system.health",
            None,
            None,
            serde_json::json!({"i": i}),
        ));
    }
    let stats = mgr.stats();
    assert!(
        stats.evicted_subscribers >= 1,
        "expected at least one eviction, got {stats:?}"
    );
    assert_eq!(stats.active_subscribers, 0);
}

#[tokio::test]
async fn eviction_emits_degraded_mode_event() {
    let bus = EventBus::new();
    let mgr = SubscriberManager::new(bus.clone());

    // Watcher subscriber listens to system.degraded_mode.
    let mut watcher = mgr.register(vec!["system.degraded_mode".into()]).unwrap();
    // Slow subscriber — never drains.
    let _slow = mgr.register(vec!["system.health".into()]).unwrap();

    for i in 0..(BACKPRESSURE_WINDOW * 4) {
        mgr.dispatch(&Event::from_json(
            "system.health",
            None,
            None,
            serde_json::json!({"i": i}),
        ));
    }

    let evicted = tokio::time::timeout(Duration::from_millis(250), watcher.rx.recv())
        .await
        .expect("expected a degraded_mode event")
        .expect("watcher channel closed");
    assert_eq!(evicted.topic, "system.degraded_mode");
}

// ----------------------------- HTTP / SSE smoke ------------------------------

#[tokio::test]
async fn sse_endpoint_streams_an_event() {
    use axum::routing::get;
    use axum::Router;

    let bus = EventBus::new();
    let mgr = SubscriberManager::new(bus.clone());
    let app = Router::new()
        .route("/events/:topic", get(crate::sse::sse_handler))
        .with_state(mgr.clone());

    // Bind to an OS-assigned loopback port so multiple test runs don't
    // collide.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    // Connect a raw TCP client and parse SSE manually — keeps test deps
    // light. We use reqwest only if available in dev-dependencies; here we
    // open the stream and read the first chunk.
    let client = reqwest::Client::new();
    let url = format!("http://{local}/events/project.*.file_changed");
    let req_fut = client.get(&url).send();
    let resp = tokio::time::timeout(Duration::from_secs(2), req_fut)
        .await
        .expect("connect timeout")
        .expect("request failed");
    assert!(resp.status().is_success());

    // Give the server a beat to register the subscriber, then publish.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let ev = Event::from_json(
        "project.abc.file_changed",
        None,
        Some("abc".into()),
        serde_json::json!({"path": "x.rs"}),
    );
    mgr.dispatch(&ev);

    use futures::StreamExt;
    let mut stream = resp.bytes_stream();
    let chunk = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("stream timeout");
    let bytes = chunk.expect("stream ended").expect("stream error");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("project.abc.file_changed"),
        "expected SSE frame to contain topic, got: {text}"
    );

    server.abort();
}
