//! Bridge between the Rust supervisor and the Python multimodal sidecar.
//!
//! Spawns `python -m datatree_multimodal` as a subprocess, proxies
//! length-prefixed msgpack frames between the supervisor IPC socket and
//! the Python sidecar's stdin/stdout. If the sidecar dies, the bridge
//! exits non-zero so the supervisor restarts both.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "mneme-multimodal-bridge")]
struct Cli {
    /// Path to the Python interpreter to use. Defaults to bundled
    /// `~/.mneme/runtime/python/bin/python` if present, else `python3`.
    #[arg(long, env = "DATATREE_PYTHON")]
    python: Option<PathBuf>,

    /// Override the mneme home (default: ~/.mneme).
    #[arg(long, env = "MNEME_HOME")]
    home: Option<PathBuf>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("MNEME_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let cli = Cli::parse();

    let python = cli.python.unwrap_or_else(|| {
        let home = cli.home.clone().unwrap_or_else(|| {
            dirs::home_dir().expect("no home dir").join(".mneme")
        });
        let bundled = home.join("runtime/python/bin/python");
        if bundled.exists() { bundled } else { PathBuf::from("python3") }
    });

    info!(python = %python.display(), "spawning multimodal sidecar");

    let mut child = Command::new(&python)
        .args(["-m", "datatree_multimodal"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut child_stdin = child.stdin.take().expect("piped stdin");
    let mut child_stdout = child.stdout.take().expect("piped stdout");

    // For Phase 1 the bridge runs in a "ping/wait" mode: it forwards stdin
    // → child stdin, child stdout → stdout. The supervisor's IPC routing
    // attaches us via a socketpair externally, so this loop is intentionally
    // simple. Future expansion: a 4-byte length-prefixed msgpack channel
    // multiplexer.
    let stdout_task = tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        loop {
            match child_stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let mut out = tokio::io::stdout();
                    if out.write_all(&buf[..n]).await.is_err() { break; }
                    if out.flush().await.is_err() { break; }
                }
                Err(_) => break,
            }
        }
    });

    let stdin_task = tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        let mut input = tokio::io::stdin();
        loop {
            match input.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if child_stdin.write_all(&buf[..n]).await.is_err() { break; }
                    if child_stdin.flush().await.is_err() { break; }
                }
                Err(_) => break,
            }
        }
    });

    let status = child.wait().await?;
    let _ = stdout_task.await;
    let _ = stdin_task.await;

    if !status.success() {
        error!(?status, "multimodal sidecar exited non-zero");
        std::process::exit(status.code().unwrap_or(1));
    }
    info!("multimodal bridge clean exit");
    Ok(())
}

// Reserved types for future structured channel.
#[derive(Debug, Serialize, Deserialize)]
struct Frame {
    job_id: String,
    kind: String,
    payload: serde_json::Value,
}

#[allow(dead_code)]
async fn _placeholder(_: Arc<Mutex<()>>) -> anyhow::Result<()> {
    warn!("structured-channel mode not yet wired");
    Ok(())
}
