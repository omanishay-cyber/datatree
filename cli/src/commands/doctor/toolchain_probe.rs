//! Developer-toolchain probes (G1-G12), the `--strict` pre-flight
//! verifier, and the Windows MSVC build-toolchain check.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use super::daemon_probe::which_on_path;
use super::render::{line, print_banner, utc_now_readable};
use crate::commands::doctor::DoctorRow;

// ─── ToolSeverity ─────────────────────────────────────────────────────────────

/// Severity tier for a missing toolchain entry.
///
/// `--strict` returns non-zero when ANY High-severity tool is missing.
/// Medium / Low surface as warnings only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSeverity {
    High,
    Medium,
    Low,
}

impl ToolSeverity {
    pub const fn label(self) -> &'static str {
        match self {
            ToolSeverity::High => "HIGH",
            ToolSeverity::Medium => "MED ",
            ToolSeverity::Low => "LOW ",
        }
    }
}

// ─── ToolchainEntry ────────────────────────────────────────────────────────────

/// Canonical entry for one developer-toolchain dependency. Single source
/// of truth shared by `mneme doctor --strict` and the capability summary
/// printed at the end of `mneme install`. See `KNOWN_TOOLCHAIN` for
/// the full list.
#[derive(Debug, Clone, Copy)]
pub struct ToolchainEntry {
    pub display: &'static str,
    pub probes: &'static [&'static str],
    pub cargo_subcommand: Option<&'static str>,
    pub severity: ToolSeverity,
    pub issue_id: &'static str,
    pub purpose: &'static str,
    pub hint_windows: &'static str,
    pub hint_unix: &'static str,
}

// ─── KNOWN_TOOLCHAIN ──────────────────────────────────────────────────────────

