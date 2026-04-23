//! `mneme view` — open the vision app.
//!
//! Tries the native Tauri binary first (per design §9.8 "Native desktop"),
//! and falls back to opening `http://localhost:7777` in the user's browser.
//! `--web` forces the browser path even when a Tauri binary is on PATH.

use clap::Args;
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn};

use crate::error::{CliError, CliResult};

/// Default URL for the web fallback.
const DEFAULT_WEB_URL: &str = "http://localhost:7777";

/// CLI args for `mneme view`.
#[derive(Debug, Args)]
pub struct ViewArgs {
    /// Skip the native Tauri binary; open the browser at
    /// [`DEFAULT_WEB_URL`] instead.
    #[arg(long)]
    pub web: bool,

    /// Override the URL opened in the browser. Defaults to
    /// [`DEFAULT_WEB_URL`].
    #[arg(long, default_value = "http://localhost:7777")]
    pub url: String,

    /// Override the path to the native binary.
    #[arg(long, env = "DATATREE_VISION_BIN")]
    pub bin: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: ViewArgs) -> CliResult<()> {
    if !args.web {
        let candidate = match args.bin {
            Some(ref p) => p.clone(),
            None => default_vision_binary(),
        };
        if candidate.exists() {
            info!(path = %candidate.display(), "spawning native vision binary");
            return spawn_native(&candidate);
        }
        warn!(
            path = %candidate.display(),
            "native vision binary not found; falling back to browser"
        );
    }
    open_browser(&args.url)
}

/// Spawn the Tauri binary detached — we want the CLI to return immediately.
fn spawn_native(bin: &std::path::Path) -> CliResult<()> {
    Command::new(bin)
        .spawn()
        .map_err(|e| CliError::io(bin, e))?;
    Ok(())
}

/// Open `url` in the user's default browser using the platform-native
/// opener. On Windows that's `cmd /c start`, on macOS `open`, on Linux
/// `xdg-open`.
fn open_browser(url: &str) -> CliResult<()> {
    info!(url, "opening browser for vision app");
    #[cfg(target_os = "windows")]
    {
        // `cmd /c start "" <url>` — the empty title arg is required or
        // cmd will treat a quoted url as the window title.
        Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .map_err(CliError::io_pathless)?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().map_err(CliError::io_pathless)?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(CliError::io_pathless)?;
    }
    Ok(())
}

/// Default install location for the vision app (`~/.mneme/bin/mneme-vision`).
fn default_vision_binary() -> PathBuf {
    let mut p = crate::state_dir().join("bin").join("mneme-vision");
    if cfg!(windows) {
        p.set_extension("exe");
    }
    p
}
