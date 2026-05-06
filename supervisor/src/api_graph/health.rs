//! Health-probe handlers extracted from `api_graph/mod.rs` (HIGH-45 split).
//!
//! Owns `/api/health`, `/api/daemon/health`, `/api/voice`, and the generic
//! `stub_handler` used by unimplemented endpoints. All four are zero-state
//! reads — they take `ApiGraphState` only to keep the axum extractor
//! signature uniform with the data handlers; the State is unused.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use super::ApiGraphState;

/// `GET /api/health` — daemon-side liveness probe used by
/// `vision/src/api.ts:64`. Mirrors the wire shape of the Bun server's
/// `/api/health` (see `vision/server.ts:244-253`).
pub(super) async fn api_health(State(_state): State<ApiGraphState>) -> impl IntoResponse {
    // `Date.now()` in JS is unix-millis. We emit unix-millis as `i64`
    // so the existing TS consumer parses it identically.
    let ts_ms: i64 = chrono::Utc::now().timestamp_millis();
    // LOW fix (2026-05-05 audit): drop the internal `phase: "D0"`
    // milestone code from public health responses. It leaked
    // pre-release planning state ("D0" = our internal sub-milestone
    // string) that meant nothing to operators and confused users
    // reading the JSON response. The other fields (ok/host/port/ts)
    // carry the actual liveness signal. If a future caller wants to
    // distinguish daemon flavours we'll add an explicit `version`
    // field tied to CARGO_PKG_VERSION.
    Json(json!({
        "ok": true,
        "host": "127.0.0.1",
        "port": 7777,
        "ts": ts_ms,
    }))
}

/// `GET /api/daemon/health` — alias for `/api/health`. The vision
/// frontend uses both URLs interchangeably as a daemon liveness probe
/// (see `vision/src/api/graph.ts:fetchDaemonHealth` and the older
/// Bun-server `probeDaemon` helper). We mirror the same JSON body so the
/// frontend doesn't have to discriminate.
pub(super) async fn api_daemon_health(State(_state): State<ApiGraphState>) -> impl IntoResponse {
    let ts_ms: i64 = chrono::Utc::now().timestamp_millis();
    // LOW fix (2026-05-05 audit): see `api_health` — same drop of
    // the internal `phase: "D0"` milestone leak. Health alias must
    // match the canonical /api/health shape exactly.
    Json(json!({
        "ok": true,
        "host": "127.0.0.1",
        "port": 7777,
        "ts": ts_ms,
    }))
}

/// `GET /api/voice` — stubbed voice endpoint, documented as
/// `phase: "stub"` in v0.3 (CLAUDE.md "Known limitations"). Kept
/// distinct from the 501 stubs because the wire shape `{enabled,
/// phase}` is contractual.
pub(super) async fn voice_stub() -> impl IntoResponse {
    Json(json!({
        "enabled": false,
        "phase": "stub",
    }))
}

/// Generic stub handler for endpoints not yet ported. Returns HTTP 501
/// with a JSON envelope so the frontend's `placeholderPayload()`
/// fallback fires cleanly instead of choking on HTML.
///
/// LOW fix (2026-05-05 audit): drop the `phase: "D0"` and
/// `next: "D2-D6"` internal milestone codes from the public 501
/// envelope. They leaked our planning sub-milestones into responses
/// users + operators see and meant nothing outside the maintainer
/// team. The `error: "not_implemented"` carries the only actionable
/// signal a caller needs.
pub(super) async fn stub_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "not_implemented",
        })),
    )
}
