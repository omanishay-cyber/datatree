// CSV-style println!: literal column values are intentional alongside dynamic ones.
#![allow(clippy::print_literal, clippy::type_complexity)]

//! `bench_retrieval` — mneme retrieval + scaling benchmark driver.
//!
//! Subcommands:
//!   * `index <repo_path>`                     — full index build, prints JSON.
//!   * `query <shard_path> <query>`            — blast + recall + refs JSON.
//!   * `compare <repo_path>`                   — 10 golden queries, markdown/JSON/CSV.
//!   * `bench-token-reduction <repo>`          — cold/mneme token ratios.
//!   * `bench-first-build <repo>`              — cold + warm full-build timings.
//!   * `bench-incremental <repo>`              — inject_file p50/p95 over 100 files.
//!   * `bench-viz-scale <repo>`                — graph.db bytes per node/edge.
//!   * `bench-recall <repo> <fixture.json>`    — precision@10 over golden set.
//!   * `bench-all <repo>`                      — runs everything, single CSV.
//!
//! All output is JSON, CSV, or Markdown — never free text — so CI can parse it.
//! The default `--format` for `compare` is markdown; for `bench-*` it is JSON;
//! for `bench-all` the output is always CSV.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use benchmarks::{
    bench_first_build, bench_incremental, bench_recall, bench_token_reduction, bench_viz_scale,
    cold_baseline, compare_vs_cold, index_repo, load_fixture, run_one_query, shard_graph_db,
    BenchError, BenchResult, CompareReport, FirstBuildReport, GoldenQuery, IncrementalReport,
    QueryKind, RecallReport, TokenReductionReport, VizScaleReport,
};

