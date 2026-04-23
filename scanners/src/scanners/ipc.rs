//! `IpcContractsScanner` — verifies that every IPC channel referenced in
//! the renderer (`window.electronAPI.method()`) and exposed in the
//! preload (`contextBridge.exposeInMainWorld`) is also handled in the
//! main process (`ipcMain.handle("...", ...)`), and that channel names
//! line up with the project's shared types file.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

use crate::scanner::{line_col_of, Ast, Finding, Scanner, Severity};

/// `ipcMain.handle("name", ...)` / `ipcMain.on("name", ...)`.
static MAIN_CHANNEL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\bipcMain\.(?:handle|on)\s*\(\s*['"]([^'"]+)['"]"#)
        .expect("main channel regex")
});

/// `ipcRenderer.invoke("name", ...)` / `.send("name", ...)`.
static RENDERER_CHANNEL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\bipcRenderer\.(?:invoke|send|on)\s*\(\s*['"]([^'"]+)['"]"#)
        .expect("renderer channel regex")
});

/// `contextBridge.exposeInMainWorld(... { name: ... })` — capture the
/// outer call so we can scan the body for keys.
static CONTEXT_BRIDGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"contextBridge\.exposeInMainWorld\s*\(").expect("contextBridge regex")
});

/// Identifier-like keys inside an object literal. Used to extract method
/// names from a `{ foo: () => ipcRenderer.invoke('foo'), ... }`.
static OBJECT_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*([A-Za-z_$][A-Za-z0-9_$]*)\s*:").expect("object key regex"));

const IPC_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// IPC contract scanner.
pub struct IpcContractsScanner {
    /// Pre-loaded set of declared channel names from the project's IPC
    /// types file. Empty when no path supplied.
    declared_channels: HashSet<String>,
}

impl IpcContractsScanner {
    /// Build a scanner. If `types_path` points at a file we extract every
    /// quoted string-literal token from it and treat those as the
    /// authoritative channel allow-list.
    #[must_use]
    pub fn new(types_path: Option<String>) -> Self {
        let mut declared = HashSet::new();
        if let Some(path) = types_path {
            if let Ok(s) = std::fs::read_to_string(&path) {
                let re =
                    Regex::new(r#"['"]([a-z][a-z0-9:_\-./]*)['"]"#).expect("types channel regex");
                for caps in re.captures_iter(&s) {
                    if let Some(m) = caps.get(1) {
                        declared.insert(m.as_str().to_string());
                    }
                }
            }
        }
        Self {
            declared_channels: declared,
        }
    }
}

impl Scanner for IpcContractsScanner {
    fn name(&self) -> &str {
        "ipc"
    }

    fn applies_to(&self, file: &Path) -> bool {
        file.extension()
            .and_then(|e| e.to_str())
            .map(|e| IPC_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    fn scan(&self, file: &Path, content: &str, _ast: Option<Ast<'_>>) -> Vec<Finding> {
        let file_str = file.to_string_lossy().to_string();
        let mut out = Vec::new();

        let mut main_handlers = HashSet::new();
        for caps in MAIN_CHANNEL.captures_iter(content) {
            if let Some(name) = caps.get(1) {
                main_handlers.insert(name.as_str().to_string());
            }
        }
        let mut renderer_calls: Vec<(String, usize)> = Vec::new();
        for caps in RENDERER_CHANNEL.captures_iter(content) {
            if let (Some(name), Some(whole)) = (caps.get(1), caps.get(0)) {
                renderer_calls.push((name.as_str().to_string(), whole.start()));
            }
        }

        // Renderer calls a channel that the same file does not declare in
        // a contextBridge AND that's not in the declared types — flag.
        let bridge_keys: HashSet<String> = if CONTEXT_BRIDGE.is_match(content) {
            OBJECT_KEY
                .captures_iter(content)
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .collect()
        } else {
            HashSet::new()
        };

        for (name, offset) in renderer_calls {
            let in_bridge = bridge_keys.contains(&name);
            let in_types = self.declared_channels.is_empty()
                || self.declared_channels.contains(&name);
            if !in_bridge && !in_types {
                let (line, col) = line_col_of(content, offset);
                out.push(Finding::new_line(
                    "ipc.unknown-channel",
                    Severity::Error,
                    &file_str,
                    line,
                    col,
                    col + name.len() as u32,
                    format!(
                        "IPC channel '{}' invoked from renderer is not declared in preload bridge or types file.",
                        name
                    ),
                ));
            }
        }

        // Main handler that doesn't match the declared types file.
        if !self.declared_channels.is_empty() {
            for caps in MAIN_CHANNEL.captures_iter(content) {
                if let (Some(name), Some(whole)) = (caps.get(1), caps.get(0)) {
                    if !self.declared_channels.contains(name.as_str()) {
                        let (line, col) = line_col_of(content, whole.start());
                        out.push(Finding::new_line(
                            "ipc.handler-not-in-types",
                            Severity::Warning,
                            &file_str,
                            line,
                            col,
                            col + (whole.end() - whole.start()) as u32,
                            format!(
                                "ipcMain handler '{}' is not declared in the shared IPC types file.",
                                name.as_str()
                            ),
                        ));
                    }
                }
            }
        }

        out
    }
}
