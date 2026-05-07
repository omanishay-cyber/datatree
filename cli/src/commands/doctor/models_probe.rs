//! Local models and concept-memory persistence probes.

use super::render::line;

// ─── local models box ─────────────────────────────────────────────────────────

/// Render the "local models" box. Bug C — surface every registered model
/// file (BGE ONNX, BGE tokenizer, GGUFs) per kind so the user sees the
/// full bundle inventory at a glance, not just BGE. Reads
/// `~/.mneme/models/manifest.json`. Empty manifest renders a single
/// "no models registered" line + the install hint.
pub fn render_models_box() {
    use crate::commands::models::{public_model_root, read_manifest_or_empty, ModelKind};

    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ local models (~/.mneme/models)                          │");
    println!("├─────────────────────────────────────────────────────────┤");

    let root = public_model_root();
    line("model root", &root.display().to_string());
    let manifest = read_manifest_or_empty(&root);
    if manifest.entries.is_empty() {
        line(
            "✗ models",
            "0 registered — run `mneme models install --from-path <bundle/models>`",
        );
        println!("└─────────────────────────────────────────────────────────┘");
        return;
    }

    let mut embedding_models = 0usize;
    let mut tokenizers = 0usize;
    let mut llms = 0usize;
    let mut embedding_llms = 0usize;
    let mut total_bytes: u64 = 0;
    for entry in &manifest.entries {
        total_bytes = total_bytes.saturating_add(entry.size);
        match entry.kind {
            ModelKind::EmbeddingModel => embedding_models += 1,
            ModelKind::EmbeddingTokenizer => tokenizers += 1,
            ModelKind::Llm => llms += 1,
            ModelKind::EmbeddingLlm => embedding_llms += 1,
        }
    }
    let total_mb = total_bytes / 1_048_576;
    line(
        "✓ registered",
        &format!(
            "{} files * {} MB  ({} embedding, {} tokenizer, {} llm, {} embed-llm)",
            manifest.entries.len(),
            total_mb,
            embedding_models,
            tokenizers,
            llms,
            embedding_llms,
        ),
    );

    for entry in &manifest.entries {
        let mb = entry.size / 1_048_576;
        line(
            &format!("  * {}", entry.name),
            &format!("{:<19}  {} MB", entry.kind.label(), mb),
        );
    }
    println!("└─────────────────────────────────────────────────────────┘");
}

// ─── concept memory persistence box ─────────────────────────────────────────

/// Render the "concept memory" persistence box (v0.4 Wave 3.3).
///
/// Opens `~/.mneme/projects/<active-project>/concepts.db` (if it exists)
/// and shows the total row count so operators can confirm that concept
/// memory is being populated and surviving daemon restarts.
///
/// Gracefully degrades: if no project is active, if the shard has never
/// been created, or if SQLite fails, this renders a single informational
/// line rather than crashing or omitting the box entirely.
pub fn render_concepts_persistence_box() {
    use common::paths::PathManager;
    use rusqlite::Connection;

    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ concept memory (~/.mneme/projects/*/concepts.db)        │");
    println!("├─────────────────────────────────────────────────────────┤");

    let pm = PathManager::default_root();
    let projects_dir = pm.root().join("projects");

    if !projects_dir.exists() {
        line("status", "no projects indexed yet");
        println!("└─────────────────────────────────────────────────────────┘");
        return;
    }

    let mut total_rows: i64 = 0;
    let mut shards_found: usize = 0;
    let mut shards_ok: usize = 0;

    let read_dir = match std::fs::read_dir(&projects_dir) {
        Ok(rd) => rd,
        Err(e) => {
            line("status", &format!("cannot read projects dir: {e}"));
            println!("└─────────────────────────────────────────────────────────┘");
            return;
        }
    };

    for entry in read_dir.flatten() {
        let concepts_db = entry.path().join("concepts.db");
        if !concepts_db.exists() {
            continue;
        }
        shards_found += 1;
        match Connection::open(&concepts_db) {
            Ok(conn) => {
                // BENCH-FIX-3+ (2026-05-07): older shards on disk may
                // have an empty concepts.db (file created by some earlier
                // probe but the brain::ConceptStore CREATE TABLE never
                // ran because the user hasn't called recall_concept yet).
                // Run an idempotent CREATE TABLE IF NOT EXISTS so the
                // doctor probe's COUNT below succeeds instead of
                // emitting a misleading "no such table: concepts" WARN.
                // The schema mirrors the canonical one in
                // brain::concept_store. If the table already exists this
                // is a no-op.
                let _ = conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS concepts (\n  id INTEGER PRIMARY KEY AUTOINCREMENT,\n  label TEXT UNIQUE NOT NULL,\n  summary TEXT,\n  embedding_id INTEGER,\n  god_node_score REAL NOT NULL DEFAULT 0.0,\n  created_at TEXT NOT NULL DEFAULT (datetime('now'))\n);",
                );
                match conn.query_row("SELECT COUNT(*) FROM concepts", [], |row| {
                    row.get::<_, i64>(0)
                }) {
                    Ok(n) => {
                        total_rows += n;
                        shards_ok += 1;
                    }
                    Err(e) => {
                        tracing::warn!(?concepts_db, error = %e, "concepts.db row count failed");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(?concepts_db, error = %e, "cannot open concepts.db");
            }
        }
    }

    if shards_found == 0 {
        line(
            "status",
            "concepts.db not yet created (first recall_concept call will create it)",
        );
    } else {
        line(
            if shards_ok == shards_found {
                "✓ concepts"
            } else {
                "~ concepts"
            },
            &format!("{total_rows} rows across {shards_ok}/{shards_found} shards"),
        );
    }
    println!("└─────────────────────────────────────────────────────────┘");
}
