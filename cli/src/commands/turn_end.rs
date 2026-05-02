//! `mneme turn-end` — Stop hook entry point.
//!
//! Triggers the summarizer and updates the Step Ledger drift score.
//! The optional flags let the same handler be reused for PreCompact (so
//! we can flush the ledger to disk before compaction) and SubagentStop
//! (so per-subagent turns are accounted separately).
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code Stop / SubagentStop / PreCompact events all deliver:
//! ```json
//! { "session_id": "...", "hook_event_name": "Stop",
//!   "stop_hook_active": true|false }
//! ```
//!
//! Stop-hook failures ALWAYS exit 0 with a stderr warning. Claude Code
//! retries Stop hooks up to 5× on non-zero exit (see report-002.md §F-016
//! / §9.3 R5); on v0.3.0 this amplified the self-trap into a ~20-round
//! retry loop. v0.3.1's rule: a broken turn-end never gates the user's
//! reply. Log the problem, exit 0.
//!
//! We also honor `stop_hook_active=true` as an immediate no-op — that
//! flag means a previous turn-end already emitted `decision: "block"`;
//! firing again would loop forever.

use clap::Args;
use std::path::PathBuf;
use tracing::warn;

use crate::error::CliResult;
use crate::hook_payload::{
    make_hook_client, read_stdin_payload, resolved_session_id, HOOK_CTX_BUDGET, HOOK_IPC_BUDGET,
};
use crate::hook_writer::HookCtx;
use crate::ipc::IpcRequest;

/// CLI args for `mneme turn-end`.
#[derive(Debug, Args)]
pub struct TurnEndArgs {
    /// Session id.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,

    /// This hook fired right before a compaction event.
    #[arg(long = "pre-compact")]
    pub pre_compact: bool,

    /// This hook fired at the end of a subagent turn rather than the main
    /// model.
    #[arg(long)]
    pub subagent: bool,
}

