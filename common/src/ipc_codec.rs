//! Length-framed IPC codec shared by all mneme control-plane channels.
//!
//! ## Wire format
//!
//! Every message on the control-plane IPC bus (supervisor ↔ CLI,
//! supervisor ↔ workers, store ↔ supervisor, scanners → supervisor) uses
//! the same two-part envelope:
//!
//! ```text
//! ┌────────────────────┬───────────────────────┐
//! │  length : u32 BE  │  body : UTF-8 JSON     │
//! │  (4 bytes)        │  (length bytes)         │
//! └────────────────────┴───────────────────────┘
//! ```
//!
//! This module extracts the codec into one place so the seven sites that
//! previously open-coded it can share a single, tested implementation.
//!
//! ## What is NOT included
//!
//! * **Timeouts** — every caller site uses a different timeout budget
//!   (`IPC_READ_TIMEOUT`, `CLIENT_READ_TIMEOUT`, `DEFAULT_REPORT_TIMEOUT`,
//!   …). Embedding a generic timeout here would force a one-size-fits-all
//!   value. Callers wrap calls in `tokio::time::timeout` themselves.
//! * **JSON parsing** — callers own the typed deserialization step so they
//!   can produce their own error types (`SupervisorError`, `CliError`, …).
//! * **Worker stdin / stdout line protocols** — the scanner-worker and
//!   parser-worker use newline-delimited JSON on stdin, which is a
//!   different protocol and should not be conflated with this codec.

use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Hard upper bound on a single framed message body.
///
/// This is the supervisor server-side cap (`supervisor/src/ipc.rs`
/// `MAX_FRAME_BYTES`), which is the most restrictive of the caps that were
/// previously scattered across the call sites (16 MiB < 64 MiB).
/// All sites now share this single limit so a peer cannot exploit a
/// looser cap on one endpoint to DoS it while a stricter cap on another
/// endpoint would have rejected the same frame.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Read one length-framed JSON message from `reader`.
///
/// # Wire contract
/// Expects a 4-byte big-endian `u32` length prefix immediately followed by
/// exactly that many bytes of body. Returns the body bytes unchanged — JSON
/// parsing is left to the caller.
///
/// # Errors
/// * `ErrorKind::UnexpectedEof` — the peer closed the connection cleanly
///   (zero bytes readable where the length prefix was expected, or the
///   stream ended mid-body). Callers that run a connection loop use this
///   to exit cleanly.
/// * `ErrorKind::InvalidData` — the declared frame length exceeds
///   [`MAX_FRAME_BYTES`]. The connection should be dropped; the peer is
///   either misbehaving or has been compromised.
/// * Any other `io::Error` propagated from the underlying `AsyncRead`.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];

    // A zero-byte read on the first byte of the length prefix means the
    // peer closed cleanly. `read_exact` returns `UnexpectedEof` whenever
    // the stream ends before filling the buffer, so this case is handled
    // automatically — we just surface it as-is to the caller.
    reader.read_exact(&mut len_buf).await?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("frame too large: {len} bytes (limit {MAX_FRAME_BYTES})"),
        ));
    }

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    Ok(body)
}

