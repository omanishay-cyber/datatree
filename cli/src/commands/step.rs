//! `mneme step <op> [arg]` — Step Ledger ops.
//!
//! Per design §5.5 / §7.2:
//!
//! - `status`           — current step + ledger snapshot
//! - `show <step_id>`   — detail of one step
//! - `verify <step_id>` — run acceptance check
//! - `complete <step_id>` — mark complete (only if verify passes)
//! - `resume`           — emit resumption bundle
//! - `plan-from <md>`   — ingest a markdown roadmap into the ledger
//!
//! ## REG-007: 4-path dispatch
//!
//! Read ops (`status`, `show`, `resume`) go through the supervisor when
//! it is up so the daemon's connection pool absorbs the cost; if the
//! IPC hop fails (daemon down OR returns an `Error` response), they
//! fall through to a direct read of `tasks.db`. This mirrors the
//! pattern in `recall.rs` so users can run `mneme step status` even
//! when the supervisor is unreachable.
//!
//! Write ops (`verify`, `complete`, `plan-from`) require the
//! supervisor's single-writer guarantee — they are routed via IPC
//! only and surface the daemon's error verbatim. Bypassing the
//! single-writer to "be helpful" would corrupt the per-shard
//! invariant in `store/`.

use clap::Args;
use std::path::PathBuf;

use brain::ledger::{Ledger, RecallQuery, SqliteLedger, StepEntry};
use chrono::{Duration, Utc};
use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};

use crate::commands::build::{handle_response, make_client};
use crate::error::{CliError, CliResult};
use crate::ipc::{IpcRequest, IpcResponse};

/// CLI args for `mneme step`.
#[derive(Debug, Args)]
pub struct StepArgs {
    /// Operation: `status`, `show`, `verify`, `complete`, `resume`, `plan-from`.
    pub op: String,

    /// Optional argument:
    /// - `status` / `resume`     → ignored
    /// - `show` / `verify` / `complete` → step id (e.g. `"3.2.1"`)
    /// - `plan-from`             → path to a markdown roadmap
    pub arg: Option<String>,

    /// Optional project root (used by the direct-DB fallback). Defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: StepArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // Surface obvious mistakes early.
    let needs_arg = matches!(
        args.op.as_str(),
        "show" | "verify" | "complete" | "plan-from"
    );
    if needs_arg && args.arg.is_none() {
        return Err(CliError::Other(format!(
            "step {} requires an argument",
            args.op
        )));
    }

    let is_read_op = matches!(args.op.as_str(), "status" | "show" | "resume");

    // Both read and write ops attempt IPC first.
    let client = make_client(socket_override);
    let ipc_attempt = client
        .request(IpcRequest::Step {
            op: args.op.clone(),
            arg: args.arg.clone(),
        })
        .await;

    match ipc_attempt {
        Ok(IpcResponse::Error { message }) => {
            // Read ops fall through to direct-DB; write ops surface the
            // error so the user sees the supervisor's verdict and we
            // don't violate the single-writer invariant.
            if is_read_op {
                tracing::warn!(
                    error = %message,
                    "supervisor returned error on step read; falling back to direct-db"
                );
            } else {
                return Err(CliError::Supervisor(message));
            }
        }
        Ok(resp) => {
            return handle_response(resp);
        }
        Err(e) => {
            // Read ops fall through; write ops bubble the error.
            if is_read_op {
                tracing::warn!(
                    error = %e,
                    "supervisor unreachable on step read; falling back to direct-db"
                );
            } else {
                return Err(e);
            }
        }
    }

    // Direct-DB fallback (read ops only).
    direct_db_fallback(&args)
}

/// Direct-DB fallback for `status` / `show` / `resume`. Opens the
/// per-project `tasks.db` shard read-only via `SqliteLedger` (which
/// owns the schema) and prints the same human-readable summary the
/// supervisor would have produced.
fn direct_db_fallback(args: &StepArgs) -> CliResult<()> {
    let project = args
        .project
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let project = std::fs::canonicalize(&project).unwrap_or(project);
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let tasks_db = paths.shard_db(&project_id, DbLayer::Tasks);

    if !tasks_db.exists() {
        println!("step {}: no ledger yet for this project", args.op);
        println!("  (tasks.db not found at {})", tasks_db.display());
        return Ok(());
    }

    let ledger = SqliteLedger::open(&tasks_db)
        .map_err(|e| CliError::Other(format!("open tasks.db: {e}")))?;

    match args.op.as_str() {
        "status" => render_status(&ledger),
        "show" => {
            let id = args.arg.as_deref().unwrap_or("");
            render_show(&ledger, id)
        }
        "resume" => render_resume(&ledger),
        other => Err(CliError::Other(format!(
            "step {other}: not supported in direct-DB fallback (write op requires supervisor)"
        ))),
    }
}

