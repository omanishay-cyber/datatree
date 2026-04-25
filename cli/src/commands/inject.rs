//! `mneme inject` — UserPromptSubmit hook entry point.
//!
//! Claude Code calls this after the user submits a prompt. We forward the
//! prompt to the supervisor, which composes a "smart inject bundle"
//! (§4.2): recent decisions, active constraints, blast-radius previews,
//! drift redirect, and the current step from the ledger.
//!
//! ## v0.3.1 — STDIN + CLI parity
//!
//! Claude Code delivers the payload on STDIN as JSON:
//!
//! ```json
//! { "session_id": "...", "hook_event_name": "UserPromptSubmit",
//!   "prompt": "...", "cwd": "..." }
//! ```
//!
//! Manual testing from a shell uses `--prompt`, `--session-id`, `--cwd`.
//! Both paths work; CLI flags win when both present. See
//! [`crate::hook_payload`] for the merge logic.
//!
//! If STDIN is a TTY and no flags are passed, all fields default to
//! safe empty values and we emit an empty `additional_context`. The
//! rule is hard: **this hook NEVER exits non-zero**. It was the
//! deepest-blast-radius hook in the v0.3.0 self-trap (it gated
//! UserPromptSubmit — a non-zero exit muted the user), and must never
//! block a prompt because of an internal failure of mneme.
//!
//! ## v0.3.1+ — skill prescription
//!
//! When the payload carries a `prompt`, the hook also runs a minimal
//! in-process skill matcher against `~/.mneme/plugin/skills/` (see
//! [`crate::skill_matcher`]) and, if the top suggestion fires at
//! `medium` or `high` confidence, appends a
//! `<mneme-skill-prescription>` block to the emitted
//! `additional_context`. Pass `--no-skill-hint` to skip this.
//!
//! Output format is the JSON shape Claude Code expects from a
//! UserPromptSubmit hook:
//!
//! ```json
//! { "hookEventName": "UserPromptSubmit",
//!   "additional_context": "<mneme-context>...</mneme-context>" }
//! ```

use clap::Args;
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::warn;

use common::{ids::ProjectId, paths::PathManager};

use crate::commands::build::make_client;
use crate::error::CliResult;
use crate::hook_payload::{choose, read_stdin_payload};
use crate::ipc::{IpcRequest, IpcResponse};
use crate::skill_matcher::{reason_for, suggest, Confidence, Suggestion};

/// Default staleness threshold (days) when `staleness_warn_days` is not
/// set in `<project_root>/.claude/mneme.json`. Audit-L12 acceptance.
const DEFAULT_STALENESS_WARN_DAYS: i64 = 7;

/// CLI args for `mneme inject`. All optional — STDIN JSON fills in
/// anything missing.
#[derive(Debug, Args)]
pub struct InjectArgs {
    /// The user prompt as captured by the hook. If absent, read from
    /// STDIN payload `.prompt` or treated as empty.
    #[arg(long)]
    pub prompt: Option<String>,

    /// Session id assigned by the host. If absent, read from STDIN
    /// `.session_id` or defaulted to `"unknown"`.
    #[arg(long = "session-id")]
    pub session_id: Option<String>,

