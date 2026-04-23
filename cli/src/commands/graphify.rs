//! `mneme graphify` — multimodal extraction pass.
//!
//! v0.2: runs the extractors from the `mneme-multimodal` crate IN-PROCESS.
//! Drops the Python sidecar entirely. The walker mirrors the one in
//! `build.rs`: recurse the project root, filter out vendor/cache dirs,
//! dispatch each file to the extractor [`Registry`] and persist the
//! resulting [`ExtractedDoc`] to the project shard's `media.db` via
//! `store::inject` on [`DbLayer::Multimodal`].
//!
//! If the supervisor is running, `--via-supervisor` forwards to the
//! legacy IPC request (kept for symmetry with `build` and `update`).

use clap::Args;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::commands::build::{handle_response, make_client, resolve_project};
use crate::error::{CliError, CliResult};
use crate::ipc::IpcRequest;

use common::{ids::ProjectId, layer::DbLayer, paths::PathManager};
use multimodal::{ExtractedDoc, Registry};
use sha2::{Digest, Sha256};
use store::{inject::InjectOptions, Store};

/// CLI args for `mneme graphify`.
#[derive(Debug, Args)]
pub struct GraphifyArgs {
    /// Project root. Defaults to CWD.
    pub project: Option<PathBuf>,

    /// Forward to the supervisor (legacy path; no-op extraction happens
    /// there). Default is to run extraction in-process.
    #[arg(long)]
    pub via_supervisor: bool,

    /// Cap the number of files processed (0 = no cap). Useful for
    /// smoke-testing on huge repos.
    #[arg(long, default_value_t = 0)]
    pub limit: usize,
}

/// Entry point used by `main.rs`.
pub async fn run(args: GraphifyArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project)?;

    if args.via_supervisor {
        let client = make_client(socket_override);
        let resp = client
            .request(IpcRequest::Graphify {
                project: project.clone(),
            })
            .await?;
        return handle_response(resp);
    }

    info!(project = %project.display(), "graphify (in-process rust extractors)");

    // Open/init the shard so media.db exists.
    let paths = PathManager::default_root();
    let store = Store::new(paths.clone());
    let project_id = ProjectId::from_path(&project)
        .map_err(|e| CliError::Other(format!("cannot hash project path: {e}")))?;
    let project_name = project
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let _shard = store
        .builder
        .build_or_migrate(&project_id, &project, &project_name)
        .await
        .map_err(|e| CliError::Other(format!("store build_or_migrate: {e}")))?;

    let registry = Registry::default_wired();
    let mut total = 0usize;
    let mut extracted = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    let walker = walkdir::WalkDir::new(&project)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()));

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "walk error; continuing");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if args.limit > 0 && extracted >= args.limit {
            break;
        }
        total += 1;
        let path = entry.path();
        if registry.find(path).is_none() {
            // Not a multimodal file — skip quietly.
            continue;
        }

        let doc = match registry.extract(path) {
            Ok(d) => d,
            Err(multimodal::ExtractError::Unsupported { .. }) => {
                continue;
            }
            Err(e) => {
                warn!(file = %path.display(), error = %e, "extract failed; skipping");
                errors += 1;
                skipped += 1;
                continue;
            }
        };

        if let Err(e) = persist(&store, &project_id, &doc).await {
            warn!(file = %path.display(), error = %e, "persist failed; skipping");
            errors += 1;
            skipped += 1;
            continue;
        }
        extracted += 1;
        if extracted % 10 == 0 {
            println!("  extracted {extracted} files ({errors} errors)");
        }
    }

    println!();
    println!("graphify complete:");
    println!("  walked:    {total}");
    println!("  extracted: {extracted}");
    println!("  skipped:   {skipped}");
    println!("  errors:    {errors}");
    println!("  shard:     {}", paths.project_root(&project_id).display());
    Ok(())
}

async fn persist(
    store: &Store,
    project: &ProjectId,
    doc: &ExtractedDoc,
) -> Result<(), String> {
    let bytes = std::fs::read(&doc.source)
        .map_err(|e| format!("read {}: {e}", doc.source.display()))?;
    let sha = hex_sha256(&bytes);
    let elements_json = serde_json::to_string(&doc.elements).unwrap_or_else(|_| "[]".into());
    let transcript_json = if doc.transcript.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&doc.transcript).unwrap_or_default()
    };

    let sql = "INSERT OR REPLACE INTO media(path, sha256, media_type, extracted_text, elements, transcript, extracted_at, extractor_version) \
               VALUES(?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), ?7)";
    let params = vec![
        serde_json::Value::String(doc.source.display().to_string()),
        serde_json::Value::String(sha),
        serde_json::Value::String(doc.kind.clone()),
        serde_json::Value::String(doc.text.clone()),
        serde_json::Value::String(elements_json),
        if transcript_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(transcript_json)
        },
        serde_json::Value::String(doc.extractor_version.clone()),
    ];
    let resp = store
        .inject
        .insert(
            project,
            DbLayer::Multimodal,
            sql,
            params,
            InjectOptions {
                emit_event: false,
                audit: false,
                ..InjectOptions::default()
            },
        )
        .await;
    if !resp.success {
        return Err(resp
            .error
            .map(|e| format!("{e:?}"))
            .unwrap_or_else(|| "unknown store error".into()));
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(
        name,
        "target"
            | "node_modules"
            | ".git"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | ".idea"
            | ".vscode"
    )
}