#[derive(Debug, Parser)]
#[command(
    name = "bench_retrieval",
    about = "Mneme retrieval + scaling benchmark harness"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Markdown,
    Json,
    Csv,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Index a repo and print a JSON report with files/nodes/edges/ms.
    Index { repo_path: PathBuf },
    /// Run one query against an existing shard (graph.db).
    Query { shard_path: PathBuf, query: String },
    /// Index (if needed) + run the 10 golden queries; emit a comparison table.
    Compare {
        repo_path: Option<PathBuf>,
        #[arg(long)]
        fixtures: Option<PathBuf>,
        #[arg(long)]
        skip_index: bool,
        /// Output format. Default: markdown.
        #[arg(long, value_enum, default_value_t = Format::Markdown)]
        format: Format,
    },
    /// Token-reduction ratios across 10 generic queries (mean + p50 + p95).
    BenchTokenReduction {
        repo_path: PathBuf,
        #[arg(long)]
        skip_index: bool,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Cold (no shard) + warm (shard present) full-build wall-clock times.
    BenchFirstBuild {
        repo_path: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Single-file `store::inject` p50 + p95 over up to 100 files.
    BenchIncremental {
        repo_path: PathBuf,
        #[arg(long)]
        skip_index: bool,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// graph.db bytes per node + per edge.
    BenchVizScale {
        repo_path: PathBuf,
        #[arg(long)]
        skip_index: bool,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Precision@10 across golden queries.
    BenchRecall {
        repo_path: PathBuf,
        fixture: PathBuf,
        #[arg(long)]
        skip_index: bool,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Run every bench and emit a single CSV with one row per metric.
    BenchAll {
        repo_path: PathBuf,
        #[arg(long)]
        fixture: Option<PathBuf>,
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
            format,
        } => {
            let repo = repo_path.unwrap_or_else(|| PathBuf::from("."));
            cmd_compare(&repo, fixtures.as_deref(), skip_index, format).await
        }
        Cmd::BenchTokenReduction {
            repo_path,
            skip_index,
            format,
        } => cmd_bench_token_reduction(&repo_path, skip_index, format).await,
        Cmd::BenchFirstBuild { repo_path, format } => {
            cmd_bench_first_build(&repo_path, format).await
        }
        Cmd::BenchIncremental {
            repo_path,
            skip_index,
            format,
        } => cmd_bench_incremental(&repo_path, skip_index, format).await,
        Cmd::BenchVizScale {
            repo_path,
            skip_index,
            format,
        } => cmd_bench_viz_scale(&repo_path, skip_index, format).await,
        Cmd::BenchRecall {
            repo_path,
            fixture,
            skip_index,
            format,
        } => cmd_bench_recall(&repo_path, &fixture, skip_index, format).await,
        Cmd::BenchAll { repo_path, fixture } => cmd_bench_all(&repo_path, fixture.as_deref()).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", serde_json::json!({ "error": e.to_string() }));
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
    format: Format,
) -> BenchResult<()> {
    if !skip_index {
        let report = index_repo(repo).await?;
        eprintln!(
            "indexed: {} files, {} nodes, {} edges in {} ms",
            report.files_indexed, report.nodes, report.edges, report.elapsed_ms
        );
    }

    let fixture_path = match fixtures {
        Some(p) => p.to_path_buf(),
        None => default_fixture_path(),
    };
    let queries = load_fixture(&fixture_path)?;
    let shard = shard_graph_db(repo)?;
    let report = compare_vs_cold(repo, &shard, &queries)?;

    let repo_tag = repo
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo")
        .to_string();
    match format {
        Format::Markdown => {
            print_compare_markdown(&report);
            eprintln!("{}", serde_json::to_string_pretty(&report)?);
        }
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Format::Csv => {
            print_compare_csv(&repo_tag, &report);
        }
    }
    Ok(())
}

async fn cmd_bench_token_reduction(
    repo: &Path,
    skip_index: bool,
    format: Format,
) -> BenchResult<()> {
    if !skip_index {
        let _ = index_repo(repo).await?;
    }
    let shard = shard_graph_db(repo)?;
    let report = bench_token_reduction(repo, &shard)?;
    emit_simple(&report, format, "token_reduction")
}

async fn cmd_bench_first_build(repo: &Path, format: Format) -> BenchResult<()> {
    let report = bench_first_build(repo).await?;
    emit_simple(&report, format, "first_build")
}

async fn cmd_bench_incremental(repo: &Path, skip_index: bool, format: Format) -> BenchResult<()> {
    if !skip_index {
        let _ = index_repo(repo).await?;
    }
    let report = bench_incremental(repo).await?;
    emit_simple(&report, format, "incremental")
}

async fn cmd_bench_viz_scale(repo: &Path, skip_index: bool, format: Format) -> BenchResult<()> {
    if !skip_index {
        let _ = index_repo(repo).await?;
    }
    let shard = shard_graph_db(repo)?;
    let report = bench_viz_scale(&shard)?;
    emit_simple(&report, format, "viz_scale")
}

async fn cmd_bench_recall(
    repo: &Path,
    fixture: &Path,
    skip_index: bool,
    format: Format,
) -> BenchResult<()> {
    if !skip_index {
        let _ = index_repo(repo).await?;
    }
    let shard = shard_graph_db(repo)?;
    let queries = load_fixture_flex(fixture)?;
    let report = bench_recall(&shard, &queries)?;
    emit_simple(&report, format, "recall")
}

async fn cmd_bench_all(repo: &Path, fixture: Option<&Path>) -> BenchResult<()> {
    // Ensure the repo is indexed once, then run every bench.
    let index = index_repo(repo).await?;
    eprintln!(
        "[bench-all] indexed: files={} nodes={} edges={} ms={}",
        index.files_indexed, index.nodes, index.edges, index.elapsed_ms
    );

    let shard = shard_graph_db(repo)?;
    let token = bench_token_reduction(repo, &shard)?;
    let viz = bench_viz_scale(&shard)?;

    // bench-incremental re-opens store writers — run after token/viz so the
    // read-only primitives above are not racing a concurrent writer.
    let incr = bench_incremental(repo).await?;

    // Compare report for per-query precision@5 + token rows.
    let fixture_path = fixture
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_fixture_path);
    let queries = load_fixture_flex(&fixture_path)?;
    let compare = compare_vs_cold(repo, &shard, &queries)?;
    let recall = bench_recall(&shard, &queries)?;

    // First-build benchmark wipes the shard — do this LAST.
    let first = bench_first_build(repo).await?;

    let repo_tag = repo
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo")
        .to_string();

    // Emit one unified CSV with: the classic per-query rows, plus meta rows
    // for token-reduction, first-build, incremental, viz-scale, recall.
    println!(
        "repo,query,mneme_top1,mneme_tokens,mneme_ms,cold_top1,cold_tokens,cold_ms,precision_at_5"
    );
    for row in &compare.rows {
        let dt_top1 = row.mneme_top.first().map(String::as_str).unwrap_or("-");
        let cold_top1 = row.cold_top.first().map(String::as_str).unwrap_or("-");
        println!(
            "{},{},{},{},{},{},{},{},{}",
            csv_esc(&repo_tag),
            csv_esc(&row.query),
            csv_esc(&short_path(dt_top1)),
            row.mneme_tokens,
            row.mneme_ms,
            csv_esc(&short_path(cold_top1)),
            row.cold_tokens,
            row.cold_ms,
            row.mneme_precision_at_5,
        );
    }

    // Meta rows use the `query` column as the metric name and pack the rest
    // into the numeric columns; top1 columns carry the unit label.
    println!(
        "{},{},{},{},{},{},{},{},{}",
        csv_esc(&repo_tag),
        "META:token_reduction_mean",
        "ratio",
        (token.mean_ratio * 1000.0) as u64,
        0,
        "ratio",
        (token.p50_ratio * 1000.0) as u64,
        (token.p95_ratio * 1000.0) as u64,
        0,
    );
    println!(
        "{},{},{},{},{},{},{},{},{}",
        csv_esc(&repo_tag),
        "META:first_build",
        "cold_ms",
        first.cold_ms,
        first.warm_ms,
        "warm_ms",
        first.nodes,
        first.edges,
        0,
    );
    println!(
        "{},{},{},{},{},{},{},{},{}",
        csv_esc(&repo_tag),
        "META:incremental_inject",
        "p50_ms",
        incr.p50_ms,
        incr.mean_ms,
        "p95_ms",
        incr.p95_ms,
        incr.max_ms,
        incr.samples,
    );
    println!(
        "{},{},{},{},{},{},{},{},{}",
        csv_esc(&repo_tag),
        "META:viz_scale",
        "bytes_per_node",
        viz.bytes_per_node,
        viz.bytes_per_edge,
        "bytes_per_edge",
        viz.nodes,
        viz.edges,
        viz.graph_db_bytes,
    );
    println!(
        "{},{},{},{},{},{},{},{},{}",
        csv_esc(&repo_tag),
        "META:precision_at_10",
        "pct",
        recall.precision_at_10_pct,
        recall.hits,
        "hits",
        recall.total_expected,
        recall.queries,
        0,
    );

    // Echo the overall totals as stderr for human glance.
    eprintln!(
        "{}",
        serde_json::json!({
            "repo": repo_tag,
            "token_reduction": token,
            "first_build": first,
            "incremental": incr,
            "viz_scale": viz,
            "recall": recall,
            "compare_totals": {
                "mneme_total_tokens": compare.mneme_total_tokens,
                "cold_total_tokens": compare.cold_total_tokens,
                "mneme_total_ms": compare.mneme_total_ms,
                "cold_total_ms": compare.cold_total_ms,
                "mneme_precision_pct": compare.mneme_precision_pct,
                "cold_precision_pct": compare.cold_precision_pct,
            }
        })
    );
    Ok(())
}

fn print_compare_markdown(report: &CompareReport) {
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

fn print_compare_csv(repo_tag: &str, report: &CompareReport) {
    println!(
        "repo,query,mneme_top1,mneme_tokens,mneme_ms,cold_top1,cold_tokens,cold_ms,precision_at_5"
    );
    for row in &report.rows {
        let dt_top1 = row.mneme_top.first().map(String::as_str).unwrap_or("-");
        let cold_top1 = row.cold_top.first().map(String::as_str).unwrap_or("-");
        println!(
            "{},{},{},{},{},{},{},{},{}",
            csv_esc(repo_tag),
            csv_esc(&row.query),
            csv_esc(&short_path(dt_top1)),
            row.mneme_tokens,
            row.mneme_ms,
            csv_esc(&short_path(cold_top1)),
            row.cold_tokens,
            row.cold_ms,
            row.mneme_precision_at_5,
        );
    }
}

fn emit_simple<T: serde::Serialize>(report: &T, format: Format, label: &str) -> BenchResult<()> {
    match format {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(report)?);
        }
        Format::Markdown => {
            println!(
                "### {label}\n\n```json\n{}\n```",
                serde_json::to_string_pretty(report)?
            );
        }
        Format::Csv => {
            // Flatten the serialisable report into key,value rows. Works
            // uniformly for every report type without per-type plumbing.
            let v = serde_json::to_value(report)?;
            println!("metric,field,value");
            emit_flat_csv(label, &v, "");
        }
    }
    Ok(())
}

fn emit_flat_csv(metric: &str, v: &serde_json::Value, prefix: &str) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, inner) in m {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                emit_flat_csv(metric, inner, &path);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, inner) in arr.iter().enumerate() {
                let path = format!("{prefix}[{i}]");
                emit_flat_csv(metric, inner, &path);
            }
        }
        serde_json::Value::Null => {
            println!("{},{},", csv_esc(metric), csv_esc(prefix));
        }
        _ => {
            println!(
                "{},{},{}",
                csv_esc(metric),
                csv_esc(prefix),
                csv_esc(&v.to_string())
            );
        }
    }
}