/// Canonical list of every dev-toolchain dependency mneme cares about.
/// Order = display order in `mneme doctor --strict` and install summary.
/// Closes G1-G12 from `phase-a-issues.md` §G.
///
/// IMPORTANT: both `scripts/install.ps1` and `scripts/install.sh` mirror
/// these entries — keep all three in sync when adding / removing a tool.
pub const KNOWN_TOOLCHAIN: &[ToolchainEntry] = &[
    ToolchainEntry {
        display: "Rust toolchain (rustc + cargo)",
        probes: &["rustc", "cargo"],
        cargo_subcommand: None,
        severity: ToolSeverity::High,
        issue_id: "G1",
        purpose: "vision/tauri/ build, future Rust-port work, workspace builds",
        hint_windows: "winget install Rustlang.Rustup",
        hint_unix: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh",
    },
    ToolchainEntry {
        display: "Bun",
        probes: &["bun"],
        cargo_subcommand: None,
        severity: ToolSeverity::High,
        issue_id: "G2",
        purpose: "vision app runtime + MCP server (mneme mcp stdio)",
        hint_windows: "irm bun.sh/install.ps1 | iex",
        hint_unix: "curl -fsSL https://bun.sh/install | bash",
    },
    ToolchainEntry {
        display: "Node.js",
        probes: &["node"],
        cargo_subcommand: None,
        severity: ToolSeverity::High,
        issue_id: "G3",
        purpose: "Claude Code CLI install, JS-tooling fallbacks, npm-based installers",
        hint_windows: "winget install OpenJS.NodeJS.LTS",
        hint_unix: "use nvm: curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.0/install.sh | bash",
    },
    ToolchainEntry {
        display: "Tauri CLI",
        probes: &["tauri"],
        cargo_subcommand: Some("tauri"),
        severity: ToolSeverity::Medium,
        issue_id: "G4",
        purpose: "ergonomic Tauri builds (tauri build, tauri dev) for vision/",
        hint_windows: "cargo install tauri-cli --version \"^2.0\"",
        hint_unix: "cargo install tauri-cli --version \"^2.0\"",
    },
    ToolchainEntry {
        display: "Git",
        probes: &["git"],
        cargo_subcommand: None,
        severity: ToolSeverity::High,
        issue_id: "G5",
        purpose: "git.db shard population (commits / blame / history), Why-Chain trace",
        hint_windows: "winget install Git.Git",
        hint_unix: "brew install git (macOS) | sudo apt install git (Debian/Ubuntu)",
    },
    ToolchainEntry {
        display: "Python",
        probes: &["python", "python3"],
        cargo_subcommand: None,
        severity: ToolSeverity::Medium,
        issue_id: "G6",
        purpose: "PNG->ICO icon conversion (PIL), multimodal sidecar, scanner fallbacks",
        hint_windows: "winget install Python.Python.3.11",
        hint_unix: "brew install python@3.11 (macOS) | sudo apt install python3 (Debian/Ubuntu)",
    },
    ToolchainEntry {
        display: "SQLite CLI",
        probes: &["sqlite3"],
        cargo_subcommand: None,
        severity: ToolSeverity::Low,
        issue_id: "G7",
        purpose: "manual shard inspection (sqlite3 graph.db .schema). Drivers are bundled.",
        hint_windows: "winget install SQLite.SQLite",
        hint_unix: "brew install sqlite (macOS) | sudo apt install sqlite3 (Debian/Ubuntu)",
    },
    ToolchainEntry {
        display: "Java JDK",
        probes: &["java"],
        cargo_subcommand: None,
        severity: ToolSeverity::Low,
        issue_id: "G8",
        purpose: "optional — only if a future feature needs the JVM (currently unused)",
        hint_windows: "winget install EclipseAdoptium.Temurin.21.JDK",
        hint_unix: "brew install openjdk@21 (macOS) | sudo apt install openjdk-21-jdk (Debian/Ubuntu)",
    },
    ToolchainEntry {
        display: "Tesseract OCR",
        probes: &["tesseract"],
        cargo_subcommand: None,
        severity: ToolSeverity::Medium,
        issue_id: "G9",
        purpose: "image OCR via multimodal sidecar (binary feature-gated; rebuild with --features tesseract to enable)",
        hint_windows: "winget install UB-Mannheim.TesseractOCR",
        hint_unix: "brew install tesseract (macOS) | sudo apt install tesseract-ocr (Debian/Ubuntu)",
    },
    ToolchainEntry {
        display: "ImageMagick",
        probes: &["magick"],
        cargo_subcommand: None,
        severity: ToolSeverity::Low,
        issue_id: "G10",
        // A8-009 (2026-05-04): clarified purpose. ImageMagick is NEVER
        // invoked by mneme-multimodal; OCR uses Tesseract (G9) only.
        purpose: "icon-pipeline: PNG->ICO fallback for Tauri app-icon build (NOT used by multimodal OCR)",
        hint_windows: "winget install ImageMagick.ImageMagick",
        hint_unix: "brew install imagemagick (macOS) | sudo apt install imagemagick (Debian/Ubuntu)",
    },
    // A8-008 (2026-05-04): ffmpeg + Whisper model entries.
    ToolchainEntry {
        display: "FFmpeg",
        probes: &["ffmpeg"],
        cargo_subcommand: None,
        severity: ToolSeverity::Low,
        issue_id: "G11",
        purpose: "video frame extraction via multimodal sidecar (--features ffmpeg). \
                  Binary not used directly; libav FFI links to system ffmpeg libraries.",
        hint_windows: "winget install Gyan.FFmpeg",
        hint_unix: "brew install ffmpeg (macOS) | sudo apt install ffmpeg (Debian/Ubuntu)",
    },
    // G12: Whisper CLI binary probe (whisper-cli / main from whisper.cpp).
    ToolchainEntry {
        display: "Whisper CLI (whisper-cli / multilingual)",
        probes: &["whisper-cli", "whisper"],
        cargo_subcommand: None,
        severity: ToolSeverity::Low,
        issue_id: "G12",
        purpose: "audio/video transcription via whisper.cpp shellout fallback \
                  (used when mneme binary was NOT compiled with --features whisper). \
                  Also requires ggml-tiny.en.bin in ~/.mneme/models/whisper/.",
        hint_windows: "Build whisper.cpp from source (cmake) or download a pre-built \
                       whisper-cli.exe from https://github.com/ggerganov/whisper.cpp/releases \
                       and add to PATH. Then place ggml-tiny.en.bin in \
                       %USERPROFILE%\\.mneme\\models\\whisper\\",
        hint_unix: "brew install whisper-cpp (macOS) | build from source: \
                    git clone https://github.com/ggerganov/whisper.cpp && cd whisper.cpp && \
                    cmake -B build && cmake --build build -j. \
                    Then place ggml-tiny.en.bin in ~/.mneme/models/whisper/",
    },
];

