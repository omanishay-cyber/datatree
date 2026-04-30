//! Project Identity Kernel (blueprint F9).
//!
//! Produces a compact, deterministic "who/what/why" document for the active
//! project. Built offline from on-disk signals only — no LLM, no network.
//!
//! Consumers:
//!   * `cli/src/commands/session_prime.rs` — prepends the kernel to the
//!     injected SessionStart primer.
//!   * `mcp/src/tools/identity.ts` — MCP tool `mneme_identity` that returns
//!     the kernel as structured JSON.
//!
//! Inputs (all local):
//!   * repo root path
//!   * `package.json`, `Cargo.toml`, `requirements.txt`, `pyproject.toml`,
//!     `go.mod`, `deno.json`, `composer.json`, `Gemfile`, `pom.xml`, etc.
//!   * `README.md` (first non-heading paragraph → `domain_summary`)
//!   * brain's `ConceptExtractor` over the README (→ `key_concepts`)
//!   * optional: Step Ledger + git log — injected by the caller.
//!
//! This module does *not* read the Step Ledger or git log itself; callers
//! thread those through. That keeps the crate dependency-free of `git2` and
//! the store layer, consistent with brain/ being the pure-compute crate.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::concept::{Concept, ConceptExtractor, ExtractInput};

/// One detected technology (language, framework, build tool, runtime).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Technology {
    /// Canonical lower-case name ("rust", "react", "electron", "vite", ...).
    pub name: String,
    /// Detected version string, if visible in a manifest. `None` when the
    /// marker file proves presence but not version (e.g. a dependency with
    /// `"*"` or a plain lockfile entry).
    pub version: Option<String>,
    /// Which on-disk marker surfaced this technology.
    pub marker: String,
    /// Broad category, used by consumers for grouping in the UI.
    pub category: TechCategory,
}

/// Grouping used by MCP + Vision app. Order is load-bearing — displayed
/// top-to-bottom as `Language` → `Runtime` → `Framework` → `BuildTool` →
/// `Tooling`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TechCategory {
    Language,
    Runtime,
    Framework,
    BuildTool,
    Tooling,
}

/// Placeholder for a `Convention` produced by the Convention Learner
/// (F3, see `brain::conventions`). Kept here as a thin re-export so callers
/// importing `identity` do not need a second `use`.
pub use crate::conventions::Convention;

/// The full kernel. Intentionally cheap to build: a few small file reads
/// plus one pass of the deterministic concept extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIdentity {
    pub name: String,
    pub stack: Vec<Technology>,
    pub domain_summary: String,
    pub key_concepts: Vec<Concept>,
    pub conventions: Vec<Convention>,
    pub recent_goals: Vec<String>,
    pub open_questions: Vec<String>,
}

impl ProjectIdentity {
    /// Build a kernel by scanning `root` on disk.
    ///
    /// `recent_goals` and `open_questions` come from the caller (Step
    /// Ledger in the production path, empty vectors in tests).
    pub fn build(
        root: &Path,
        recent_goals: Vec<String>,
        open_questions: Vec<String>,
        conventions: Vec<Convention>,
    ) -> Self {
        let name = infer_project_name(root);
        let stack = detect_stack(root);
        let (domain_summary, readme_text) = read_domain_summary(root);
        let key_concepts = extract_key_concepts(&readme_text, &stack);

        Self {
            name,
            stack,
            domain_summary,
            key_concepts,
            conventions,
            recent_goals,
            open_questions,
        }
    }

    /// Render as a compact markdown block suitable for prepending to the
    /// SessionStart primer. Keep under ~1K tokens even for large stacks.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("<mneme-identity>\n");
        out.push_str(&format!("Project: {}\n", self.name));

        if !self.stack.is_empty() {
            out.push_str("\n## Stack\n");
            for t in &self.stack {
                match &t.version {
                    Some(v) => out.push_str(&format!("- {} ({})\n", t.name, v)),
                    None => out.push_str(&format!("- {}\n", t.name)),
                }
            }
        }

        if !self.domain_summary.is_empty() {
            out.push_str("\n## What it does\n");
            out.push_str(&self.domain_summary);
            out.push('\n');
        }

        if !self.key_concepts.is_empty() {
            out.push_str("\n## Key concepts\n");
            for c in self.key_concepts.iter().take(10) {
                out.push_str(&format!("- {}\n", c.term));
            }
        }