    /// Working directory at the time the hook fired. If absent, read
    /// from STDIN `.cwd` or the process CWD.
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Skip the `<mneme-skill-prescription>` block. Useful when the
    /// user wants the supervisor's context without any skill-router
    /// nudge.
    #[arg(long = "no-skill-hint", default_value_t = false)]
    pub no_skill_hint: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: InjectArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    // Read STDIN payload; log and continue on any parse error so we never
    // block the user's prompt because of our own bug. See module docs.
    let stdin_payload = match read_stdin_payload() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "hook STDIN parse failed; falling back to CLI flags / empty");
            None
        }
    };

    let stdin_prompt = stdin_payload.as_ref().and_then(|p| p.prompt.clone());
    let stdin_session = stdin_payload.as_ref().and_then(|p| p.session_id.clone());
    let stdin_cwd = stdin_payload.as_ref().and_then(|p| p.cwd.clone());

    let prompt = choose(args.prompt, stdin_prompt, String::new());
    let session_id = choose(args.session_id, stdin_session, "unknown".to_string());
    let cwd = choose(
        args.cwd,
        stdin_cwd,
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );

    // Capture cwd before we move it into the IPC request - we still need
    // it locally for the staleness probe.
    let cwd_for_staleness = cwd.clone();

    let client = make_client(socket_override);
    let response = client
        .request(IpcRequest::Inject {
            prompt: prompt.clone(),
            session_id,
            cwd,
        })
        .await;

    let mut payload = match response {
        Ok(IpcResponse::Ok { message }) => message.unwrap_or_default(),
        Ok(IpcResponse::Error { message }) => {
            warn!(error = %message, "supervisor returned error; emitting empty additional_context");
            String::new()
        }
        Ok(IpcResponse::Pong)
        | Ok(IpcResponse::Status { .. })
        | Ok(IpcResponse::Logs { .. })
        | Ok(IpcResponse::JobQueued { .. })
        | Ok(IpcResponse::JobQueue { .. })
        | Ok(IpcResponse::RecallResults { .. })
        | Ok(IpcResponse::BlastResults { .. })
        | Ok(IpcResponse::GodNodesResults { .. }) => String::new(),
        Err(e) => {
            warn!(error = %e, "supervisor unreachable; emitting empty additional_context");
            String::new()
        }
    };

    // Append the skill-router recommendation when:
    //   - the user actually typed something (skip empty prompts),
    //   - the caller did not pass --no-skill-hint,
    //   - the top suggestion fires at medium or high confidence.
    if !args.no_skill_hint && !prompt.trim().is_empty() {
        match std::panic::catch_unwind(|| suggest(&prompt, 1)) {
            Ok(hits) => {
                if let Some(hit) = hits.into_iter().next() {
                    if matches!(hit.confidence, Confidence::Medium | Confidence::High) {
                        let block = render_skill_block(&prompt, &hit);
                        if payload.is_empty() {
                            payload = block;
                        } else {
                            payload.push_str("\n\n");
                            payload.push_str(&block);
                        }
                    }
                }
            }
            Err(_) => {
                warn!("skill matcher panicked; dropping skill prescription");
            }
        }
    }

    // Append the staleness nag (audit-L12) when the project has been
    // built before but not in the configured threshold window. Failure
    // here is silent: this hook NEVER blocks the user's prompt, and a
    // missing meta.db row is a different problem (project not yet built)
    // with a different fix (run `mneme build`), not this user's concern.
    let paths = PathManager::default_root();
    if let Some(block) = render_staleness_block(&paths, &cwd_for_staleness) {
        if payload.is_empty() {
            payload = block;
        } else {
            payload.push_str("\n\n");
            payload.push_str(&block);
        }
    }

    let out = json!({
        "hookEventName": "UserPromptSubmit",
        "additional_context": payload,
    });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}

/// Render a single `<mneme-skill-prescription>` block. Kept ASCII-only
/// — the user's Windows cp1252 terminal breaks on em-dashes and other
/// fancy punctuation.
fn render_skill_block(prompt: &str, hit: &Suggestion) -> String {
    let excerpt = excerpt(prompt, 120);
    // `to_load` is a plain `cat` against the absolute SKILL.md path so
    // the assistant can load the skill without the MCP server being up.
    // The path is the one mneme actually parsed, so dev-tree runs work
    // the same as installed-plugin runs.
    let source = hit.source_path.to_string_lossy();
    let reason = reason_for(hit);
    format!(
        concat!(
            "<mneme-skill-prescription>\n",
            "  task: {task}\n",
            "  recommended_skill: {skill}\n",
            "  confidence: {confidence}\n",
            "  reason: {reason}\n",
            "  to_load: cat {path}\n",
            "</mneme-skill-prescription>",
        ),
        task = excerpt,
        skill = hit.skill,
        confidence = hit.confidence.as_str(),
        reason = reason,
        path = source,
    )
}

/// Collapse whitespace + truncate so the excerpt never blows out the
/// hook JSON. Keeps output single-line-friendly.
fn excerpt(raw: &str, max_chars: usize) -> String {
    let collapsed: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(max_chars).collect();
    out.push_str("...");
    out
}

// ---------------------------------------------------------------------------
// L12: stale-index nag - `<mneme-primer-staleness>` block
// ---------------------------------------------------------------------------

/// Render the `<mneme-primer-staleness>` block for the inject hook.
///
/// Returns `None` (suppressed) when:
/// - The project has never been built (no `meta.db` row, or
///   `last_indexed_at` is NULL - that's a different message,
///   surfaced by the build path itself).
/// - The project was indexed within the threshold window
///   (default 7 days, configurable via
///   `<project_root>/.claude/mneme.json::staleness_warn_days`).
/// - Any I/O / SQL error occurs (silent: this hook NEVER blocks the
///   user's prompt because of an internal mneme failure).
///
/// Returns `Some(block)` only when `last_indexed_at` is populated and
/// older than the configured threshold.
fn render_staleness_block(paths: &PathManager, cwd: &Path) -> Option<String> {
    let project = find_project_root_for_cwd(cwd)?;
    let project_id = ProjectId::from_path(&project).ok()?;
    let last_indexed = read_last_indexed(paths, &project_id)?;
    let age_days = age_in_days(&last_indexed)?;
    let threshold = staleness_threshold_days(&project);
    if age_days <= threshold {
        return None;
    }
    Some(format_staleness_block(age_days, threshold))
}

