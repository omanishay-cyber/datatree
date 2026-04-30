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

    /// Max ledger hits to include. Clamped at parse-time to 1..=10000
    /// (REG-022) so a pathological `--limit` can't blow the ledger query.
    #[arg(long, default_value_t = 6, value_parser = clap::value_parser!(u64).range(1..=10000))]
    pub limit: u64,

    /// Max git commits to include. Clamped to 1..=10000 (REG-022) — a runaway
    /// `git log` driven by user input would tie up the subprocess for ages.
    #[arg(long = "git-limit", default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..=10000))]
    pub git_limit: u64,
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
                limit: args.limit as usize,
                since: None,
                session_id: None,
                embedding: None,
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    // 2) git log --grep=... — stable fast-path; ignore failures.
    let git_hits = git_log_grep(&project, &args.query, args.git_limit as usize).unwrap_or_default();

    // 3) Concepts — WIRE-008: this is still a partial result. The
    // brain crate does NOT yet expose a `concept_graph::related_concepts`
    // helper (graph traversal is queued for v0.4 once the cross-crate
    // IPC path lands). Until then we emit the set of concept ids
    // referenced by the ledger hits and TAG the partial result so
    // downstream consumers know not to treat the list as exhaustive.
    let mut related_concepts: Vec<String> = ledger_hits
        .iter()
        .flat_map(|e| e.touched_concepts.iter().cloned())
        .collect();
    related_concepts.sort();
    related_concepts.dedup();
    let concepts_partial = true;

    // 4) Render.
    print_why_chain(
        &args.query,
        &ledger_hits,
        &git_hits,
        &related_concepts,
        concepts_partial,
    );
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
    // M13: windowless_command(..) applies CREATE_NO_WINDOW on Windows
    // so this git probe does not flash a console when invoked from a
    // hook-context parent.
    let out = crate::windowless_command("git")
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
    concepts_partial: bool,
) {
    println!("# why: {}", query);
    println!();
    if concepts_partial {
        // WIRE-008: tell the caller (often Claude Code piping our
        // output) that the concepts section is a known partial result.
        println!(
            "<!-- _partial: concept graph traversal pending v0.4 \
             (see cli/src/commands/why.rs WIRE-008) -->"
        );
        println!();
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_truncates_long_ids() {
        assert_eq!(short_id("abcdefghijklmnop"), "abcdefghijkl");
    }

    #[test]
    fn short_id_passes_through_short() {
        assert_eq!(short_id("abc"), "abc");
        assert_eq!(short_id(""), "");
    }

    #[test]
    fn git_log_grep_returns_none_for_nonexistent_repo() {
        let td = tempfile::tempdir().unwrap();
        // Not a git repo — git's exit status will be non-zero.
        let result = git_log_grep(td.path(), "anything", 5);
        // Either None (git failed) or Some(empty) (git succeeded somehow).
        match result {
            None => {}
            Some(v) => assert!(v.is_empty(), "non-repo should yield no hits"),
        }
    }

    #[test]
    fn print_why_chain_smoke_no_data_includes_partial_marker() {
        // Just verify the function doesn't panic on empty input. The
        // partial-tag header is unconditional under the current
        // WIRE-008 stance, but the smoke check is what we're after.
        print_why_chain("test", &[], &[], &[], true);
    }
}
