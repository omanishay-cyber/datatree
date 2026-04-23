//! [`ScannerRegistry`] — owns one instance of every built-in scanner and
//! routes a file to the subset that declares `applies_to(file) == true`.

use std::path::Path;
use std::sync::Arc;

use crate::scanner::Scanner;
use crate::scanners::{
    a11y::A11yScanner, drift::DriftScanner, ipc::IpcContractsScanner,
    markdown_drift::MarkdownDriftScanner, perf::PerfScanner, refactor::RefactorScanner,
    secrets::SecretsScanner, security::SecurityScanner, theme::ThemeScanner,
    types_ts::TsTypesScanner,
};

/// Project-level configuration that scanners need at construction time.
/// Most fields are optional; absent paths simply disable the dependent
/// scanner (e.g. no `theme_tokens_path` -> ThemeScanner runs with the
/// built-in default allowlist).
#[derive(Debug, Default, Clone)]
pub struct RegistryConfig {
    /// Path to the project's CSS theme tokens file (e.g. `src/styles/tokens.css`).
    pub theme_tokens_path: Option<String>,
    /// Path to the project's IPC contract types file (e.g. `src/types/ipc.ts`).
    pub ipc_types_path: Option<String>,
    /// Project root used to resolve relative `.md` link/path claims.
    pub project_root: Option<String>,
    /// Pre-loaded constraint specs the [`DriftScanner`] should enforce.
    pub drift_constraints: Vec<crate::scanners::drift::ConstraintSpec>,
}

/// Holds every built-in scanner. Cheap to clone — the inner scanners are
/// shared via `Arc`.
#[derive(Clone)]
pub struct ScannerRegistry {
    scanners: Vec<Arc<dyn Scanner>>,
}

impl ScannerRegistry {
    /// Build the default registry with every built-in scanner registered.
    #[must_use]
    pub fn new(config: RegistryConfig) -> Self {
        let scanners: Vec<Arc<dyn Scanner>> = vec![
            Arc::new(ThemeScanner::new(config.theme_tokens_path.clone())),
            Arc::new(TsTypesScanner::new()),
            Arc::new(SecurityScanner::new()),
            Arc::new(A11yScanner::new()),
            Arc::new(PerfScanner::new()),
            Arc::new(DriftScanner::new(config.drift_constraints.clone())),
            Arc::new(IpcContractsScanner::new(config.ipc_types_path.clone())),
            Arc::new(MarkdownDriftScanner::new(config.project_root.clone())),
            Arc::new(SecretsScanner::new()),
            Arc::new(RefactorScanner::new()),
        ];
        Self { scanners }
    }

    /// Return the subset of scanners that report `applies_to(file) == true`.
    #[must_use]
    pub fn applicable_scanners(&self, file: &Path) -> Vec<&dyn Scanner> {
        self.scanners
            .iter()
            .filter(|s| s.applies_to(file))
            .map(|s| s.as_ref())
            .collect()
    }

    /// All scanners regardless of applicability — used by tests + telemetry.
    #[must_use]
    pub fn all(&self) -> &[Arc<dyn Scanner>] {
        &self.scanners
    }

    /// Look up a scanner by exact name.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&dyn Scanner> {
        self.scanners
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
    }
}