/// Loader that accepts either the v0.1 flat array form or the v0.2
/// `{ queries: [...] }` wrapper used in `integration-*.json` fixtures.
fn load_fixture_flex(path: &Path) -> BenchResult<Vec<GoldenQuery>> {
    if let Ok(plain) = load_fixture(path) {
        if !plain.is_empty() {
            return Ok(plain);
        }
    }
    let bytes =
        std::fs::read(path).map_err(|e| BenchError::Fixture(format!("{}: {e}", path.display())))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let arr = value
        .get("queries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| BenchError::Fixture("missing `queries` array".into()))?;
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        let q = entry
            .get("q")
            .and_then(|v| v.as_str())
            .or_else(|| entry.get("query").and_then(|v| v.as_str()))
            .ok_or_else(|| BenchError::Fixture("missing `q`/`query`".into()))?
            .to_string();
        let expected_top: Vec<String> = entry
            .get("expect_top_k")
            .and_then(|v| v.as_array())
            .or_else(|| entry.get("expected_top").and_then(|v| v.as_array()))
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        out.push(GoldenQuery {
            query: q,
            kind: QueryKind::Recall,
            target: None,
            expected_top,
        });
    }
    Ok(out)
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
    let normalised = p.replace('\\', "/");
    let parts: Vec<&str> = normalised.rsplit('/').collect();
    let take = parts.len().min(2);
    let mut out: Vec<&str> = parts[..take].to_vec();
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

fn csv_esc(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[allow(dead_code)]
fn _cold_baseline_anchor(repo: &Path, q: &str) -> BenchResult<(Vec<String>, u64)> {
    cold_baseline(repo, q)
}

// Keep unused type imports from triggering warnings if a feature-flagged
// report variant becomes code-gated in the future.
#[allow(dead_code)]
fn _type_anchors() -> (
    Option<TokenReductionReport>,
    Option<FirstBuildReport>,
    Option<IncrementalReport>,
    Option<VizScaleReport>,
    Option<RecallReport>,
) {
    (None, None, None, None, None)
}