// ─── ToolProbe ────────────────────────────────────────────────────────────────

/// Outcome of probing one toolchain entry on this host.
#[derive(Debug, Clone)]
pub struct ToolProbe {
    pub entry: ToolchainEntry,
    /// Path of the first matching binary, or None if no probe hit.
    pub found_at: Option<PathBuf>,
    /// `--version` output (first line, trimmed) or None if probe failed.
    pub version: Option<String>,
}

impl ToolProbe {
    pub fn is_present(&self) -> bool {
        self.found_at.is_some()
    }
}

// ─── probe functions ─────────────────────────────────────────────────────────

/// Probe one toolchain entry: try each binary in `entry.probes` on
/// PATH, fall back to `cargo <subcommand>` if present. First hit wins.
pub fn probe_tool(entry: &ToolchainEntry) -> ToolProbe {
    for bin in entry.probes {
        if let Some(path) = which_on_path(bin) {
            let version = run_version_probe(&path);
            return ToolProbe {
                entry: *entry,
                found_at: Some(path),
                version,
            };
        }
    }

    if let Some(sub) = entry.cargo_subcommand {
        if let Some(cargo) = which_on_path("cargo") {
            let out = StdCommand::new(&cargo)
                .args([sub, "--version"])
                .output()
                .ok();
            if let Some(o) = out {
                if o.status.success() {
                    let v = String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    return ToolProbe {
                        entry: *entry,
                        found_at: Some(cargo),
                        version: if v.is_empty() { None } else { Some(v) },
                    };
                }
            }
        }
    }

    ToolProbe {
        entry: *entry,
        found_at: None,
        version: None,
    }
}

/// Run `<bin> --version` and return the first non-empty line, trimmed.
/// Returns None on any error so the caller can render a "version
/// unknown" row without crashing the doctor.
fn run_version_probe(bin: &Path) -> Option<String> {
    let out = StdCommand::new(bin).arg("--version").output().ok()?;
    if !out.status.success() {
        // Some tools (notably older `java`) print --version on stderr.
        let s = String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        return if s.is_empty() { None } else { Some(s) };
    }
    let s = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if s.is_empty() {
        let s2 = String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if s2.is_empty() {
            None
        } else {
            Some(s2)
        }
    } else {
        Some(s)
    }
}

/// Probe every entry in [`KNOWN_TOOLCHAIN`] and return the results in
/// canonical order.
pub fn probe_all_toolchain() -> Vec<ToolProbe> {
    KNOWN_TOOLCHAIN.iter().map(probe_tool).collect()
}

/// G12 model-file probe: check whether a Whisper GGML model is present
/// in `~/.mneme/models/whisper/` and report as a [`DoctorRow`].
///
/// This is a *file-presence* probe, not a binary-on-PATH probe.
pub fn probe_whisper_model() -> DoctorRow {
    use multimodal::whisper::{best_model_path, WhisperModel};

    let label = "  [LOW ] G12 Whisper model (file)".to_string();
    match best_model_path() {
        Some((path, WhisperModel::TinyMultilingual)) => DoctorRow::new(
            label,
            format!("present (multilingual ✓) — {}", path.display()),
        ),
        Some((path, WhisperModel::TinyEn)) => DoctorRow::new(
            label,
            format!("present (English-only) — {}", path.display()),
        ),
        None => {
            let dir_hint = multimodal::whisper::whisper_model_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| "~/.mneme/models/whisper/".into());
            DoctorRow::new(
                label,
                format!(
                    "absent — audio ingestion disabled. \
                     Place ggml-tiny.en.bin in {dir_hint} to enable."
                ),
            )
        }
    }
}

