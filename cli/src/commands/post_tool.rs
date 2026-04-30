//! `mneme post-tool` — PostToolUse hook entry point.
//!
//! Fire-and-forget capture (per design §6.4). We send the request to the
//! supervisor but don't wait for its response — the host doesn't read
//! anything from this hook's stdout.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code PostToolUse delivers `tool_response` as an object on STDIN.
//! We persist the full response to a temp file and hand the path to the
//! supervisor (the existing IPC contract expects a file path — keeping
//! the payload out of the hot IPC wire).
//!
//! Exits 0 on every path. PostToolUse blocking would delay the next tool
//! call pointlessly.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::error::CliResult;
use crate::hook_payload::{
    choose, make_hook_client, read_stdin_payload, HOOK_CTX_BUDGET, HOOK_IPC_BUDGET,
};
use crate::hook_writer::HookCtx;
use crate::ipc::IpcRequest;

/// CLI args for `mneme post-tool`. All optional — STDIN fills in.
#[derive(Debug, Args)]
pub struct PostToolArgs {
    /// Tool name that ran.
    #[arg(long)]
    pub tool: Option<String>,

    /// Path to the file holding the tool's serialized result. When
    /// Claude Code invokes us it passes the result inline via
    /// `tool_response`; we spool that to a temp file and fill this
    /// field automatically.
    #[arg(long = "result-file")]
    pub result_file: Option<PathBuf>,

    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: PostToolArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "post-tool STDIN parse failed; continuing with CLI flags / defaults");
            None
        }
    };

    let stdin_tool = stdin_payload.as_ref().and_then(|p| p.tool_name.clone());
    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    // Capture tool_input + tool_response BEFORE we move stdin_payload's
    // option fields into the result-file spool. The hook-side writer
    // needs both the params (for the params_hash key) and the response.
    let stdin_tool_input = stdin_payload
        .as_ref()
        .and_then(|p| p.tool_input.as_ref())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".to_string());
    let stdin_tool_response = stdin_payload
        .as_ref()
        .and_then(|p| p.tool_response.as_ref())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string());

    let tool = choose(args.tool, stdin_tool, String::new());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());

    // HOOK-CANCELLED-001 layer 1: short-lived host invocations may fire
    // PostToolUse with no tool name. With nothing to record we exit Ok
    // promptly so Claude Code never cancels us for budget overrun.
    if tool.trim().is_empty() {
        tracing::debug!("post-tool fired without a tool name; exiting 0 with no work");
        return Ok(());
    }

    // Bucket B4 fix: persist this tool call directly to tool_cache.db.
    // Without this write tool_cache.db stayed empty even on tool-heavy
    // sessions — the supervisor's ControlCommand has no PostTool variant,
    // so the IPC below silently round-trips an Error response.
    //
    // HOOK-CANCELLED-001 layer 2: bound `HookCtx::resolve` so first-time
    // shard creation can't push past Claude Code's hook budget.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    match tokio::time::timeout(HOOK_CTX_BUDGET, HookCtx::resolve(&cwd)).await {
        Ok(Ok(ctx)) => {
            if let Err(e) = ctx
                .write_tool_call(&session_id, &tool, &stdin_tool_input, &stdin_tool_response)
                .await
            {
                warn!(error = %e, "tool_cache.tool_calls insert failed (non-fatal)");
            }
        }
        Ok(Err(e)) => {
            warn!(error = %e, "hook ctx resolve failed; skipping tool_cache write");
        }
        Err(_) => {
            warn!(
                budget_ms = HOOK_CTX_BUDGET.as_millis() as u64,
                "hook ctx resolve exceeded budget; skipping tool_cache write"
            );
        }
    }

    // Resolve the result-file: prefer explicit CLI flag, then STDIN
    // `tool_response` (spool it to a temp file), else a sentinel empty
    // file so the IPC contract is satisfied.
    let result_file = match args.result_file {
        Some(p) => p,
        None => {
            let stdin_response = stdin_payload
                .as_ref()
                .and_then(|p| p.tool_response.as_ref());
            match stdin_response {
                Some(val) => spool_to_temp(&session_id, val),
                None => spool_to_temp(&session_id, &serde_json::Value::Null),
            }
        }
    };

    // HOOK-CANCELLED-001 layer 2 + Bug E (the resurrection-loop killer,
    // 2026-04-29): use the no-autospawn hook client. Supervisor down ⇒
    // hook silently no-ops. The post-tool capture is a best-effort write
    // to history.db; missing it is fine, but resurrecting a daemon every
    // tool call is not.
    let client = make_hook_client(socket_override);
    let ipc_call = client.request(IpcRequest::PostTool {
        tool,
        result_file,
        session_id,
    });
    match tokio::time::timeout(HOOK_IPC_BUDGET, ipc_call).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            // Non-blocking: if the supervisor is down, we shouldn't fail the
            // hook. Just log and exit 0 so the host moves on.
            warn!(error = %e, "post-tool capture skipped (supervisor unreachable)");
        }
        Err(_) => {
            warn!(
                budget_ms = HOOK_IPC_BUDGET.as_millis() as u64,
                "post-tool IPC exceeded hook budget; skipping capture"
            );
        }
    }
    Ok(())
}

