//! `mneme update [project_path]` — incremental update sweep.
//!
//! v0.3.1: direct-DB path. Delegates to the same inline parse pipeline
//! `mneme build` uses, with `--full=false` semantics — only files
//! modified since the last build get re-parsed. No supervisor round-
//! trip; works even when the daemon is down.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{resolve_project, BuildArgs};
use crate::error::CliResult;

/// CLI args for `mneme update`.
#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Path to the project root. Defaults to CWD.
    pub project: Option<PathBuf>,

    /// Skip the pre-flight file-count confirmation. Equivalent to
    /// `mneme build --yes`. Update already targets an existing shard,
    /// so this is usually safe to set.
    #[arg(long, short = 'y', default_value_t = true)]
    pub yes: bool,
}

/// Entry point used by `main.rs`.
pub async fn run(args: UpdateArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project)?;
    tracing::info!(project = %project.display(), "incremental update (via build pipeline)");

    // Reuse the build pipeline — full=false means "only changed files"
    // (the pipeline's incremental parse path is keyed off mtime + sha).
    let build_args = BuildArgs {
        project: Some(project),
        full: false,
        limit: 0,
        dispatch: false,
        inline: true,
        yes: args.yes,
    };
    crate::commands::build::run(build_args, socket_override).await
}
