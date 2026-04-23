//! `datatree models` — local model management.
//!
//! The brain crate's semantic recall needs a sentence-embedding model
//! (BGE-Small-En-v1.5, 384-dim, ~130 MB ONNX). This subcommand downloads and
//! caches it into `~/.datatree/llm/` so every subsequent embed call is fully
//! local.
//!
//! Subcommands:
//! - `install` — download default models (BGE-Small-En-v1.5).
//! - `status`  — print which models are present and their sizes.
//! - `path`    — print the model directory path (for scripts).
//!
//! All other network-bearing work in datatree is forbidden; this command is
//! the single explicit user-initiated download point.

use clap::{Args, Subcommand};
use std::fs;
use std::path::PathBuf;

use crate::CliResult;

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[command(subcommand)]
    pub op: Op,
}

#[derive(Debug, Subcommand)]
pub enum Op {
    /// Download default embedding model into `~/.datatree/llm/`.
    Install {
        /// Force re-download even if cached.
        #[arg(long)]
        force: bool,
    },
    /// Show which models are installed.
    Status,
    /// Print the model directory path (useful for scripts).
    Path,
}

pub fn run(args: ModelsArgs) -> CliResult<()> {
    match args.op {
        Op::Install { force } => install(force),
        Op::Status => status(),
        Op::Path => path_cmd(),
    }
}

fn model_root() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".datatree").join("llm"))
        .unwrap_or_else(|| PathBuf::from(".datatree/llm"))
}

fn install(force: bool) -> CliResult<()> {
    let root = model_root();
    fs::create_dir_all(&root).ok();

    let bge_dir = root.join("bge-small");
    let marker = bge_dir.join(".installed");
    if marker.exists() && !force {
        println!("datatree: BGE-Small-En-v1.5 already installed at {}", bge_dir.display());
        println!("        · run with --force to reinstall");
        return Ok(());
    }

    println!("datatree: installing BGE-Small-En-v1.5 (~130 MB) into {}", bge_dir.display());
    println!("        · this is the ONLY network call datatree will make");

    #[cfg(feature = "fastembed-install")]
    {
        match ::brain::install_default_model() {
            Ok(()) => {
                fs::create_dir_all(&bge_dir).ok();
                fs::write(&marker, b"v0.2 BGESmallENV15 via fastembed\n").ok();
                println!("datatree: model installed");
            }
            Err(e) => {
                eprintln!("datatree: install failed: {e}");
                eprintln!("        · the embedder will run in fallback (hashing-trick) mode");
                eprintln!("        · retry: datatree models install --force");
            }
        }
    }

    #[cfg(not(feature = "fastembed-install"))]
    {
        let _ = &bge_dir;
        let _ = &marker;
        println!("        · this datatree build was compiled without the `fastembed-install` feature.");
        println!("          rebuild with `--features fastembed-install` or drop model files manually into:");
        println!("            {}", bge_dir.display());
    }

    Ok(())
}

fn status() -> CliResult<()> {
    let root = model_root();
    println!("datatree model root: {}", root.display());

    let bge = root.join("bge-small");
    if bge.join(".installed").exists() {
        let size = directory_size(&bge).unwrap_or(0);
        println!("  [x] bge-small-en-v1.5    {} MB   {}", size / 1_048_576, bge.display());
    } else {
        println!("  [ ] bge-small-en-v1.5    not installed — run `datatree models install`");
    }
    Ok(())
}

fn path_cmd() -> CliResult<()> {
    println!("{}", model_root().display());
    Ok(())
}

fn directory_size(p: &std::path::Path) -> std::io::Result<u64> {
    let mut total: u64 = 0;
    for entry in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}