/// Best-effort temp file for the tool's JSON response. Returns a sentinel
/// empty path on failure — the supervisor treats it as "no result to
/// record" (which is the right behavior when this hook can't write).
///
/// Silent-3 fix (Class H-silent in `docs/dev/DEEP-AUDIT-2026-04-29.md`):
/// the previous version did `let _ = f.write_all(&bytes)` and silently
/// returned the path even when no bytes landed on disk — the supervisor
/// would then read an empty file and record "no result" for the tool
/// call. Now we capture the write error, log it at `debug` (so
/// production logs aren't spammed but operators running with
/// `RUST_LOG=mneme=debug` see it), and return the empty-path sentinel
/// — the documented "couldn't spool" signal. The hook itself still
/// exits 0 (Bug E policy: hooks-never-fail-the-host).
fn spool_to_temp(session_id: &str, value: &serde_json::Value) -> PathBuf {
    let dir = std::env::temp_dir().join("mneme-post-tool");
    if std::fs::create_dir_all(&dir).is_err() {
        return PathBuf::new();
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file = dir.join(format!("{session_id}-{stamp}.json"));
    let bytes = match serde_json::to_vec(value) {
        Ok(b) => b,
        Err(_) => b"null".to_vec(),
    };
    match std::fs::File::create(&file) {
        Ok(mut f) => {
            if let Err(e) = spool_payload(&mut f, &bytes) {
                tracing::debug!(
                    error = %e,
                    file = %file.display(),
                    "post-tool spool write failed; supervisor will see empty result"
                );
                return PathBuf::new();
            }
            file
        }
        Err(e) => {
            tracing::debug!(
                error = %e,
                file = %file.display(),
                "post-tool spool create failed; returning empty-path sentinel"
            );
            PathBuf::new()
        }
    }
}

/// Inner write step for `spool_to_temp`, isolated for unit testing
/// (we can pass any `Write` impl and assert error propagation without
/// touching the filesystem). Silent-3.
fn spool_payload<W: std::io::Write>(writer: &mut W, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Smoke clap harness — verify args parser without spinning up the
    /// full binary. WIRE-005: every command file gets at least one test.
    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: PostToolArgs,
    }

    #[test]
    fn post_tool_args_parse_with_no_flags() {
        // All optional — should parse with no args.
        let h = Harness::try_parse_from(["x"]).unwrap();
        assert!(h.args.tool.is_none());
        assert!(h.args.result_file.is_none());
        assert!(h.args.session_id.is_none());
    }

    #[test]
    fn post_tool_args_parse_with_all_flags() {
        let h = Harness::try_parse_from([
            "x",
            "--tool",
            "Bash",
            "--result-file",
            "/tmp/r.json",
            "--session-id",
            "s-123",
        ])
        .unwrap();
        assert_eq!(h.args.tool.as_deref(), Some("Bash"));
        assert_eq!(h.args.session_id.as_deref(), Some("s-123"));
    }

    #[test]
    fn spool_to_temp_writes_a_file_or_empty_path() {
        // Best-effort path: either we get a writable temp file, or an
        // empty PathBuf when the temp dir is unavailable. Either is a
        // valid post-condition; just exercise the function.
        let p = spool_to_temp("test-session", &serde_json::json!({"ok": true}));
        if !p.as_os_str().is_empty() {
            // If we did write, it should exist.
            assert!(
                p.exists(),
                "spool_to_temp returned non-empty path that doesn't exist: {}",
                p.display()
            );
            let _ = std::fs::remove_file(&p);
        }
    }

    /// Test isolation helper. See session_end.rs for rationale.
    fn cwd_into_marker_free_tempdir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");
        dir
    }

    /// HOOK-CANCELLED-001: post-tool with no tool name exits Ok promptly
    /// without doing any persistence or IPC.
    #[tokio::test]
    async fn post_tool_with_empty_stdin_exits_zero() {
        let _keep = cwd_into_marker_free_tempdir();
        let start = std::time::Instant::now();
        let args = PostToolArgs {
            tool: None,
            result_file: None,
            session_id: None,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme.sock"))).await;
        let elapsed = start.elapsed();
        assert!(r.is_ok(), "post-tool with no tool must exit Ok; got: {r:?}");
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "post-tool no-op path must be effectively instant; took {elapsed:?}"
        );
    }

    // ---- Silent-3: spool write failures must be captured ---------------

    /// A `std::io::Write` that always returns `BrokenPipe` — the
    /// minimum reproducible "write failed" condition that doesn't depend
    /// on platform-specific filesystem state (full disk, read-only fs,
    /// etc).
    struct FailingWriter;
    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic write failure for Silent-3 test",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// RED → GREEN. Silent-3 in `docs/dev/DEEP-AUDIT-2026-04-29.md`:
    /// the spool path used `let _ = f.write_all(&bytes)`, silently
    /// dropping write errors and returning the (empty) file path as
    /// if it had succeeded. The supervisor then read zero bytes and
    /// recorded "no result" — silent data loss.
    ///
    /// `spool_payload` isolates the write step so we can assert the
    /// error is surfaced (rather than swallowed). With `FailingWriter`
    /// the call must return Err — that's the contract `spool_to_temp`
    /// now relies on to log + return the empty-path sentinel.
    #[test]
    fn spool_payload_propagates_write_errors() {
        let mut w = FailingWriter;
        let r = spool_payload(&mut w, b"{\"hello\":\"world\"}");
        match r {
            Err(e) => assert_eq!(
                e.kind(),
                std::io::ErrorKind::BrokenPipe,
                "expected the synthetic BrokenPipe to surface; got {e:?}"
            ),
            Ok(()) => panic!("spool_payload must propagate write errors, not swallow them"),
        }
    }

    /// Sanity: when the writer accepts every byte, `spool_payload`
    /// returns Ok and the buffer ends up with the bytes. Guards
    /// against a regression where the wrapper accidentally always errors.
    #[test]
    fn spool_payload_writes_bytes_on_success() {
        let mut buf = Vec::<u8>::new();
        let r = spool_payload(&mut buf, b"abc");
        assert!(
            r.is_ok(),
            "spool_payload must succeed on a healthy writer; got {r:?}"
        );
        assert_eq!(buf, b"abc");
    }

    /// End-to-end Silent-3 contract: even when the supervisor cannot be
    /// reached AND the spool path is exercised, the hook still exits 0.
    /// This is the Bug E policy ("hooks never fail the host tool")
    /// re-asserted in the DEEP-AUDIT (see Silent-3 prescription:
    /// "post_tool MUST still exit 0 even on internal failure").
    #[tokio::test]
    async fn post_tool_exits_zero_when_supervisor_unreachable() {
        let _keep = cwd_into_marker_free_tempdir();
        let args = PostToolArgs {
            tool: Some("Bash".to_string()),
            result_file: None,
            session_id: Some("silent3-check".to_string()),
        };
        // Use a guaranteed-unreachable socket path — the IPC call will
        // time out / fail, and the spool path will run to completion.
        let r = run(args, Some(PathBuf::from("/nope-mneme-silent3.sock"))).await;
        assert!(
            r.is_ok(),
            "post-tool must exit Ok regardless of supervisor / spool state; got {r:?}"
        );
    }
}