        if !self.conventions.is_empty() {
            out.push_str("\n## Conventions (top by confidence)\n");
            for c in self.conventions.iter().take(5) {
                out.push_str(&format!(
                    "- [{:.0}%] {}\n",
                    c.confidence * 100.0,
                    c.pattern.describe()
                ));
            }
        }

        if !self.recent_goals.is_empty() {
            out.push_str("\n## Recent goals\n");
            for g in self.recent_goals.iter().take(5) {
                out.push_str(&format!("- {}\n", g));
            }
        }

        if !self.open_questions.is_empty() {
            out.push_str("\n## Open questions\n");
            for q in self.open_questions.iter().take(5) {
                out.push_str(&format!("- {}\n", q));
            }
        }

        out.push_str("</mneme-identity>\n");
        out
    }
}

// ---------------------------------------------------------------------------
// Stack detection
// ---------------------------------------------------------------------------

/// Walk `root` for known manifest markers and return every detected
/// technology. Runs only on the repo root (1-level) to stay fast.
pub fn detect_stack(root: &Path) -> Vec<Technology> {
    let mut out: Vec<Technology> = Vec::new();

    // Node / JS ecosystem via package.json.
    let pkg_json_path = root.join("package.json");
    if pkg_json_path.exists() {
        out.push(Technology {
            name: "node".into(),
            version: None,
            marker: "package.json".into(),
            category: TechCategory::Runtime,
        });
        if let Ok(text) = fs::read_to_string(&pkg_json_path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(deps) = value.get("dependencies").and_then(|v| v.as_object()) {
                    detect_js_framework(deps, &mut out);
                }
                if let Some(deps) = value.get("devDependencies").and_then(|v| v.as_object()) {
                    detect_js_framework(deps, &mut out);
                }
            }
        }
    }

    // Bun lockfile → Bun runtime.
    if root.join("bun.lockb").exists() || root.join("bunfig.toml").exists() {
        push_unique(
            &mut out,
            Technology {
                name: "bun".into(),
                version: None,
                marker: "bun.lockb".into(),
                category: TechCategory::Runtime,
            },
        );
    }

    // Deno.
    if root.join("deno.json").exists() || root.join("deno.jsonc").exists() {
        out.push(Technology {
            name: "deno".into(),
            version: None,
            marker: "deno.json".into(),
            category: TechCategory::Runtime,
        });
    }

    // Rust via Cargo.toml.
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.exists() {
        out.push(Technology {
            name: "rust".into(),
            version: None,
            marker: "Cargo.toml".into(),
            category: TechCategory::Language,
        });
        if let Ok(text) = fs::read_to_string(&cargo_toml) {
            if text.contains("tauri") {
                push_unique(
                    &mut out,
                    Technology {
                        name: "tauri".into(),
                        version: None,
                        marker: "Cargo.toml".into(),
                        category: TechCategory::Framework,
                    },
                );
            }
            if text.contains("tokio") {
                push_unique(
                    &mut out,
                    Technology {
                        name: "tokio".into(),
                        version: None,
                        marker: "Cargo.toml".into(),
                        category: TechCategory::Framework,
                    },
                );
            }
        }
    }

    // Python markers.
    if root.join("requirements.txt").exists() {
        out.push(Technology {
            name: "python".into(),
            version: None,
            marker: "requirements.txt".into(),
            category: TechCategory::Language,
        });
    }
    if root.join("pyproject.toml").exists() {
        push_unique(
            &mut out,
            Technology {
                name: "python".into(),
                version: None,
                marker: "pyproject.toml".into(),
                category: TechCategory::Language,
            },
        );
    }

    // Go.
    if root.join("go.mod").exists() {
        out.push(Technology {
            name: "go".into(),
            version: read_go_mod_version(&root.join("go.mod")),
            marker: "go.mod".into(),
            category: TechCategory::Language,
        });
    }

    // Ruby.
    if root.join("Gemfile").exists() {
        out.push(Technology {
            name: "ruby".into(),
            version: None,
            marker: "Gemfile".into(),
            category: TechCategory::Language,
        });
    }

    // Java/Kotlin.
    if root.join("pom.xml").exists() {
        out.push(Technology {
            name: "java".into(),
            version: None,
            marker: "pom.xml".into(),
            category: TechCategory::Language,
        });
    }
    if root.join("build.gradle").exists() || root.join("build.gradle.kts").exists() {
        out.push(Technology {
            name: "gradle".into(),
            version: None,
            marker: "build.gradle".into(),
            category: TechCategory::BuildTool,
        });
    }

    // PHP.
    if root.join("composer.json").exists() {
        out.push(Technology {
            name: "php".into(),
            version: None,
            marker: "composer.json".into(),
            category: TechCategory::Language,
        });
    }

    // .NET.
    if dir_contains_suffix(root, ".csproj") || dir_contains_suffix(root, ".sln") {
        out.push(Technology {
            name: "dotnet".into(),
            version: None,
            marker: ".csproj".into(),
            category: TechCategory::Runtime,
        });
    }

    out
}

