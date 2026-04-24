//! `mneme models` — local model management.
//!
//! The brain crate's semantic recall needs a sentence-embedding model
//! (BGE-Small-En-v1.5, 384-dim, ~130 MB ONNX). This subcommand downloads and
//! caches it into `~/.mneme/llm/` so every subsequent embed call is fully
//! local.
//!
//! Subcommands:
//! - `install` — download default models (BGE-Small-En-v1.5).
//! - `status`  — print which models are present and their sizes.
//! - `path`    — print the model directory path (for scripts).
//!
//! All other network-bearing work in mneme is forbidden; this command is
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
    /// Install BGE-small-en-v1.5 from a local directory you already have.
    ///
    /// Either pass `--from-path <dir>` pointing at a directory that
    /// contains `bge-small-en-v1.5.onnx` and `tokenizer.json`, or (with
    /// `fastembed-install` feature enabled at build time) omit the flag
    /// to let fastembed download the default model.
    ///
    /// Network download via an arbitrary URL is only available when
    /// `--from-url <url>` is passed explicitly — there are no implicit
    /// network calls.
    Install {
        /// Local directory containing `bge-small-en-v1.5.onnx` +
        /// `tokenizer.json`. Files are copied into `~/.mneme/models/`.
        #[arg(long, value_name = "DIR")]
        from_path: Option<PathBuf>,

        /// Explicit download URL (opt-in network). Not yet implemented —
        /// documented for forward compatibility so users know this is the
        /// only path that can make a network call.
        #[arg(long, value_name = "URL")]
        from_url: Option<String>,

        /// Force re-install even if already cached.
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
        Op::Install {
            from_path,
            from_url,
            force,
        } => install(from_path, from_url, force),
        Op::Status => status(),
        Op::Path => path_cmd(),
    }
}

/// Model root. Honors `$MNEME_HOME` first, then `~/.mneme/`.
fn model_root() -> PathBuf {
    if let Ok(h) = std::env::var("MNEME_HOME") {
        return PathBuf::from(h).join("models");
    }
    dirs::home_dir()
        .map(|h| h.join(".mneme").join("models"))
        .unwrap_or_else(|| PathBuf::from(".mneme/models"))
}

fn install(from_path: Option<PathBuf>, from_url: Option<String>, force: bool) -> CliResult<()> {
    let root = model_root();
    fs::create_dir_all(&root).ok();

    let marker = root.join(".installed");
    let target_onnx = root.join("bge-small-en-v1.5.onnx");
    let target_tok = root.join("tokenizer.json");

    if marker.exists() && !force && from_path.is_none() && from_url.is_none() {
        println!("mneme: BGE-Small-En-v1.5 already installed at {}", root.display());
        println!("        · run with --force or --from-path to reinstall");
        return Ok(());
    }

    // --- from-url: explicit opt-in network download. Not yet implemented ---
    if from_url.is_some() {
        eprintln!("mneme: --from-url is not yet wired. Current policy: no implicit network.");
        eprintln!("        · workaround: download manually, then re-run with --from-path <dir>");
        return Ok(());
    }

    // --- from-path: copy local files (NO network) ---
    if let Some(src_dir) = from_path {
        if !src_dir.is_dir() {
            eprintln!("mneme: --from-path {} is not a directory", src_dir.display());
            return Ok(());
        }
        let src_onnx = src_dir.join("bge-small-en-v1.5.onnx");
        let src_tok = src_dir.join("tokenizer.json");

        if !src_onnx.exists() {
            eprintln!("mneme: missing {}", src_onnx.display());
            eprintln!("        · expected BGE-small-en-v1.5 ONNX export");
            return Ok(());
        }
        if !src_tok.exists() {
            eprintln!("mneme: missing {}", src_tok.display());
            eprintln!("        · expected BGE tokenizer.json");
            return Ok(());
        }

        println!("mneme: copying BGE-Small-En-v1.5 from {}", src_dir.display());
        println!("       into {}", root.display());

        if let Err(e) = fs::copy(&src_onnx, &target_onnx) {
            eprintln!("mneme: copy onnx failed: {e}");
            return Ok(());
        }
        if let Err(e) = fs::copy(&src_tok, &target_tok) {
            eprintln!("mneme: copy tokenizer.json failed: {e}");
            return Ok(());
        }

        fs::write(&marker, b"v0.2 bge-small-en-v1.5 via --from-path\n").ok();
        let size_mb = fs::metadata(&target_onnx).map(|m| m.len() / 1_048_576).unwrap_or(0);
        println!("mneme: model installed ({} MB)", size_mb);
        println!("        · enable `real-embeddings` feature at build time to use it");
        println!("        · on Windows also set ORT_DYLIB_PATH or place onnxruntime.dll on PATH");
        return Ok(());
    }

    // --- default install path: fastembed download (feature-gated) ---
    println!("mneme: installing BGE-Small-En-v1.5 (~130 MB) into {}", root.display());

    #[cfg(feature = "fastembed-install")]
    {
        match ::brain::install_default_model() {
            Ok(()) => {
                fs::write(&marker, b"v0.2 BGESmallENV15 via fastembed\n").ok();
                println!("mneme: model installed");
            }
            Err(e) => {
                eprintln!("mneme: install failed: {e}");
                eprintln!("        · the embedder will run in fallback (hashing-trick) mode");
                eprintln!("        · retry: mneme models install --force");
                eprintln!(
                    "        · or manual install: drop bge-small-en-v1.5.onnx + \
                     tokenizer.json into {} and re-run with --from-path",
                    root.display()
                );
            }
        }
    }

    #[cfg(not(feature = "fastembed-install"))]
    {
        let _ = (&target_onnx, &target_tok, &marker);
        println!("        · this mneme build was compiled without the `fastembed-install` feature.");
        println!("          Two ways forward:");
        println!("           1. rebuild with `--features fastembed-install` (pulls fastembed).");
        println!("           2. manual: download BGE-small-en-v1.5.onnx + tokenizer.json and run:");
        println!("                mneme models install --from-path <download-dir>");
        println!("          Target directory: {}", root.display());
    }

    Ok(())
}

fn status() -> CliResult<()> {
    let root = model_root();
    println!("mneme model root: {}", root.display());

    // New layout (v0.2.4+): files directly under root.
    let onnx = root.join("bge-small-en-v1.5.onnx");
    let tok = root.join("tokenizer.json");
    let marker = root.join(".installed");

    // Legacy layout (v0.2.0-v0.2.3): everything under `bge-small/` subdir.
    let legacy = root.join("bge-small");
    let legacy_marker = legacy.join(".installed");

    let installed = marker.exists() || legacy_marker.exists();
    if installed {
        let size = if onnx.exists() {
            fs::metadata(&onnx).map(|m| m.len()).unwrap_or(0)
        } else {
            directory_size(&legacy).unwrap_or(0)
        };
        println!(
            "  [x] bge-small-en-v1.5    {} MB   {}",
            size / 1_048_576,
            root.display()
        );
        println!(
            "       onnx:      {} {}",
            if onnx.exists() { "[x]" } else { "[ ]" },
            onnx.display()
        );
        println!(
            "       tokenizer: {} {}",
            if tok.exists() { "[x]" } else { "[ ]" },
            tok.display()
        );
    } else {
        println!("  [ ] bge-small-en-v1.5    not installed — run `mneme models install --from-path <dir>`");
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
