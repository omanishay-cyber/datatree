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
use std::io::Write;
use std::path::PathBuf;
use tracing::warn;

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
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

    let tool = choose(args.tool, stdin_tool, String::new());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());

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

    let client = make_client(socket_override);
    if let Err(e) = client
        .request(IpcRequest::PostTool {
            tool,
            result_file,
            session_id,
        })
        .await
    {
        // Non-blocking: if the supervisor is down, we shouldn't fail the
        // hook. Just log and exit 0 so the host moves on.
        warn!(error = %e, "post-tool capture skipped (supervisor unreachable)");
    }
    Ok(())
}

/// Best-effort temp file for the tool's JSON response. Returns a sentinel
/// empty path on failure — the supervisor treats it as "no result to
/// record" (which is the right behavior when this hook can't write).
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
    let handle = std::fs::File::create(&file);
    if let Ok(mut f) = handle {
        let _ = f.write_all(&bytes);
    }
    file
}