/// Format the staleness block. Kept ASCII-only - Windows cp1252
/// terminals corrupt non-ASCII characters in additional_context.
fn format_staleness_block(age_days: i64, threshold_days: i64) -> String {
    format!(
        concat!(
            "<mneme-primer-staleness>\n",
            "Project last indexed {age} days ago (threshold: {threshold} days).\n",
            "Recall + blast results may not reflect recent edits.\n",
            "Run `mneme build` to refresh, or `mneme rebuild` for a clean reset.\n",
            "</mneme-primer-staleness>",
        ),
        age = age_days,
        threshold = threshold_days,
    )
}

/// Mirror of MCP's findProjectRoot: walk up from `cwd` looking for
/// any of the standard project markers. Returns the first match, or
/// `None` if we hit the filesystem root without finding any.
fn find_project_root_for_cwd(cwd: &Path) -> Option<PathBuf> {
    let markers = [".git", ".claude", "package.json", "Cargo.toml", "pyproject.toml"];
    let mut cur: PathBuf = cwd.to_path_buf();
    for _ in 0..40 {
        for m in markers.iter() {
            if cur.join(m).exists() {
                return Some(cur);
            }
        }
        match cur.parent() {
            Some(p) if p != cur => cur = p.to_path_buf(),
            _ => return None,
        }
    }
    None
}