fn detect_js_framework(
    deps: &serde_json::Map<String, serde_json::Value>,
    out: &mut Vec<Technology>,
) {
    // Probe list keeps us O(entries) rather than O(entries * frameworks).
    let probes: &[(&str, &str, TechCategory)] = &[
        ("electron", "electron", TechCategory::Framework),
        ("react", "react", TechCategory::Framework),
        ("next", "next.js", TechCategory::Framework),
        ("vue", "vue", TechCategory::Framework),
        ("svelte", "svelte", TechCategory::Framework),
        ("solid-js", "solid", TechCategory::Framework),
        ("@tauri-apps/api", "tauri", TechCategory::Framework),
        ("vite", "vite", TechCategory::BuildTool),
        ("webpack", "webpack", TechCategory::BuildTool),
        ("esbuild", "esbuild", TechCategory::BuildTool),
        ("turbo", "turborepo", TechCategory::BuildTool),
        ("typescript", "typescript", TechCategory::Language),
        ("tailwindcss", "tailwind", TechCategory::Tooling),
        ("zustand", "zustand", TechCategory::Tooling),
        ("vitest", "vitest", TechCategory::Tooling),
        ("jest", "jest", TechCategory::Tooling),
        ("playwright", "playwright", TechCategory::Tooling),
    ];
    for (dep_key, name, cat) in probes {
        if let Some(v) = deps.get(*dep_key) {
            let version = v
                .as_str()
                .map(|s| s.trim_start_matches(['^', '~']).to_string());
            push_unique(
                out,
                Technology {
                    name: (*name).to_string(),
                    version,
                    marker: "package.json".into(),
                    category: *cat,
                },
            );
        }
    }
}

fn push_unique(out: &mut Vec<Technology>, t: Technology) {
    if !out.iter().any(|e| e.name == t.name) {
        out.push(t);
    }
}

