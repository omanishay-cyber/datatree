//! `mneme session-prime` — SessionStart hook entry point.
//!
//! Claude Code calls this when a new session starts. We respond with the
//! initial primer block (recent decisions, open todos, last drift findings)
//! and the resumption bundle if a Step Ledger task is mid-flight.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code SessionStart payload:
//! ```json
//! { "session_id": "...", "hook_event_name": "SessionStart",
//!   "source": "startup" | "resume" | "clear" | "compact",
//!   "cwd": "...", "transcript_path": "..." }
//! ```
//!
//! Project is derived from `cwd` when Claude Code doesn't send an
//! explicit project path — which it doesn't. The old `--project` CLI
//! flag is still accepted for manual testing.

use clap::Args;
use serde_json::json;
use std::path::PathBuf;
use tracing::warn;

use crate::error::CliResult;
use crate::hook_payload::{
    choose, make_hook_client, read_stdin_payload, resolved_session_id, HOOK_CTX_BUDGET,
    HOOK_IPC_BUDGET,
};
use crate::hook_writer::HookCtx;
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `mneme session-prime`. All optional — STDIN fills in.
#[derive(Debug, Args)]
pub struct SessionPrimeArgs {
    /// Active project root. If absent, resolved from STDIN `cwd` or
    /// process CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// Session id assigned by the host.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: SessionPrimeArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "session-prime STDIN parse failed; falling back to defaults");
            None
        }
    };

    // Prefer explicit --project; else STDIN `cwd` (where the user
    // launched Claude Code); else process CWD.
    let stdin_cwd = stdin_payload.as_ref().and_then(|p| p.cwd.clone());
    let project = choose(
        args.project,
        stdin_cwd,
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );

    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());

    // HOOK-CANCELLED-001 layer 1: short-lived host invocations may fire
    // SessionStart with no session id. The session-prime contract is to
    // emit a JSON `additional_context` payload back to Claude Code; with
    // no session to prime, we emit an empty block and exit Ok promptly.
    let session_id = match resolved_session_id(args.session_id, stdin_session) {
        Some(s) => s,
        None => {
            tracing::debug!(
                "session-prime fired without a session id; emitting empty context"
            );
            let out = json!({
                "hookEventName": "SessionStart",
                "additional_context": "",
            });
            println!("{}", serde_json::to_string(&out)?);
            return Ok(());
        }
    };
    let stdin_source = stdin_payload
        .as_ref()
        .and_then(|p| p.source.clone())
        .unwrap_or_else(|| "startup".to_string());

    // Bucket B4 fix: seed a session-start row in history.turns so the
    // session boundary is queryable. Without this row, history.db has
    // no anchor for "when did session X start?". Failure is non-fatal
    // (RULE 17 — never block the user).
    //
    // HOOK-CANCELLED-001 layer 2: bound `HookCtx::resolve` so first-time
    // shard creation can't push past Claude Code's hook budget.
    let cwd = project.clone();
    match tokio::time::timeout(HOOK_CTX_BUDGET, HookCtx::resolve(&cwd)).await {
        Ok(Ok(ctx)) => {
            let summary = format!("Session start ({stdin_source})");
            if let Err(e) = ctx
                .write_turn(&session_id, "session_start", &summary)
                .await
            {
                warn!(error = %e, "history.turns (session-start) insert failed (non-fatal)");
            }
        }
        Ok(Err(e)) => {
            warn!(error = %e, "hook ctx resolve failed; skipping session-start seed");
        }
        Err(_) => {
            warn!(
                budget_ms = HOOK_CTX_BUDGET.as_millis() as u64,
                "hook ctx resolve exceeded budget; skipping session-start seed"
            );
        }
    }

    // HOOK-CANCELLED-001 layer 2 + Bug E (the resurrection-loop killer,
    // 2026-04-29): use the no-autospawn hook client. Daemon down on
    // SessionStart ⇒ silent no-op (empty additional_context emitted
    // below). The user runs `mneme daemon start` to opt in.
    let client = make_hook_client(socket_override);
    let ipc_call = client.request(IpcRequest::SessionPrime {
        project,
        session_id,
    });
    let response = match tokio::time::timeout(HOOK_IPC_BUDGET, ipc_call).await {
        Ok(r) => r,
        Err(_) => {
            warn!(
                budget_ms = HOOK_IPC_BUDGET.as_millis() as u64,
                "session-prime IPC exceeded hook budget; emitting empty context"
            );
            Err(crate::error::CliError::Ipc("hook budget exceeded".into()))
        }
    };

    let payload = match response {
        Ok(IpcResponse::Ok { message }) => message.unwrap_or_default(),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "supervisor returned error");
            String::new()
        }
        Ok(IpcResponse::Pong)
        | Ok(IpcResponse::Status { .. })
        | Ok(IpcResponse::Logs { .. })
        | Ok(IpcResponse::JobQueued { .. })
        | Ok(IpcResponse::JobQueue { .. })
        | Ok(IpcResponse::RecallResults { .. })
        | Ok(IpcResponse::BlastResults { .. })
        | Ok(IpcResponse::GodNodesResults { .. }) => String::new(),
        Ok(_) => String::new(),
        Err(e) => {
            warn!(error = %e, "supervisor unreachable");
            String::new()
        }
    };

    let out = json!({
        "hookEventName": "SessionStart",
        "additional_context": payload,
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
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
        args: SessionPrimeArgs,
    }

    #[test]
    fn session_prime_args_parse_with_no_flags() {
        // Both optional — STDIN fills in `cwd` and session_id at runtime.
        let h = Harness::try_parse_from(["x"]).unwrap();
        assert!(h.args.project.is_none());
        assert!(h.args.session_id.is_none());
    }

    #[test]
    fn session_prime_args_parse_with_all_flags() {
        let h = Harness::try_parse_from([
            "x",
            "--project", "/tmp/p",
            "--session-id", "s-1",
        ]).unwrap();
        assert!(h.args.project.is_some());
        assert_eq!(h.args.session_id.as_deref(), Some("s-1"));
    }

    #[test]
    fn session_prime_args_parse_with_only_project() {
        // --project alone, no session-id — both flags must be independent.
        let h = Harness::try_parse_from(["x", "--project", "/tmp/p"]).unwrap();
        assert_eq!(h.args.project.as_ref().unwrap(), &PathBuf::from("/tmp/p"));
        assert!(h.args.session_id.is_none());
    }

    #[test]
    fn session_prime_args_parse_with_only_session_id() {
        // --session-id alone, no project — both flags must be independent.
        let h = Harness::try_parse_from(["x", "--session-id", "s-2"]).unwrap();
        assert!(h.args.project.is_none());
        assert_eq!(h.args.session_id.as_deref(), Some("s-2"));
    }

    #[test]
    fn session_prime_args_parser_rejects_unknown_flag() {
        // Unknown flag must fail at parse time so STDIN-fed garbage
        // doesn't get silently accepted.
        let r = Harness::try_parse_from(["x", "--bogus", "1"]);
        assert!(r.is_err(), "unknown flag must be rejected");
    }

    /// Test isolation helper. See session_end.rs for rationale.
    fn cwd_into_marker_free_tempdir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");
        dir
    }

    /// HOOK-CANCELLED-001: session-prime with no session id must exit Ok
    /// promptly with an empty additional_context block.
    #[tokio::test]
    async fn session_prime_with_empty_stdin_exits_zero() {
        let _keep = cwd_into_marker_free_tempdir();
        let start = std::time::Instant::now();
        let args = SessionPrimeArgs {
            project: None,
            session_id: None,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme.sock"))).await;
        let elapsed = start.elapsed();
        assert!(r.is_ok(), "session-prime with no session must exit Ok; got: {r:?}");
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "session-prime no-op path must be effectively instant; took {elapsed:?}"
        );
    }
}