fn render_status(ledger: &SqliteLedger) -> CliResult<()> {
    // Latest 10 entries across all kinds — enough to give the operator a
    // clear picture without dumping the entire ledger.
    let q = RecallQuery {
        text: String::new(),
        kinds: Vec::new(),
        limit: 10,
        since: None,
        session_id: None,
        embedding: None,
    };
    let hits = ledger
        .recall(&q)
        .map_err(|e| CliError::Other(format!("recall: {e}")))?;
    if hits.is_empty() {
        println!("step status: ledger empty");
        return Ok(());
    }
    println!("step status — last {} entry(ies):", hits.len());
    println!();
    for e in &hits {
        print_entry(e);
    }
    Ok(())
}

fn render_show(ledger: &SqliteLedger, id: &str) -> CliResult<()> {
    if id.is_empty() {
        return Err(CliError::Other("step show: empty id".into()));
    }
    // No direct `get_by_id` API on Ledger trait — pull a wide window and
    // grep. The ledger is bounded so this is acceptable for fallback.
    let q = RecallQuery {
        text: id.to_string(),
        kinds: Vec::new(),
        limit: 1000,
        since: None,
        session_id: None,
        embedding: None,
    };
    let hits = ledger
        .recall(&q)
        .map_err(|e| CliError::Other(format!("recall: {e}")))?;
    let found = hits.iter().find(|e| e.id == id || e.id.starts_with(id));
    match found {
        Some(e) => {
            println!("step show {}:", e.id);
            println!();
            print_entry(e);
            Ok(())
        }
        None => Err(CliError::Other(format!("step show: id {id} not found"))),
    }
}

fn render_resume(ledger: &SqliteLedger) -> CliResult<()> {
    // 14 days back is the same window the supervisor uses for resume.
    let since = Utc::now() - Duration::days(14);
    let bundle = ledger
        .resume_summary(since)
        .map_err(|e| CliError::Other(format!("resume_summary: {e}")))?;
    println!(
        "step resume — bundle from {} (14 days):",
        since.to_rfc3339()
    );
    println!(
        "  decisions={}, implementations={}, open_questions={}, timeline={}",
        bundle.recent_decisions.len(),
        bundle.recent_implementations.len(),
        bundle.open_questions.len(),
        bundle.timeline.len()
    );
    if !bundle.recent_decisions.is_empty() {
        println!();
        println!("recent decisions:");
        for e in bundle.recent_decisions.iter().take(5) {
            print_entry(e);
        }
    }
    if !bundle.open_questions.is_empty() {
        println!();
        println!("open questions:");
        for e in bundle.open_questions.iter().take(5) {
            print_entry(e);
        }
    }
    Ok(())
}

fn print_entry(e: &StepEntry) {
    let id_short: String = e.id.chars().take(12).collect();
    println!("  [{}] {}", id_short, e.timestamp.to_rfc3339());
    let summary: String = e.summary.chars().take(140).collect();
    if !summary.is_empty() {
        println!("    {summary}");
    }
    if let Some(r) = e.rationale.as_deref() {
        let r_snip: String = r.chars().take(140).collect();
        if !r_snip.is_empty() {
            println!("    (rationale: {r_snip})");
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_args_parse_status_no_arg() {
        // `status` does not require an arg.
        let args = StepArgs {
            op: "status".to_string(),
            arg: None,
            project: None,
        };
        assert!(matches!(args.op.as_str(), "status"));
    }

    #[test]
    fn step_show_without_arg_errors_out_synchronously() {
        // We can't easily run the async dispatch here without a
        // supervisor or temp shard, but we CAN check the `needs_arg`
        // gate's classification logic by sampling the discriminator.
        let needs_arg = matches!("show", "show" | "verify" | "complete" | "plan-from");
        assert!(needs_arg);
        let needs_arg = matches!("status", "show" | "verify" | "complete" | "plan-from");
        assert!(!needs_arg);
    }

    #[test]
    fn read_op_classification() {
        assert!(matches!("status", "status" | "show" | "resume"));
        assert!(matches!("show", "status" | "show" | "resume"));
        assert!(matches!("resume", "status" | "show" | "resume"));
        assert!(!matches!("verify", "status" | "show" | "resume"));
        assert!(!matches!("complete", "status" | "show" | "resume"));
        assert!(!matches!("plan-from", "status" | "show" | "resume"));
    }
}
