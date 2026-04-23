//! `datatree graphify` — multimodal extraction pass.
//!
//! Triggers the multimodal-bridge worker which fans out to PyMuPDF /
//! Tesseract / faster-whisper / nbformat / python-docx / openpyxl as
//! appropriate for the file types found under the project root.

use clap::Args;
use std::path::PathBuf;

use crate::commands::build::{handle_response, make_client, resolve_project};
use crate::error::CliResult;
use crate::ipc::IpcRequest;

/// CLI args for `datatree graphify`.
#[derive(Debug, Args)]
pub struct GraphifyArgs {
    /// Project root. Defaults to CWD.
    pub project: Option<PathBuf>,
}

/// Entry point used by `main.rs`.
pub async fn run(args: GraphifyArgs, socket_override: Option<PathBuf>) -> CliResult<()> {
    let project = resolve_project(args.project)?;
    let client = make_client(socket_override);
    let resp = client.request(IpcRequest::Graphify { project }).await?;
    handle_response(resp)
}