/// G12 combined row: whisper runtime availability (binary + model + feature).
pub fn probe_whisper_runtime_summary() -> DoctorRow {
    use multimodal::{whisper_runtime_available, WHISPER_FEATURE_ENABLED};

    let label = "  [LOW ] G12 Whisper transcription (runtime)".to_string();
    let value = if whisper_runtime_available() {
        if WHISPER_FEATURE_ENABLED {
            "present (compiled FFI ✓ + model ✓)".to_string()
        } else {
            "present (shellout fallback ✓ + model ✓)".to_string()
        }
    } else if WHISPER_FEATURE_ENABLED {
        "feature compiled in but model MISSING — audio ingestion disabled".to_string()
    } else {
        "absent (no --features whisper and no whisper-cli on PATH)".to_string()
    };
    DoctorRow::new(label, value)
}

/// Choose the platform-appropriate install hint for a tool.
pub fn install_hint_for(entry: &ToolchainEntry) -> &'static str {
    if cfg!(windows) {
        entry.hint_windows
    } else {
        entry.hint_unix
    }
}

// ─── render helpers ───────────────────────────────────────────────────────────

/// Render the developer-toolchain section. Used by both the regular
/// `mneme doctor` output and `--strict`. Returns `true` if every
/// HIGH-severity tool was found (so `--strict` knows whether to exit
/// non-zero).
pub fn render_toolchain_box(probes: &[ToolProbe]) -> bool {
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ developer toolchain (G1-G12)                            │");
    println!("├─────────────────────────────────────────────────────────┤");
    let mut all_high_present = true;
    for probe in probes {
        let mark = if probe.is_present() { "✓" } else { "✗" };
        let label = format!(
            "{mark} [{}] {}",
            probe.entry.severity.label(),
            probe.entry.display
        );
        let value = match (&probe.found_at, &probe.version) {
            (Some(_), Some(v)) => v.clone(),
            (Some(p), None) => format!("present at {}", p.display()),
            (None, _) => format!("MISSING — {}", probe.entry.issue_id),
        };
        line(&label, &value);
        if probe.entry.severity == ToolSeverity::High && !probe.is_present() {
            all_high_present = false;
        }
    }
    println!("└─────────────────────────────────────────────────────────┘");

    // B10: per-tool fix hints for everything missing — printed below
    // the box so the table stays readable.
    let missing: Vec<&ToolProbe> = probes.iter().filter(|p| p.found_at.is_none()).collect();
    if !missing.is_empty() {
        println!();
        println!("install hints for missing tools:");
        for probe in missing {
            println!(
                "  [{}] {} ({}): {}",
                probe.entry.severity.label().trim(),
                probe.entry.display,
                probe.entry.issue_id,
                install_hint_for(&probe.entry),
            );
        }
    }

    all_high_present
}

// ─── MSVC install hint ────────────────────────────────────────────────────────

/// Suggested install hint shown when the doctor cannot find any MSVC
/// compiler / linker on this machine. Used by both the live probe and
/// the unit-test that pins the message text. Closes I-16.
pub const MSVC_INSTALL_HINT: &str =
    "(optional, dev-only) MSVC Build Tools not installed — only needed if you plan to build mneme from source. Install via `winget install Microsoft.VisualStudio.2022.BuildTools` or VS Installer if you want to.";

// ─── Windows MSVC build-toolchain check ──────────────────────────────────────