fn dir_contains_suffix(root: &Path, suffix: &str) -> bool {
    let Ok(rd) = fs::read_dir(root) else {
        return false;
    };
    for entry in rd.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

fn read_go_mod_version(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("go ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Project name
// ---------------------------------------------------------------------------

fn infer_project_name(root: &Path) -> String {
    // Prefer package.json "name", then Cargo.toml [package].name, then dir.
    if let Ok(text) = fs::read_to_string(root.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("Cargo.toml")) {
        if let Some(name) = extract_cargo_package_name(&text) {
            return name;
        }
    }
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "project".to_string())
}

fn extract_cargo_package_name(text: &str) -> Option<String> {
    // Intentionally avoids pulling in a full TOML parser for this crate.
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("name") {
                let rest = rest.trim_start().trim_start_matches('=').trim();
                let name = rest.trim_matches(|c| c == '"' || c == '\'');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// README extraction
// ---------------------------------------------------------------------------

/// Read the README and return `(first_paragraph, full_text)`. The full text
/// is also returned so the concept extractor can run over a larger corpus.
fn read_domain_summary(root: &Path) -> (String, String) {
    // Try common README filenames in priority order.
    let candidates: [PathBuf; 3] = [
        root.join("README.md"),
        root.join("Readme.md"),
        root.join("readme.md"),
    ];
    let readme = candidates
        .iter()
        .find(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_default();
    let summary = first_non_heading_paragraph(&readme);
    (summary, readme)
}

/// Take the first block of non-heading lines that contains prose. Skips
/// heading lines (`#` / `##` / …), leading blank lines, and common noise
/// like badge-only lines and HTML-div blocks used in fancy READMEs.
fn first_non_heading_paragraph(text: &str) -> String {
    let mut buf = String::new();
    let mut inside_html_block = false;
    for raw in text.lines() {
        let line = raw.trim();

        // Handle HTML blocks (e.g. <div align="center"> … </div>).
        if line.starts_with('<') && !line.starts_with("</") {
            inside_html_block = true;
        }
        if inside_html_block {
            if line.starts_with("</") || line.ends_with('>') && line.contains("</") {
                inside_html_block = false;
            }
            continue;
        }

        if line.is_empty() {
            if !buf.is_empty() {
                break;
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with("---") {
            continue;
        }
        // Skip markdown badge-only lines (links with inline image and no prose).
        if line.starts_with("![") || line.starts_with("[!") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(')') && !line.contains(". ") {
            continue;
        }

        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(line);
    }

    // Clamp to ~500 chars so the summary never blows the primer budget.
    const MAX: usize = 500;
    if buf.chars().count() > MAX {
        let truncated: String = buf.chars().take(MAX).collect();
        format!("{truncated}…")
    } else {
        buf
    }
}

// ---------------------------------------------------------------------------
// Key concepts
// ---------------------------------------------------------------------------

fn extract_key_concepts(readme_text: &str, stack: &[Technology]) -> Vec<Concept> {
    // The brain crate already ships a deterministic concept extractor. We
    // synthesise a small corpus from the README plus stack names so that
    // even a blank README still yields something useful.
    let mut corpus = String::new();
    corpus.push_str(readme_text);
    corpus.push('\n');
    for t in stack {
        corpus.push_str(&t.name);
        corpus.push('\n');
    }

    let extractor = ConceptExtractor::new();
    let mut concepts = extractor
        .extract(ExtractInput {
            kind: "readme",
            text: &corpus,
        })
        .unwrap_or_default();

    // Keep top 10 by score. ConceptExtractor already sorts descending.
    concepts.truncate(10);
    concepts
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
    }

    #[test]
    fn detects_node_react_electron_vite() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{
              "name": "demo-app",
              "dependencies": { "react": "^18.0.0", "electron": "28.0.0" },
              "devDependencies": { "vite": "^5.0.0", "typescript": "^5.3.0" }
            }"#,
        );
        let stack = detect_stack(dir.path());
        let names: Vec<&str> = stack.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"node"));
        assert!(names.contains(&"react"));
        assert!(names.contains(&"electron"));
        assert!(names.contains(&"vite"));
        assert!(names.contains(&"typescript"));
    }

    #[test]
    fn detects_rust_and_tauri() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            r#"[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = "2"
tokio = "1"
"#,
        );
        let stack = detect_stack(dir.path());
        let names: Vec<&str> = stack.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"rust"));
        assert!(names.contains(&"tauri"));
        assert!(names.contains(&"tokio"));
    }

    #[test]
    fn extracts_domain_summary_from_readme() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "README.md",
            "# My Project\n\nThis tool organises files automatically.\nIt runs locally.\n\n## Install\n",
        );
        let (summary, _) = read_domain_summary(dir.path());
        assert!(summary.contains("organises files"));
        assert!(!summary.contains("My Project"));
    }

    #[test]
    fn project_name_from_package_json() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", r#"{"name":"cool-app"}"#);
        assert_eq!(infer_project_name(dir.path()), "cool-app");
    }

    #[test]
    fn project_name_from_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"rusty\"\nversion = \"0.1.0\"\n",
        );
        assert_eq!(infer_project_name(dir.path()), "rusty");
    }

    #[test]
    fn build_produces_markdown() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{"name":"cool-app","dependencies":{"react":"18"}}"#,
        );
        write(
            dir.path(),
            "README.md",
            "# Project Overview\n\nA Small Tool For Indexing Files automatically.\n",
        );
        let id =
            ProjectIdentity::build(dir.path(), vec!["goal1".into()], vec!["q?".into()], vec![]);
        let md = id.to_markdown();
        assert!(md.contains("Project: cool-app"));
        assert!(md.contains("react"));
        assert!(md.contains("Small Tool") || md.contains("small tool"));
        assert!(md.contains("goal1"));
        assert!(md.contains("q?"));
    }
}