/// Entry point used by `main.rs`. Always exits 0.
pub async fn run(args: TurnEndArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "turn-end STDIN parse failed; falling back to CLI flags");
            None
        }
    };

    // Short-circuit loops per Claude Code's retry contract.
    if let Some(p) = stdin_payload.as_ref() {
        if p.stop_hook_active.unwrap_or(false) {
            // Previous turn-end already blocked; re-running would loop.
            return Ok(());
        }
    }

    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    let stdin_event = stdin_payload
        .as_ref()
        .and_then(|p| p.hook_event_name.clone())
        .unwrap_or_default();
    // The payload's hook_event_name lets STDIN-only invocations distinguish
    // Stop / SubagentStop / PreCompact without extra CLI flags.
    let pre_compact_from_stdin = stdin_event == "PreCompact";
    let subagent_from_stdin = stdin_event == "SubagentStop";
    let pre_compact = args.pre_compact || pre_compact_from_stdin;
    let subagent = args.subagent || subagent_from_stdin;

    // HOOK-CANCELLED-001 layer 1: short-lived host commands (e.g.
    // `claude mcp list`) may fire Stop with no session id; persisting a
    // turn-boundary row keyed on a placeholder string would be junk and
    // doing the work would push the hook past Claude Code's budget.
    // Skip every side effect when no real session id arrived.
    let session_id = match resolved_session_id(args.session_id, stdin_session) {
        Some(s) => s,
        None => {
            tracing::debug!("turn-end fired without a session id; exiting 0 with no work");
            return Ok(());
        }
    };

    let suffix = match (pre_compact, subagent) {
        (true, _) => ":pre-compact",
        (_, true) => ":subagent",
        _ => "",
    };
    let session_id_qualified = format!("{}{}", session_id, suffix);

    // Bucket B4 fix: persist BOTH a tasks.db ledger row (decision-kind
    // session marker) AND a history.db turn row (role='session_end' /
    // 'pre_compact' / 'subagent_end') so the Stop hook actually leaves
    // a trail. Without these writes, tasks.db and the second half of
    // history.db were permanently empty.
    //
    // The "decision" row carries enough context for `recall_decision`
    // to surface a session boundary; per-turn distillation of an actual
    // human decision is the brain crate's job (out of scope here).
    //
    // HOOK-CANCELLED-001 layer 2: bound `HookCtx::resolve` so first-time
    // shard creation can't push past Claude Code's hook budget.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    match tokio::time::timeout(HOOK_CTX_BUDGET, HookCtx::resolve(&cwd)).await {
        Ok(Ok(ctx)) => {
            let kind_label = match (pre_compact, subagent) {
                (true, _) => "pre_compact",
                (_, true) => "subagent_end",
                _ => "session_end",
            };
            let summary = format!("Turn boundary: {kind_label}");
            // Bug G-12 (2026-05-01): hook errors used to only go to
            // tracing — Claude Code never saw them because the hook
            // contract is "always exit 0" and host harnesses discard
            // hook stderr. We now ALSO emit a one-line structured JSON
            // to stderr so any harness that captures hook stderr (or a
            // human running the hook manually for diagnosis) can see
            // the failure. Tracing copy preserved for supervisor.log.
            if let Err(e) = ctx
                .write_ledger_entry(
                    &session_id_qualified,
                    "decision",
                    &summary,
                    Some("Auto-emitted by Stop hook (mneme turn-end)."),
                )
                .await
            {
                warn!(error = %e, "tasks.ledger_entries insert failed (non-fatal)");
                eprintln!(
                    "{{\"hook\":\"turn_end\",\"error\":\"ledger_entries.insert\",\"detail\":\"{}\"}}",
                    e.to_string().replace('"', "\\\"")
                );
            }
            if let Err(e) = ctx
                .write_turn(&session_id_qualified, kind_label, &summary)
                .await
            {
                warn!(error = %e, "history.turns (session-end) insert failed (non-fatal)");
                eprintln!(
                    "{{\"hook\":\"turn_end\",\"error\":\"history.turns.insert\",\"detail\":\"{}\"}}",
                    e.to_string().replace('"', "\\\"")
                );
            }

            // I1 batch 3 — agents.db::subagent_runs producer.
            // SubagentStop fires once per Task / subagent invocation;
            // capture it as a row so the agents shard (previously
            // empty in the cycle-3 build) reflects real usage.
            if subagent {
                if let Err(e) = ctx
                    .write_subagent_run(
                        &session_id_qualified,
                        // The Claude Code SubagentStop payload doesn't
                        // currently carry the dispatched agent name,
                        // so we record `unknown` and let downstream
                        // tooling overwrite via richer payloads later.
                        "unknown",
                        "completed",
                        Some(&summary),
                    )
                    .await
                {
                    warn!(error = %e, "agents.subagent_runs insert failed (non-fatal)");
                }
            }
        }
        Ok(Err(e)) => {
            warn!(error = %e, "hook ctx resolve failed; skipping turn-end persistence");
        }
        Err(_) => {
            warn!(
                budget_ms = HOOK_CTX_BUDGET.as_millis() as u64,
                "hook ctx resolve exceeded budget; skipping turn-end persistence"
            );
        }
    }

    // HOOK-CANCELLED-001 layer 2 + Bug E (the resurrection-loop killer,
    // 2026-04-29): use the no-autospawn hook client. Daemon down ⇒ silent
    // no-op. No daemon resurrection on every Stop / SubagentStop / PreCompact.
    let client = make_hook_client(socket_override);
    let ipc_call = client.request(IpcRequest::TurnEnd {
        session_id: session_id_qualified,
    });
    match tokio::time::timeout(HOOK_IPC_BUDGET, ipc_call).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            warn!(error = %e, "turn-end notification skipped (supervisor unreachable)");
        }
        Err(_) => {
            warn!(
                budget_ms = HOOK_IPC_BUDGET.as_millis() as u64,
                "turn-end IPC exceeded hook budget; skipping notification"
            );
        }
    }
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
        args: TurnEndArgs,
    }

    #[test]
    fn turn_end_args_parse_with_no_flags() {
        // Default: regular Stop, not pre-compact, not subagent.
        let h = Harness::try_parse_from(["x"]).unwrap();
        assert!(h.args.session_id.is_none());
        assert!(!h.args.pre_compact);
        assert!(!h.args.subagent);
    }

    #[test]
    fn turn_end_args_parse_with_pre_compact_flag() {
        let h = Harness::try_parse_from(["x", "--session-id", "s-7", "--pre-compact"]).unwrap();
        assert_eq!(h.args.session_id.as_deref(), Some("s-7"));
        assert!(h.args.pre_compact);
    }

    #[test]
    fn turn_end_args_parse_with_subagent_flag() {
        let h = Harness::try_parse_from(["x", "--session-id", "s-9", "--subagent"]).unwrap();
        assert!(h.args.subagent);
    }

    /// Test isolation helper. See session_end.rs for rationale.
    fn cwd_into_marker_free_tempdir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(dir.path()).expect("set_current_dir");
        dir
    }

    /// HOOK-CANCELLED-001: turn-end with no session id must exit Ok with
    /// no side effects. Same contract as session-end — a Stop hook that
    /// fires for a non-session host invocation MUST return promptly.
    #[tokio::test]
    async fn turn_end_with_empty_stdin_exits_zero() {
        let _keep = cwd_into_marker_free_tempdir();
        let start = std::time::Instant::now();
        let args = TurnEndArgs {
            session_id: None,
            pre_compact: false,
            subagent: false,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme.sock"))).await;
        let elapsed = start.elapsed();
        assert!(
            r.is_ok(),
            "turn-end with no session id must exit Ok; got: {r:?}"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "turn-end with no session id must be effectively instant; took {elapsed:?}"
        );
    }

    /// HOOK-CANCELLED-001: turn-end with a session id MUST complete within
    /// the hook budget even when the supervisor is unreachable.
    #[tokio::test]
    async fn turn_end_completes_within_hook_budget_when_supervisor_unreachable() {
        let _keep = cwd_into_marker_free_tempdir();
        let start = std::time::Instant::now();
        let args = TurnEndArgs {
            session_id: None,
            pre_compact: false,
            subagent: false,
        };
        let r = run(args, Some(PathBuf::from("/nope-mneme.sock"))).await;
        let elapsed = start.elapsed();
        assert!(r.is_ok(), "turn-end must always exit Ok; got: {r:?}");
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "turn-end must complete within 3s when supervisor is unreachable; \
             took {elapsed:?}"
        );
    }
}