/// Read `meta.db::projects.last_indexed_at` for `project_id`. Returns
/// `None` on any error or when the column is NULL.
fn read_last_indexed(paths: &PathManager, project_id: &ProjectId) -> Option<String> {
    let meta_path = paths.meta_db();
    if !meta_path.exists() {
        return None;
    }
    let conn = rusqlite::Connection::open_with_flags(
        &meta_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    let result: Result<Option<String>, rusqlite::Error> = conn.query_row(
        "SELECT last_indexed_at FROM projects WHERE id = ?1",
        rusqlite::params![project_id.as_str()],
        |r| r.get(0),
    );
    result.ok().flatten()
}

/// Compute integer days between SQLite `datetime('now')` format
/// ("YYYY-MM-DD HH:MM:SS", UTC) and the current wall clock. Returns
/// `None` on parse failure.
fn age_in_days(stamp: &str) -> Option<i64> {
    // SQLite datetime('now') is UTC and uses space separator. chrono's
    // RFC3339 parser does not accept that directly - use NaiveDateTime.
    let parsed = chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S").ok()?;
    let parsed_utc = parsed.and_utc();
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(parsed_utc);
    let days = delta.num_seconds() / 86_400;
    Some(days)
}

/// Read `<project_root>/.claude/mneme.json` and return its
/// `staleness_warn_days` key. Falls back to [`DEFAULT_STALENESS_WARN_DAYS`]
/// on any of the silent-default conditions: missing file, parse error,
/// missing key, non-integer value, or non-positive value.
fn staleness_threshold_days(project_root: &Path) -> i64 {
    let path = project_root.join(".claude").join("mneme.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return DEFAULT_STALENESS_WARN_DAYS;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return DEFAULT_STALENESS_WARN_DAYS;
    };
    let Some(n) = value
        .get("staleness_warn_days")
        .and_then(|v| v.as_i64())
    else {
        return DEFAULT_STALENESS_WARN_DAYS;
    };
    if n <= 0 {
        return DEFAULT_STALENESS_WARN_DAYS;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn excerpt_collapses_and_truncates() {
        let long = "  hello\n  world  this  is   a   very   long   prompt   that   must   be   truncated ";
        let out = excerpt(long, 30);
        assert!(out.starts_with("hello world"));
        assert!(out.len() <= 33); // 30 chars + "..."
        assert!(out.ends_with("..."));
    }

    #[test]
    fn render_block_is_ascii() {
        let hit = Suggestion {
            skill: "fireworks-debug".to_string(),
            triggers_matched: vec!["debug".to_string()],
            tags_matched: Vec::new(),
            confidence: Confidence::Medium,
            source_path: PathBuf::from("/tmp/SKILL.md"),
            score: 2,
        };
        let block = render_skill_block("debug a test", &hit);
        assert!(block.is_ascii());
        assert!(block.contains("recommended_skill: fireworks-debug"));
        assert!(block.contains("confidence: medium"));
        assert!(block.contains("to_load: cat /tmp/SKILL.md"));
    }

    // ---------- L12 staleness nag tests ----------

    /// Build a complete fake mneme env: a project root with a `.git`
    /// marker (so find_project_root_for_cwd resolves it), and a
    /// `~/.mneme/meta.db` containing a single `projects` row with a
    /// custom `last_indexed_at` value. Returns the kept tempdir, the
    /// path manager, and the project root.
    fn fixture_with_indexed(
        last_indexed: Option<&str>,
    ) -> (tempfile::TempDir, PathManager, PathBuf) {
        let dir = tempdir().expect("tempdir");
        let mneme_root = dir.path().join("mneme-home");
        std::fs::create_dir_all(&mneme_root).unwrap();
        let paths = PathManager::with_root(mneme_root);

        let project_root = dir.path().join("proj");
        std::fs::create_dir_all(project_root.join(".git")).unwrap();

        // Build meta.db with the same shape as schema::META_SQL.
        let conn = rusqlite::Connection::open(paths.meta_db()).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));
             CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                root TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_indexed_at TEXT,
                schema_version INTEGER NOT NULL
             );",
        )
        .unwrap();

        let id = ProjectId::from_path(&project_root).unwrap();
        match last_indexed {
            Some(ts) => {
                conn.execute(
                    "INSERT INTO projects(id, root, name, last_indexed_at, schema_version) VALUES(?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![id.as_str(), project_root.to_string_lossy(), "fixture", ts, 1],
                )
                .unwrap();
            }
            None => {
                conn.execute(
                    "INSERT INTO projects(id, root, name, schema_version) VALUES(?1, ?2, ?3, ?4)",
                    rusqlite::params![id.as_str(), project_root.to_string_lossy(), "fixture", 1],
                )
                .unwrap();
            }
        }

        (dir, paths, project_root)
    }

    fn ts_days_ago(days: i64) -> String {
        let stamp = chrono::Utc::now() - chrono::Duration::days(days);
        stamp.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    #[test]
    fn staleness_block_emitted_when_index_is_old() {
        let stamp = ts_days_ago(15);
        let (_keep, paths, project_root) = fixture_with_indexed(Some(&stamp));
        let block = render_staleness_block(&paths, &project_root)
            .expect("expected a staleness block for a 15-day-old index");
        assert!(block.is_ascii());
        assert!(block.starts_with("<mneme-primer-staleness>"));
        assert!(block.ends_with("</mneme-primer-staleness>"));
        assert!(block.contains("threshold: 7 days"));
        assert!(block.contains("Run `mneme build`"));
    }

    #[test]
    fn staleness_block_suppressed_when_index_is_fresh() {
        let stamp = ts_days_ago(1);
        let (_keep, paths, project_root) = fixture_with_indexed(Some(&stamp));
        let block = render_staleness_block(&paths, &project_root);
        assert!(
            block.is_none(),
            "expected NO block for a 1-day-old index, got {block:?}"
        );
    }

    #[test]
    fn staleness_block_suppressed_when_never_indexed() {
        let (_keep, paths, project_root) = fixture_with_indexed(None);
        let block = render_staleness_block(&paths, &project_root);
        assert!(
            block.is_none(),
            "expected NO block for a never-built project (different problem)"
        );
    }

    #[test]
    fn staleness_threshold_default_is_seven() {
        let dir = tempdir().unwrap();
        // No .claude/mneme.json file.
        assert_eq!(
            staleness_threshold_days(dir.path()),
            DEFAULT_STALENESS_WARN_DAYS
        );
    }

    #[test]
    fn staleness_threshold_reads_mneme_json() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(
            dir.path().join(".claude/mneme.json"),
            r#"{"staleness_warn_days": 30}"#,
        )
        .unwrap();
        assert_eq!(staleness_threshold_days(dir.path()), 30);
    }

    #[test]
    fn staleness_threshold_silent_default_on_garbage() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/mneme.json"), "not json").unwrap();
        assert_eq!(
            staleness_threshold_days(dir.path()),
            DEFAULT_STALENESS_WARN_DAYS
        );
    }

    #[test]
    fn staleness_threshold_silent_default_on_missing_key() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/mneme.json"), r#"{"other": 5}"#).unwrap();
        assert_eq!(
            staleness_threshold_days(dir.path()),
            DEFAULT_STALENESS_WARN_DAYS
        );
    }

    #[test]
    fn staleness_threshold_honors_per_project_override() {
        // A 10-day-old index with threshold=20 should NOT trigger the block.
        let stamp = ts_days_ago(10);
        let (_keep, paths, project_root) = fixture_with_indexed(Some(&stamp));
        std::fs::create_dir_all(project_root.join(".claude")).unwrap();
        std::fs::write(
            project_root.join(".claude/mneme.json"),
            r#"{"staleness_warn_days": 20}"#,
        )
        .unwrap();
        let block = render_staleness_block(&paths, &project_root);
        assert!(block.is_none(), "10-day index with 20-day threshold must not warn");

        // Same index with threshold=5 (override below default) MUST trigger.
        std::fs::write(
            project_root.join(".claude/mneme.json"),
            r#"{"staleness_warn_days": 5}"#,
        )
        .unwrap();
        let block = render_staleness_block(&paths, &project_root)
            .expect("10-day index with 5-day threshold MUST warn");
        assert!(block.contains("threshold: 5 days"));
    }
}
