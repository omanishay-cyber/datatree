//! `bench_retrieval` — mneme retrieval benchmark driver.
//!
//! Three subcommands:
//!   * `index <repo_path>`              — full index build, prints JSON.
//!   * `query <shard_path> <query>`     — blast + recall + refs JSON.
//!   * `compare <repo_path>`            — 10 golden queries, markdown table.
//!
//! All output is JSON or Markdown, never free text, so CI can parse it.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use benchmarks::{
    compare_vs_cold, cold_baseline, index_repo, load_fixture, run_one_query, shard_graph_db,
    BenchError, BenchResult, CompareReport, GoldenQuery, QueryKind,
};

#[derive(Debug, Parser)]
#[command(name = "bench_retrieval", about = "Mneme retrieval benchmark harness")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Index a repo and print a JSON report with files/nodes/edges/ms.
    Index {
        /// Path to the repo to index.
        repo_path: PathBuf,
    },
    /// Run one query against an existing shard (graph.db).
    Query {
        /// Absolute path to the graph.db file in the project shard.
        shard_path: PathBuf,
        /// Free-form query text.
        query: String,
    },
    /// Index (if needed) + run the 10 golden queries; print a markdown
    /// comparison table against the cold-Claude grep baseline.
    Compare {
        /// Repo to benchmark (defaults to CWD).
        repo_path: Option<PathBuf>,
        /// Optional override of the fixtures JSON.
        #[arg(long)]
        fixtures: Option<PathBuf>,
        /// Skip indexing; reuse the existing shard as-is.
        #[arg(long)]
        skip_index: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Index { repo_path } => cmd_index(&repo_path).await,
        Cmd::Query { shard_path, query } => cmd_query(&shard_path, &query),
        Cmd::Compare {
            repo_path,
            fixtures,
            skip_index,
        } => {
            let repo = repo_path.unwrap_or_else(|| PathBuf::from("."));
            cmd_compare(&repo, fixtures.as_deref(), skip_index).await
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "{}",
                serde_json::json!({ "error": e.to_string() })
            );
            ExitCode::from(1)
        }
    }
}

async fn cmd_index(repo: &Path) -> BenchResult<()> {
    let report = index_repo(repo).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn cmd_query(shard: &Path, query: &str) -> BenchResult<()> {
    let conn = rusqlite::Connection::open_with_flags(
        shard,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(BenchError::Sql)?;

    let kinds = [QueryKind::Blast, QueryKind::Recall, QueryKind::References];
    let mut out = serde_json::Map::new();

    for k in kinds {
        let gq = GoldenQuery {
            query: query.to_string(),
            kind: k,
            target: Some(query.to_string()),
            expected_top: Vec::new(),
        };
        let start = std::time::Instant::now();
        let top = run_one_query(&conn, &gq)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let token_est = tokens_for(&top);
        out.insert(
            kind_label(k).to_string(),
            serde_json::json!({
                "top_files": top,
                "token_count_est": token_est,
                "elapsed_ms": elapsed_ms,
            }),
        );
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::Value::Object(out))?
    );
    Ok(())
}

async fn cmd_compare(
    repo: &Path,
    fixtures: Option<&Path>,
    skip_index: bool,
) -> BenchResult<()> {
    // 1. Index the repo unless the caller opts out.
    if !skip_index {
        let report = index_repo(repo).await?;
        eprintln!(
            "indexed: {} files, {} nodes, {} edges in {} ms",
            report.files_indexed, report.nodes, report.edges, report.elapsed_ms
        );
    }

    // 2. Locate the fixture file.
    let fixture_path = match fixtures {
        Some(p) => p.to_path_buf(),
        None => default_fixture_path(),
    };
    let queries = load_fixture(&fixture_path)?;

    // 3. Locate the graph.db for this repo.
    let shard = shard_graph_db(repo)?;

    // 4. Run the comparison.
    let report = compare_vs_cold(repo, &shard, &queries)?;

    // 5. Emit markdown table to stdout, JSON to stderr.
    print_markdown(&report);
    eprintln!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn print_markdown(report: &CompareReport) {
    println!("## Retrieval Benchmark");
    println!();
    println!("| # | Query | DT top-1 | DT tokens | DT ms | Cold top-1 | Cold tokens | Cold ms | DT p@5 | Cold p@5 |");
    println!("|---|-------|----------|-----------|-------|------------|-------------|---------|--------|----------|");
    for (i, row) in report.rows.iter().enumerate() {
        let dt_top1 = row.mneme_top.first().map(String::as_str).unwrap_or("-");
        let cold_top1 = row.cold_top.first().map(String::as_str).unwrap_or("-");
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            i + 1,
            truncate(&row.query, 40),
            truncate(&short_path(dt_top1), 32),
            row.mneme_tokens,
            row.mneme_ms,
            truncate(&short_path(cold_top1), 32),
            row.cold_tokens,
            row.cold_ms,
            row.mneme_precision_at_5,
            row.cold_precision_at_5,
        );
    }
    println!();
    println!("### Totals");
    println!();
    println!(
        "| Metric | Mneme | Cold baseline |\n|---|---|---|\n| Tokens (sum) | {} | {} |\n| Wall time (ms) | {} | {} |\n| Precision@5 (%) | {} | {} |",
        report.mneme_total_tokens,
        report.cold_total_tokens,
        report.mneme_total_ms,
        report.cold_total_ms,
        report.mneme_precision_pct,
        report.cold_precision_pct,
    );
}

fn default_fixture_path() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.join("fixtures").join("golden.json")
}

fn tokens_for(files: &[String]) -> u64 {
    let mut total = 0u64;
    for f in files {
        if let Ok(meta) = std::fs::metadata(f) {
            total += meta.len() / 4;
        }
    }
    total
}

fn kind_label(k: QueryKind) -> &'static str {
    match k {
        QueryKind::Blast => "blast_radius",
        QueryKind::Recall => "recall_file",
        QueryKind::References => "find_references",
    }
}

fn short_path(p: &str) -> String {
    // Keep only the last two path segments to fit markdown columns.
    let normalised = p.replace('\\', "/");
    let parts: Vec<&str> = normalised.rsplit('/').collect();
    let take = parts.len().min(2);
    let mut out: Vec<&str> = parts[..take].iter().copied().collect();
    out.reverse();
    out.join("/")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

// Keep cold_baseline imported so the linker does not drop-warn it when
// a future subcommand wires it directly.
#[allow(dead_code)]
fn _cold_baseline_anchor(
    repo: &Path,
    q: &str,
) -> BenchResult<(Vec<String>, u64)> {
    cold_baseline(repo, q)
}