/// Probe the Windows MSVC build toolchain. Closes I-16.
///
/// On non-Windows returns an empty Vec so the box is skipped entirely.
#[cfg(windows)]
pub fn check_build_toolchain() -> Vec<DoctorRow> {
    let mut rows = Vec::new();

    let mut link_ok = which_on_path("link").is_some();
    let mut cl_ok = which_on_path("cl").is_some();

    let vswhere_path = which_on_path("vswhere").or_else(|| {
        let pf =
            std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))?;
        let candidate = std::path::PathBuf::from(pf)
            .join("Microsoft Visual Studio")
            .join("Installer")
            .join("vswhere.exe");
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    });

    let mut vc_tools_install: Option<String> = None;
    let mut vc_tools_compiler_dir: Option<std::path::PathBuf> = None;
    let vswhere_row_value: String;
    match vswhere_path {
        Some(p) => {
            let install = std::process::Command::new(&p)
                .args([
                    "-latest",
                    "-products",
                    "*",
                    "-requires",
                    "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                    "-property",
                    "installationPath",
                ])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    } else {
                        None
                    }
                });
            match install {
                Some(install_path) => {
                    vswhere_row_value = format!("ok ({install_path})");
                    if let Some(bin_dir) = locate_vc_compiler_bin(&install_path) {
                        if !link_ok && bin_dir.join("link.exe").is_file() {
                            link_ok = true;
                        }
                        if !cl_ok && bin_dir.join("cl.exe").is_file() {
                            cl_ok = true;
                        }
                        vc_tools_compiler_dir = Some(bin_dir);
                    }
                    vc_tools_install = Some(install_path);
                }
                None => {
                    vswhere_row_value =
                        "(optional, dev-only) — VS Installer present but no VC.Tools.x86.x64 component"
                            .to_string();
                }
            }
        }
        None => {
            vswhere_row_value =
                "(optional, dev-only) — install Visual Studio Installer for source builds"
                    .to_string();
        }
    }

    rows.push(DoctorRow::new(
        "link.exe",
        if link_ok {
            match &vc_tools_compiler_dir {
                Some(d) if which_on_path("link").is_none() => {
                    format!("ok (via vswhere: {})", d.display())
                }
                _ => "ok".to_string(),
            }
        } else {
            "(optional, dev-only) — only needed for building mneme from source".to_string()
        },
    ));
    rows.push(DoctorRow::new(
        "cl.exe",
        if cl_ok {
            match &vc_tools_compiler_dir {
                Some(d) if which_on_path("cl").is_none() => {
                    format!("ok (via vswhere: {})", d.display())
                }
                _ => "ok".to_string(),
            }
        } else {
            "(optional, dev-only) — only needed for building mneme from source".to_string()
        },
    ));

    if vc_tools_install.is_some() {
        rows.push(DoctorRow::new("VC Tools", vswhere_row_value.clone()));
    } else {
        let label = if vswhere_row_value.starts_with("MISSING — install Visual Studio Installer")
        {
            "vswhere.exe"
        } else {
            "VC Tools"
        };
        rows.push(DoctorRow::new(label, vswhere_row_value));
    }

    let sdk_lib = locate_windows_sdk_kernel32_lib();
    match &sdk_lib {
        Some(path) => rows.push(DoctorRow::new("Windows SDK", format!("ok ({path})"))),
        None => rows.push(DoctorRow::new(
            "Windows SDK",
            "(optional, dev-only) — install Windows 10/11 SDK only if building from source",
        )),
    }

    // FAIL → WARN (Bug REL-5/D-2, 2026-05-01): MSVC is only required to
    // BUILD mneme from source. Binary-installer users never need it.
    let any_compiler = link_ok || cl_ok;
    let toolchain_ok = any_compiler && (vc_tools_install.is_some() || sdk_lib.is_some());
    rows.push(DoctorRow::new(
        "summary",
        if toolchain_ok {
            "PASS — MSVC build toolchain available".to_string()
        } else {
            format!("WARN — only needed for building from source ({MSVC_INSTALL_HINT})")
        },
    ));

    rows
}

/// Resolve the concrete MSVC compiler bin directory for a Visual Studio
/// install. Returns `None` if anything along the chain is missing.
#[cfg(windows)]
fn locate_vc_compiler_bin(install_path: &str) -> Option<std::path::PathBuf> {
    let install = std::path::Path::new(install_path);
    let ver_file = install
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("Microsoft.VCToolsVersion.default.txt");
    let ver = std::fs::read_to_string(&ver_file).ok()?.trim().to_string();
    if ver.is_empty() {
        return None;
    }
    let bin = install
        .join("VC")
        .join("Tools")
        .join("MSVC")
        .join(&ver)
        .join("bin")
        .join("Hostx64")
        .join("x64");
    if bin.is_dir() {
        Some(bin)
    } else {
        None
    }
}

