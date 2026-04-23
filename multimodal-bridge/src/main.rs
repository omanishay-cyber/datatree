//! `mneme-multimodal` — thin CLI around [`mneme_multimodal::Registry`].
//!
//! Use cases:
//!   * `mneme-multimodal extract path/to/file.pdf` → prints JSON to stdout
//!   * `mneme-multimodal extract-dir /project --out results.jsonl`
//!
//! The daemon/supervisor path now imports this crate as a library
//! (via `mneme-cli`) rather than spawning a Python sidecar, so this
//! binary exists mainly for ad-hoc inspection and test fixtures.

use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;

use mneme_multimodal::{ExtractedDoc, Registry};

#[derive(Debug, Parser)]
#[command(
    name = "mneme-multimodal",
    version,
    about = "Pure-Rust multimodal extraction (PDF / Markdown / Image / Audio / Video)"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Extract a single file; print the resulting `ExtractedDoc` as JSON.
    Extract {
        /// File to extract.
        path: PathBuf,
    },
    /// Walk a directory and emit one JSON object per successfully
    /// extracted file (JSON lines).
    ExtractDir {
        /// Root directory.
        path: PathBuf,
        /// Optional output file. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print every file extension the default registry handles.
    Kinds,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("MNEME_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let registry = Registry::default_wired();

    match cli.command {
        Cmd::Extract { path } => {
            let doc = registry.extract(&path)?;
            let out = serde_json::to_string_pretty(&doc)?;
            println!("{out}");
        }
        Cmd::ExtractDir { path, out } => {
            let mut sink: Box<dyn Write> = match out.as_ref() {
                Some(p) => Box::new(std::fs::File::create(p)?),
                None => Box::new(std::io::stdout().lock()),
            };
            let mut n_ok = 0usize;
            let mut n_skip = 0usize;
            for entry in walk(&path) {
                let doc = match registry.try_extract(&entry) {
                    Some(d) => d,
                    None => {
                        n_skip += 1;
                        continue;
                    }
                };
                write_jsonl(&mut sink, &doc)?;
                n_ok += 1;
            }
            info!(ok = n_ok, skipped = n_skip, root = %path.display(), "extract-dir complete");
        }
        Cmd::Kinds => {
            for k in registry.known_kinds() {
                println!("{k}");
            }
        }
    }
    Ok(())
}

fn walk(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                // Skip conventional noise; the CLI's own walker does more.
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if matches!(
                    name,
                    ".git" | "target" | "node_modules" | "__pycache__" | ".venv" | "venv"
                ) {
                    continue;
                }
                stack.push(p);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    out
}

fn write_jsonl(sink: &mut dyn Write, doc: &ExtractedDoc) -> anyhow::Result<()> {
    let s = serde_json::to_string(doc)?;
    sink.write_all(s.as_bytes())?;
    sink.write_all(b"\n")?;
    Ok(())
}
