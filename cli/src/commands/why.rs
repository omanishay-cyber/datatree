//! `mneme why <question>` — F6 Why-Chain.
//!
//! Renders a decision trace in response to a natural-language "why did we
//! pick X?" style question. Blends three sources:
//!
//!   1. The Step Ledger (`brain::ledger`) — decisions + rationale + rejected
//!      alternatives, filtered to Decision / Refactor kinds.
//!   2. `git log --grep=...` — commits whose message mentions the query.
//!   3. The concept graph — concepts related to the ledger hits (via
//!      `brain::Concept`'s label match, done in-process).
//!
//! The output is a compact markdown blob printed to stdout so the caller
//! (Claude Code, a human shell, or another tool) can paste or re-ingest.

use clap::Args;
use std::path::PathBuf;
use std::process::Command;

use brain::ledger::{Ledger, RecallQuery, SqliteLedger, StepEntry, StepKind};
use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};

use crate::error::{CliError, CliResult};

/// CLI args for `mneme why`.
#[derive(Debug, Args)]
pub struct WhyArgs {
    /// Free-form question. The first positional argument forms the whole
    /// query so users can say `mneme why "did we pick SQLite"`.
    pub query: String,

    /// Optional project path; defaults to CWD.
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// Max ledger hits to include.
    #[arg(long, default_value_t = 6)]
    pub limit: usize,

    /// Max git commits to include.
    #[arg(long = "git-limit", default_value_t = 5)]
    pub git_limit: usize,
}

/// Entry point used by `main.rs`.
pub async fn cmd_why(args: WhyArgs, _socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = crate::commands::build::resolve_project(args.project)?;
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let tasks_db = paths.shard_db(&project_id, DbLayer::Tasks);

    // 1) Ledger — filter to Decision + Refactor (the "why" kinds).
    let ledger_hits: Vec<StepEntry> = match SqliteLedger::open(&tasks_db) {
        Ok(led) => led
            .recall(&RecallQuery {
                text: args.query.clone(),
                kinds: vec!["decision".into(), "refactor".into()],
                limit: args.limit,
                since: None,
                session_id: None,
                embedding: None,
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    // 2) git log --grep=... — stable fast-path; ignore failures.
    let git_hits = git_log_grep(&project, &args.query, args.git_limit).unwrap_or_default();

    // 3) Concepts — v0.2 placeholder: we emit the set of concept ids referenced
    // by the ledger hits. Real concept graph traversal lives in the brain
    // crate and will be wired once the cross-crate IPC path lands.
    let mut related_concepts: Vec<String> = ledger_hits
        .iter()
        .flat_map(|e| e.touched_concepts.iter().cloned())
        .collect();
    related_concepts.sort();
    related_concepts.dedup();

    // 4) Render.
    print_why_chain(&args.query, &ledger_hits, &git_hits, &related_concepts);
    Ok(())
}

/// One matching git commit.
#[derive(Debug, Clone)]
pub struct GitHit {
    pub sha: String,
    pub date: String,
    pub subject: String,
}

/// Run `git log --grep=<query> -n <limit>` against `project_root`. Returns
/// an empty vec if git isn't available or the repo isn't initialised.
pub fn git_log_grep(project_root: &std::path::Path, query: &str, limit: usize) -> Option<Vec<GitHit>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("log")
        .arg(format!("--grep={}", query))
        .arg(format!("-n{}", limit))
        .arg("--pretty=format:%H\t%ad\t%s")
        .arg("--date=short")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut hits = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let sha = parts.next().unwrap_or("").to_string();
        let date = parts.next().unwrap_or("").to_string();
        let subject = parts.next().unwrap_or("").to_string();
        if !sha.is_empty() {
            hits.push(GitHit { sha, date, subject });
        }
    }
    Some(hits)
}

fn print_why_chain(
    query: &str,
    ledger_hits: &[StepEntry],
    git_hits: &[GitHit],
    concepts: &[String],
) {
    println!("# why: {}", query);
    println!();

    if ledger_hits.is_empty() && git_hits.is_empty() {
        println!("_no matching decisions, refactors, or commits found._");
        return;
    }

    if !ledger_hits.is_empty() {
        println!("## decisions from the step ledger");
        for e in ledger_hits {
            println!();
            println!("### {}", short_id(&e.id));
            println!("- **summary**: {}", e.summary);
            println!("- **when**: {}", e.timestamp.to_rfc3339());
            if let Some(r) = &e.rationale {
                println!("- **rationale**: {}", r);
            }
            match &e.kind {
                StepKind::Decision { chosen, rejected } => {
                    println!("- **chosen**: {}", chosen);
                    if !rejected.is_empty() {
                        println!("- **rejected**: {}", rejected.join(", "));
                    }
                }
                StepKind::Refactor { before, after } => {
                    println!("- **before**: {}", before);
                    println!("- **after**: {}", after);
                }
                _ => {}
            }
            if !e.touched_files.is_empty() {
                println!(
                    "- **touched files**: {}",
                    e.touched_files
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }

    if !git_hits.is_empty() {
        println!();
        println!("## git commits mentioning the query");
        for g in git_hits {
            println!("- `{}` ({}) {}", &g.sha[..g.sha.len().min(10)], g.date, g.subject);
        }
    }

    if !concepts.is_empty() {
        println!();
        println!("## related concepts");
        for c in concepts {
            println!("- {}", c);
        }
    }
}

fn short_id(id: &str) -> &str {
    let end = id.len().min(12);
    &id[..end]
}