/// Walk `%ProgramFiles(x86)%\Windows Kits\10\Lib\*\um\x64\kernel32.lib`
/// and return the highest-numbered SDK directory that contains the lib.
#[cfg(windows)]
fn locate_windows_sdk_kernel32_lib() -> Option<String> {
    let pf = std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))?;
    let lib_root = std::path::PathBuf::from(pf)
        .join("Windows Kits")
        .join("10")
        .join("Lib");
    let read = std::fs::read_dir(&lib_root).ok()?;
    let mut versions: Vec<std::path::PathBuf> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    versions.sort();
    versions.reverse();
    for v in versions {
        let kernel32 = v.join("um").join("x64").join("kernel32.lib");
        if kernel32.is_file() {
            return Some(kernel32.display().to_string());
        }
    }
    None
}

#[cfg(not(windows))]
pub fn check_build_toolchain() -> Vec<DoctorRow> {
    Vec::new()
}

/// Render the build-toolchain section. No-ops on non-Windows.
pub fn print_build_toolchain_section() {
    let rows = check_build_toolchain();
    if rows.is_empty() {
        return;
    }
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ build toolchain (Windows MSVC, optional - source builds)│");
    println!("├─────────────────────────────────────────────────────────┤");
    for row in rows {
        line(&row.label, &row.value);
    }
    println!("└─────────────────────────────────────────────────────────┘");
}

// ─── run_strict ───────────────────────────────────────────────────────────────

/// G11 strict-mode entry point. Runs all G1-G12 probes, verifies every
/// binary in `~/.mneme/bin/` launches with `--version` cleanly, probes
/// the optional vision app, and returns a non-zero exit code if any
/// HIGH-severity check failed.
pub fn run_strict() -> i32 {
    print_banner();
    println!();
    println!("  {:<16}{}", "timestamp:", utc_now_readable());
    println!("  {:<16}strict (G11 pre-flight verification)", "mode:");
    println!();

    let mut all_ok = true;

    let probes = probe_all_toolchain();
    let toolchain_ok = render_toolchain_box(&probes);
    if !toolchain_ok {
        all_ok = false;
    }

    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ binary self-test (~/.mneme/bin/* --version)             │");
    println!("├─────────────────────────────────────────────────────────┤");
    let bin_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut binary_failures = 0usize;
    if let Some(dir) = bin_dir {
        for b in super::expected_binary_names() {
            let p = dir.join(b);
            if !p.exists() {
                line(&format!("✗ {b}"), "MISSING on disk");
                binary_failures += 1;
                continue;
            }
            let is_cli = b.starts_with("mneme.") || *b == "mneme";
            if !is_cli {
                line(
                    &format!("✓ {b}"),
                    "present (no --version probe — IPC binary)",
                );
                continue;
            }
            match StdCommand::new(&p).arg("--version").output() {
                Ok(out) if out.status.success() => {
                    let v = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    line(&format!("✓ {b}"), &v);
                }
                Ok(out) => {
                    binary_failures += 1;
                    line(
                        &format!("✗ {b}"),
                        &format!("--version exited {}", out.status.code().unwrap_or(-1)),
                    );
                }
                Err(e) => {
                    binary_failures += 1;
                    line(&format!("✗ {b}"), &format!("spawn failed: {e}"));
                }
            }
        }
    } else {
        line("✗ bin dir", "could not resolve current_exe parent");
        binary_failures += 1;
    }
    println!("└─────────────────────────────────────────────────────────┘");
    if binary_failures > 0 {
        all_ok = false;
    }

    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ optional: vision app (mneme-vision)                     │");
    println!("├─────────────────────────────────────────────────────────┤");
    let vision_bin = if cfg!(windows) {
        "mneme-vision.exe"
    } else {
        "mneme-vision"
    };
    let vision_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|d| d.join(vision_bin));
    match vision_path.as_ref().filter(|p| p.exists()) {
        Some(p) => line(&format!("✓ {vision_bin}"), &p.display().to_string()),
        None => line(
            &format!("- {vision_bin}"),
            "not installed (vision app is optional — install with `cargo build --release -p mneme-vision`)",
        ),
    }
    println!("└─────────────────────────────────────────────────────────┘");

    print_build_toolchain_section();

    println!();
    if all_ok {
        println!("strict pre-flight: PASS — all HIGH-severity toolchain present, binaries healthy");
        0
    } else {
        println!("strict pre-flight: FAIL — see install hints above + run individual fix commands");
        1
    }
}
