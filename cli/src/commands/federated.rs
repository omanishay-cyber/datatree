//! `mneme federated` — Moat 4: federated pattern matching CLI surface.
//!
//! Subcommands:
//!
//! ```text
//! mneme federated status          # local fingerprint counts
//! mneme federated opt-in          # enable upload (writes ~/.mneme/federated.optin)
//! mneme federated opt-out         # disable upload (removes the marker)
//! mneme federated scan <path>     # compute + index fingerprints for a path
//! mneme federated sync            # v0.2 stub: report "would upload N"
//! ```
//!
//! # Local-only invariant
//!
//! Per CLAUDE.md §40, `sync` is a stub in v0.2: it prints how many
//! fingerprints *would* be uploaded and writes nothing to the network.
//! Actual uploads land in v0.3 once the relay server design is signed off.

use clap::{Args, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

use brain::federated::{FederatedStore, PatternFingerprint};
use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};

use crate::error::{CliError, CliResult};

// ---------------------------------------------------------------------------
// CLI surface
// ---------------------------------------------------------------------------

/// Top-level args for `mneme federated`.
#[derive(Debug, Args)]
pub struct FederatedArgs {
    #[command(subcommand)]
    pub op: FederatedOp,
}

/// Every `mneme federated ...` subcommand.
#[derive(Debug, Subcommand)]
pub enum FederatedOp {
    /// Show local fingerprint counts broken down by pattern kind.
    Status {
        /// Optional project path. Defaults to CWD.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Enable opt-in upload. Writes `~/.mneme/federated.optin`.
    #[command(name = "opt-in")]
    OptIn,
    /// Disable opt-in upload. Removes `~/.mneme/federated.optin`.
    #[command(name = "opt-out")]
    OptOut,
    /// Compute + index fingerprints for every source file under `path`.
    Scan {
        /// Path to scan. Defaults to CWD.
        path: Option<PathBuf>,
        /// Optional project path (where the shard lives). Defaults to CWD.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// v0.2 stub: count pending fingerprints and print what *would* be
    /// uploaded. No bytes leave the machine.
    Sync {
        /// Optional project path. Defaults to CWD.
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Entry point used by `main.rs`.
pub fn run(args: FederatedArgs) -> CliResult<()> {
    match args.op {
        FederatedOp::Status { project } => cmd_status(project),
        FederatedOp::OptIn => cmd_opt_in(),
        FederatedOp::OptOut => cmd_opt_out(),
        FederatedOp::Scan { path, project } => cmd_scan(path, project),
        FederatedOp::Sync { project } => cmd_sync(project),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn cmd_status(project: Option<PathBuf>) -> CliResult<()> {
    let store = open_store(project)?;
    let counts = store
        .counts()
        .map_err(|e| CliError::Other(format!("counts: {e}")))?;

    println!("mneme federated — local fingerprint index");
    println!("  total:          {}", counts.total);
    println!("  pending upload: {}", counts.pending_upload);
    println!("  opt-in:         {}", if optin_marker().exists() { "yes" } else { "no" });
    if counts.by_kind.is_empty() {
        println!("  by kind:        (none yet — run `mneme federated scan`)");
    } else {
        println!("  by kind:");
        for (kind, n) in counts.by_kind {
            println!("    - {kind}: {n}");
        }
    }
    Ok(())
}

fn cmd_opt_in() -> CliResult<()> {
    let marker = optin_marker();
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent).map_err(CliError::io_pathless)?;
    }
    fs::write(
        &marker,
        b"# mneme federated opt-in -- remove this file to opt out.\n\
          # v0.2: sync is a stub, no network traffic yet.\n",
    )
    .map_err(|e| CliError::io(marker.clone(), e))?;
    println!("mneme: federated upload opt-in recorded at {}", marker.display());
    println!("       · v0.2: no network upload yet — `mneme federated sync` is a stub.");
    Ok(())
}

fn cmd_opt_out() -> CliResult<()> {
    let marker = optin_marker();
    if marker.exists() {
        fs::remove_file(&marker).map_err(|e| CliError::io(marker.clone(), e))?;
        println!("mneme: federated upload opt-in removed ({})", marker.display());
    } else {
        println!("mneme: not opted-in, nothing to remove");
    }
    Ok(())
}

fn cmd_scan(path: Option<PathBuf>, project: Option<PathBuf>) -> CliResult<()> {
    let scan_root = path
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let mut store = open_store(project)?;

    let mut indexed = 0usize;
    let mut skipped = 0usize;
    for entry in walkdir::WalkDir::new(&scan_root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_source_file(path) || is_ignored(path) {
            skipped += 1;
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if content.is_empty() {
            skipped += 1;
            continue;
        }

        let kind = pattern_kind_for(path);
        let fp = FederatedStore::compute_fingerprint(&content, kind);
        let source = path.to_string_lossy().into_owned();
        store
            .index_local_with_source(fp, Some(&source))
            .map_err(|e| CliError::Other(format!("index fingerprint for {}: {e}", path.display())))?;
        indexed += 1;
    }

    println!(
        "mneme federated: scanned {}, indexed {}, skipped {}",
        scan_root.display(),
        indexed,
        skipped
    );
    Ok(())
}

fn cmd_sync(project: Option<PathBuf>) -> CliResult<()> {
    let store = open_store(project)?;
    let export = store
        .export_for_upload()
        .map_err(|e| CliError::Other(format!("export: {e}")))?;
    let opted_in = optin_marker().exists();

    if !opted_in {
        println!("mneme federated: not opted-in — run `mneme federated opt-in` first.");
        println!("                 {} fingerprints would be sanitised and uploaded.", export.len());
        return Ok(());
    }

    // v0.2: stub — no network. Report counts + a tiny histogram so the
    // user can see exactly what they're agreeing to ship later.
    println!("mneme federated sync — v0.2 stub (no network upload in v0.2)");
    println!("  fingerprints pending: {}", export.len());
    println!("  bytes (serialized):   {}", est_bytes(&export));
    println!("  local-only fields dropped at export: source_file");
    if export.is_empty() {
        println!("  nothing to upload.");
    } else {
        // Show a small preview: first 5 kinds.
        let preview: Vec<&str> = export.iter().take(5).map(|fp| fp.pattern_kind.as_str()).collect();
        println!("  preview kinds:        {:?}", preview);
    }
    println!("  (actual relay upload ships in v0.3 once the server design lands.)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn open_store(project: Option<PathBuf>) -> CliResult<FederatedStore> {
    let project = crate::commands::build::resolve_project(project)?;
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let paths = PathManager::default_root();
    let db_path = paths.shard_db(&project_id, DbLayer::Federated);
    FederatedStore::new(&db_path)
        .map_err(|e| CliError::Other(format!("open federated store: {e}")))
}

/// Location of the user-level opt-in marker. Presence = opted-in.
fn optin_marker() -> PathBuf {
    PathManager::default_root().root().join("federated.optin")
}

/// Very rough source-file filter. Covers the languages we already parse
/// for Convention Learner output.
fn is_source_file(path: &Path) -> bool {
    const EXTS: &[&str] = &[
        "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "kt", "swift", "c", "cc",
        "cpp", "h", "hpp", "rb", "php", "cs", "scala", "sh", "bash", "zsh", "ps1",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| EXTS.contains(&ext))
        .unwrap_or(false)
}

/// Skip vendored / generated dirs that would otherwise flood the index.
fn is_ignored(path: &Path) -> bool {
    const BAD: &[&str] = &[
        "node_modules",
        "target",
        "dist",
        "build",
        ".git",
        ".venv",
        "venv",
        "__pycache__",
        ".next",
        ".nuxt",
        ".cache",
    ];
    path.components()
        .any(|c| matches!(c, std::path::Component::Normal(n) if BAD.iter().any(|b| n == std::ffi::OsStr::new(b))))
}

/// Bucket an extension into a coarse pattern-kind tag.
fn pattern_kind_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust_file",
        Some("ts") | Some("tsx") => "ts_file",
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => "js_file",
        Some("py") => "py_file",
        Some("go") => "go_file",
        Some("java") | Some("kt") | Some("scala") => "jvm_file",
        Some("swift") => "swift_file",
        Some("c") | Some("cc") | Some("cpp") | Some("h") | Some("hpp") => "c_file",
        Some("rb") => "rb_file",
        Some("php") => "php_file",
        Some("cs") => "cs_file",
        Some("sh") | Some("bash") | Some("zsh") => "shell_file",
        Some("ps1") => "ps1_file",
        _ => "other_file",
    }
}

/// Rough byte-size estimate of the serialised payload (bincode). Each
/// fingerprint is 4 bytes per minhash u32 (512 B) + the short string
/// fields + headers — 600 B is a safe upper bound.
fn est_bytes(export: &[PatternFingerprint]) -> usize {
    const PER_FP: usize = 600;
    export.len() * PER_FP
}