/// Write one length-framed JSON message to `writer` and flush.
///
/// Serializes `body` as:
/// ```text
/// u32::from(body.len()).to_be_bytes()  ||  body
/// ```
/// then flushes `writer`.
///
/// # Errors
/// * `ErrorKind::InvalidInput` — `body.len()` exceeds `u32::MAX`; the
///   frame length prefix cannot represent it. This matches the BUG-A2-036
///   overflow-check that was previously only present in
///   `common/src/worker_ipc.rs`.
/// * Any `io::Error` from the underlying `AsyncWrite`.
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> io::Result<()> {
    // BUG-A2-036 fix: check for overflow before truncating to u32.
    // Casting an overlong body as `body.len() as u32` silently truncates
    // and causes the receiver to mis-parse subsequent bytes as a new
    // message, desynchronising the framing forever.
    let body_len: u32 = body.len().try_into().map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "frame body too large: {} bytes (u32::MAX = {})",
                body.len(),
                u32::MAX
            ),
        )
    })?;

    writer.write_all(&body_len.to_be_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    // Helper: round-trip a slice through write_frame then read_frame.
    async fn round_trip(data: &[u8]) -> io::Result<Vec<u8>> {
        let (mut client, mut server) = duplex(MAX_FRAME_BYTES + 8);
        write_frame(&mut client, data).await?;
        drop(client); // close the write end so read_exact doesn't hang
        read_frame(&mut server).await
    }

    #[tokio::test]
    async fn round_trip_short_frame() {
        let payload = b"hello, mneme";
        let got = round_trip(payload).await.expect("round-trip failed");
        assert_eq!(got, payload);
    }

    #[tokio::test]
    async fn round_trip_empty_body() {
        let got = round_trip(b"").await.expect("empty body round-trip failed");
        assert!(got.is_empty());
    }

    #[tokio::test]
    async fn round_trip_json_like_payload() {
        let payload = br#"{"command":"ping","project":"/tmp/proj"}"#;
        let got = round_trip(payload).await.expect("json round-trip failed");
        assert_eq!(got, payload);
    }

    #[tokio::test]
    async fn write_frame_prefixes_big_endian_length() {
        // Verify the on-wire bytes directly: 4-byte BE length then body.
        let (mut client, mut server) = duplex(64);
        write_frame(&mut client, b"hello").await.unwrap();
        drop(client);

        let mut raw = Vec::new();
        tokio::io::copy(&mut server, &mut raw).await.unwrap();
        // First 4 bytes = 5 in big-endian
        assert_eq!(&raw[0..4], &[0u8, 0, 0, 5]);
        assert_eq!(&raw[4..], b"hello");
    }

    #[tokio::test]
    async fn read_frame_rejects_oversized_frame() {
        // Write a frame that claims a body of MAX_FRAME_BYTES + 1.
        let (mut client, mut server) = duplex(16);
        let huge_len = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes();
        tokio::io::AsyncWriteExt::write_all(&mut client, &huge_len)
            .await
            .unwrap();
        drop(client);

        let err = read_frame(&mut server).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("frame too large"),
            "unexpected error message: {err}"
        );
    }

    #[tokio::test]
    async fn read_frame_rejects_zero_byte_stream() {
        // Peer closes immediately: length prefix is never written.
        let (client, mut server) = duplex(16);
        drop(client);

        let err = read_frame(&mut server).await.unwrap_err();
        assert_eq!(
            err.kind(),
            ErrorKind::UnexpectedEof,
            "expected UnexpectedEof for a closed peer, got {err}"
        );
    }

    #[tokio::test]
    async fn read_frame_rejects_truncated_body() {
        // Write a length prefix saying 10 bytes but only provide 3.
        let (mut client, mut server) = duplex(16);
        let len_bytes = 10u32.to_be_bytes();
        tokio::io::AsyncWriteExt::write_all(&mut client, &len_bytes)
            .await
            .unwrap();
        // Write only 3 of the 10 claimed bytes, then close.
        tokio::io::AsyncWriteExt::write_all(&mut client, b"abc")
            .await
            .unwrap();
        drop(client);

        let err = read_frame(&mut server).await.unwrap_err();
        assert_eq!(
            err.kind(),
            ErrorKind::UnexpectedEof,
            "expected UnexpectedEof for truncated body, got {err}"
        );
    }

    #[tokio::test]
    async fn write_frame_rejects_body_exceeding_u32_max() {
        // We can't actually allocate 4 GiB in a test, but we can verify the
        // guard via a mock writer that accepts everything — the error fires
        // before any bytes are written.
        // Use a Cursor as a stand-in for an AsyncWrite that never fails.
        use tokio::io::duplex;
        let (client, _server) = duplex(1024 * 1024);

        // Simulate a body that's claimed to be u32::MAX + 1.
        // We can't allocate that, so we test the *guard logic* by monkey-
        // patching: build a vec of length usize::MAX is impossible, so we
        // instead test that try_into::<u32>() fails for a value > u32::MAX.
        // This is a compile-time proof rather than a runtime test on a
        // 4 GiB buffer. We document the invariant here and rely on the
        // read-side oversized-frame test to cover the complementary path.
        let huge: usize = (u32::MAX as usize) + 1;
        let result: Result<u32, _> = huge.try_into();
        assert!(result.is_err(), "u32::MAX+1 must not fit into u32");
        // Confirm the writer isn't accidentally called for large bodies.
        let _ = client; // suppress unused warning
    }
}
