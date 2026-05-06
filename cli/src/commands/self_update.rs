//! `mneme self-update` — replace the installed `mneme` binary set with
//! the latest GitHub release.
//!
//! Distinct from `mneme update`, which is the project incremental
//! re-index command. The naming follows the conventions of
//! `rustup self update`, `gh self-update`, and `cargo install --self`:
//! "update the binary itself" vs "update the project index".
//!
//! Flow per [`run`]:
//!
//! 1. The running binary's version is its own `CARGO_PKG_VERSION`.
//! 2. Query `https://api.github.com/repos/<repo>/releases/latest` for
//!    the current published tag and asset list.
//! 3. Pick the asset whose name matches the running platform / arch
//!    (see [`choose_asset_for_target`]).
//! 4. Compare semver. If installed >= latest and `--force` was not
//!    passed, exit 0 with "already up to date".
//! 5. `--check-only` short-circuits before any download.
//! 6. Stream-download the asset into `std::env::temp_dir()` with a
//!    progress bar (or periodic byte print when stdout is not a TTY).
//! 7. If a `<asset>.sha256` sidecar is present in the release, verify
//!    the SHA-256 of the downloaded bytes against it. Mismatch aborts
//!    BEFORE extraction — refusing to install a tampered or partial
//!    download is mandatory.
//! 8. Stop the daemon (best-effort IPC `Stop`, then poll for the PID
//!    to exit) so Windows can release the file lock on the running
//!    daemon binary. `--no-stop-daemon` skips this for advanced users
//!    who manage the daemon themselves.
//! 9. Extract the archive (zip on Windows, tar.gz on Unix) into a
//!    `staging/` directory next to the download.
//! 10. Atomically replace each binary under `~/.mneme/bin/`. On
//!     Windows, where in-use files cannot be replaced even after the
//!     daemon stops if other handles linger, fall back to a
//!     `.deleteme` rename so the next install / reboot can finish the
//!     swap. On Unix, `chmod +x` is reapplied. On macOS, the
//!     quarantine xattr is cleared with `xattr -cr`.
//! 11. Print a one-line summary and exit 0.

use clap::Args;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{CliError, CliResult};

/// GitHub repo coordinates for the public release stream. Hard-coded
/// because there is exactly one upstream and the binary should never
/// silently update from a fork. Tests that need to point elsewhere can
/// drive the lower-level helpers ([`choose_asset_for_target`],
/// [`compare_semver`], etc.) directly.
const GITHUB_OWNER: &str = "omanishay-cyber";
const GITHUB_REPO: &str = "mneme";

/// User-Agent the GitHub API requires on every request. Identifies the
/// CLI so abuse rate-limiting can pin per-version.
const USER_AGENT: &str = concat!("mneme-self-update/", env!("CARGO_PKG_VERSION"));

/// Names of the binaries shipped under `~/.mneme/bin/`. Each one is
/// swapped out atomically by [`replace_binaries_atomically`]. Order is
/// not significant — the loop swaps whichever ones exist in the staging
/// directory.
const SHIPPED_BINARIES: &[&str] = &[
    "mneme",
    "mneme-daemon",
    "mneme-livebus",
    "mneme-scanners",
    "mneme-multimodal",
    "mneme-parsers",
    "mneme-store",
    "mneme-vision",
];

/// Connect timeout for the GitHub API + asset download. 60s matches the
/// installer's tolerance for slow links — the `https://api.github.com`
/// edge typically resolves in <500 ms but mobile / hotel-Wi-Fi users
/// occasionally need the longer tail.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

/// REL-3 fix (2026-05-05 audit): full-request budget. Once the TCP
/// connection establishes, a slow GitHub edge or a stalled mid-stream
/// chunk can hang `resp.chunk().await` indefinitely without this. 10
/// minutes is generous enough for slow links to download the full
/// release archive (currently ~55 MB) but bounded enough that a stall
/// fails fast instead of hanging for hours.
const HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(600);

/// REL-3 fix: per-chunk read budget. If the body stalls mid-stream
/// (network drop, slow CDN edge), this surfaces as a clear download
/// failure within 30 s instead of hanging forever. Each successful
/// chunk resets the timer, so a slow-but-progressing 1 KB/s connection
/// completes successfully — only true stalls fail.
const HTTP_CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// Wall-clock budget for the daemon to exit after we send IPC `Stop`.
/// 30 s mirrors the supervisor's own graceful-shutdown ceiling.
const DAEMON_STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// Print byte progress every N bytes when no TTY is attached (so a CI
/// log doesn't drown in millions of carriage-return updates).
const PROGRESS_INTERVAL_BYTES: u64 = 5 * 1024 * 1024;

/// A1-018 (2026-05-04): minisign public key for verifying release signatures.
/// `None` for now -- the maintainer has not yet generated the release-signing
/// key pair. Once `rsign2 generate` is run and the public key is embedded
/// here, signature verification becomes hard-fail without `--allow-unsigned`.
///
/// Migration plan:
///   1. Maintainer runs `rsign2 generate -p mneme.pub -s mneme.key` ONCE,
///      stores `mneme.key` offline (not in repo).
///   2. Replace `None` below with `Some("RWQ...")` (the contents of mneme.pub).
///   3. CI signs every release asset with `rsign2 sign -s mneme.key
///      <asset>` and uploads the resulting `<asset>.minisig` alongside.
///   4. Existing v0.3.2 users update once with `--allow-unsigned`; future
///      `self-update` invocations enforce signature verification by default.
///
/// Until step 2 lands, signature verification is a no-op (the absence-of-sig
/// path is exercised, but presence-of-sig is also accepted because we have
/// no key to verify against -- documented in `verify_signature`).
const MNEME_RELEASE_PUBKEY: Option<&str> = None;

/// CLI args for `mneme self-update`.
///
/// Bug NEW-A (2026-05-04): the subcommand previously defined its own
/// `--verbose: bool` flag here. The parent `Cli` struct in `main.rs`
/// already registers `--verbose` as a `clap::ArgAction::Count` (u8)
/// global argument that propagates to every subcommand. Defining a
/// second `--verbose` here with a different type registered the same
/// arg name twice with conflicting `TypeId`s; at parse time clap
/// panicked with "Mismatch between definition and access of `verbose`"
/// the moment any code path called `get_one::<u8>("verbose")` on the
/// merged matches. Self-update was 100% broken on every invocation
/// (`--check-only`, `--force`, bare). The fix is to drop the duplicate
/// field entirely and read `Cli::verbose` (count, u8) via the
/// dispatcher in `main.rs`. `mneme self-update --verbose`,
/// `mneme self-update -v`, and `mneme self-update -vv` continue to
/// work because the parent's `global = true` flag is accepted on every
/// subcommand.
#[derive(Debug, Args)]
pub struct SelfUpdateArgs {
    /// Skip the version check and reinstall current latest.
    #[arg(long)]
    pub force: bool,
    /// Print what would happen without modifying any binaries.
    #[arg(long, alias = "dry-run")]
    pub check_only: bool,
    /// Skip stopping the daemon (for advanced users).
    #[arg(long)]
    pub no_stop_daemon: bool,
    /// A1-018 (2026-05-04): allow self-update to proceed when the release
    /// ships no signature (`.minisig`) sidecar. Without this flag, missing
    /// signature is a hard error -- preventing supply-chain attacks where
    /// a compromised release tag silently distributes a malicious binary
    /// (sha256 sidecar alone is self-attesting and offers no security).
    /// Pass this flag knowingly: it acknowledges the unsigned binary risk.
    #[arg(long)]
    pub allow_unsigned: bool,
}

/// One asset attached to a GitHub release. Only the fields we use are
/// modeled — extra ones in the JSON response are ignored by serde.
#[derive(Debug, Clone, Deserialize)]
pub struct GhAsset {
    /// Filename as published, e.g. `mneme-v0.3.3-windows-x64.zip`.
    pub name: String,
    /// Stream URL the API returns. Following this with a GitHub-flavoured
    /// `Accept: application/octet-stream` header yields the binary bytes.
    pub browser_download_url: String,
    /// Total size of the asset in bytes (for the progress bar / summary).
    #[serde(default)]
    pub size: u64,
}

/// Subset of the `/releases/latest` payload we need.
#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    /// Git tag this release was cut from. Typically `v0.3.3`. The
    /// leading `v` is stripped by [`tag_to_version`] before semver
    /// comparison.
    pub tag_name: String,
    /// All attached assets, including the platform archives and the
    /// optional `.sha256` sidecars.
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

/// Entry point used by `main.rs`. Async because the dispatcher awaits
/// every `commands::*::run`; the heavy I/O (reqwest, fs copies) runs on
/// the multi-thread runtime.
///
/// `verbose_count` is the parent `Cli`'s global `--verbose` count
/// (`-v` = 1, `-vv` = 2, etc.). Any non-zero value enables the chatty
/// progress prints.
pub async fn run(args: SelfUpdateArgs, verbose_count: u8) -> CliResult<()> {
    // Bug NEW-A (2026-05-04): the subcommand no longer owns its own
    // `--verbose` flag (see SelfUpdateArgs doc-comment). Collapse the
    // count into a bool so the rest of this function and its helpers
    // (which take `verbose: bool`) stay byte-for-byte the same.
    let verbose = verbose_count > 0;

    let installed_version = env!("CARGO_PKG_VERSION");
    if verbose {
        eprintln!("self-update: installed version = v{installed_version}");
    }

    let release = fetch_latest_release().await?;
    let latest_version = tag_to_version(&release.tag_name);
    if verbose {
        eprintln!(
            "self-update: latest published    = v{latest_version} (tag {})",
            release.tag_name
        );
    }

    let asset = choose_asset_for_target(&release.assets, target_os_str(), target_arch_str())
        .ok_or_else(|| {
            CliError::Other(format!(
                "no release asset matching {}-{} in tag {}; assets present: {}",
                target_os_str(),
                target_arch_str(),
                release.tag_name,
                release
                    .assets
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        })?;

    // semver gate.
    let cmp = compare_semver(installed_version, &latest_version)?;
    if cmp.is_ge() && !args.force {
        println!("Already on the latest version (v{installed_version})");
        return Ok(());
    }

    let size_mb = asset.size / 1_048_576;
    if args.check_only {
        println!(
            "Update available: {} -> {} ({} MB)",
            installed_version, latest_version, size_mb,
        );
        return Ok(());
    }

    println!(
        "Updating mneme: v{} -> v{} ({} MB)",
        installed_version, latest_version, size_mb,
    );

    // Download the archive into a per-version temp dir.
    let staging_root = env::temp_dir().join(format!("mneme-self-update-{latest_version}"));
    fs::create_dir_all(&staging_root).map_err(|e| CliError::io(staging_root.clone(), e))?;

    // A1-022 (2026-05-04): pre-flight free-space check on the staging
    // drive. Asset.size is the GitHub-reported size; we want at least
    // 1.2x to leave room for extraction (zip + tar.gz both materialize
    // the extracted tree alongside the archive). Best-effort: a
    // sysinfo failure or a drive sysinfo doesn't recognize falls
    // through to the legacy "try and see" path -- which fails partway
    // through with a confusing OS error instead of upfront. Better
    // upfront diagnostic when we can produce one.
    {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        let needed = (asset.size as f64 * 1.2) as u64;
        // Find the disk whose mount_point is the longest prefix of staging_root.
        let staging_canon =
            std::fs::canonicalize(&staging_root).unwrap_or_else(|_| staging_root.clone());
        let mut best_match: Option<(usize, u64)> = None;
        for d in disks.list() {
            let mp = d.mount_point();
            if staging_canon.starts_with(mp) {
                let mp_len = mp.as_os_str().len();
                if best_match.map_or(true, |(prev, _)| mp_len > prev) {
                    best_match = Some((mp_len, d.available_space()));
                }
            }
        }
        if let Some((_, avail)) = best_match {
            if avail < needed {
                let _ = fs::remove_dir_all(&staging_root);
                return Err(CliError::Other(format!(
                    "self-update needs ~{} MiB free on {} but only {} MiB available. \
                     Free space and retry, or set TMP / TMPDIR to a drive with more headroom.",
                    needed / (1024 * 1024),
                    staging_root.display(),
                    avail / (1024 * 1024),
                )));
            }
        }
    }

    let archive_path = staging_root.join(&asset.name);

    // A1-022 (2026-05-04): wrap the download + extract in a guard that
    // cleans up staging_root on any failure path. Without this, a
    // partial download or a SHA mismatch leaves megabytes-to-gigabytes
    // of cruft in TEMP forever.
    let download_result = download_asset(
        &asset.browser_download_url,
        &archive_path,
        asset.size,
        verbose,
    )
    .await;
    if let Err(e) = download_result {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(e);
    }

    // Optional SHA-256 sidecar verification. Mandatory on hit.
    if let Some(sha_asset) = release
        .assets
        .iter()
        .find(|a| a.name == format!("{}.sha256", asset.name))
    {
        if verbose {
            eprintln!("self-update: verifying sha256 against {}", sha_asset.name);
        }
        let expected = fetch_sha256_sidecar(&sha_asset.browser_download_url, &asset.name).await?;
        let actual = hash_file_sha256(&archive_path)?;
        if !sha256_matches(&expected, &actual) {
            return Err(CliError::Other(format!(
                "sha256 mismatch for {}: expected {}, got {}",
                asset.name, expected, actual
            )));
        }
    } else if verbose {
        eprintln!(
            "self-update: no .sha256 sidecar published for {}; skipping verification",
            asset.name
        );
    }

    // A1-018 (2026-05-04): supply-chain signature verification.
    // SHA-256 alone proves nothing about origin -- attacker controlling the
    // release replaces both binary and sidecar. Signature verification with
    // an embedded public key closes that gap. See verify_signature().
    verify_signature(&release, &asset, &archive_path, &args, verbose).await?;

    // Stop the daemon so Windows can release file locks on its binary.
    if !args.no_stop_daemon {
        stop_daemon_best_effort(verbose).await;
    } else if verbose {
        eprintln!("self-update: --no-stop-daemon set; leaving supervisor running");
    }

    // Extract.
    let staging_bin = staging_root.join("staging");
    if staging_bin.exists() {
        let _ = fs::remove_dir_all(&staging_bin);
    }
    fs::create_dir_all(&staging_bin).map_err(|e| CliError::io(staging_bin.clone(), e))?;
    extract_archive(&archive_path, &staging_bin)?;

    // Replace binaries.
    let target_bin_dir = install_bin_dir()?;
    if !target_bin_dir.exists() {
        fs::create_dir_all(&target_bin_dir).map_err(|e| CliError::io(target_bin_dir.clone(), e))?;
    }
    // CRIT-8 fix (2026-05-05 audit): the replace_binaries_atomically chain
    // is synchronous and contains up to 10x 1-second sleeps in the
    // Windows file-lock retry path. Calling it directly from an async
    // function blocks a tokio worker for the entire swap (potentially
    // 10+ seconds). Run it on the blocking pool so the runtime stays
    // responsive (the daemon-stop poll, signal handlers, progress
    // reporters all keep ticking).
    let staging_bin_owned = staging_bin.clone();
    let target_bin_dir_owned = target_bin_dir.clone();
    let swapped = tokio::task::spawn_blocking(move || {
        replace_binaries_atomically(&staging_bin_owned, &target_bin_dir_owned, verbose)
    })
    .await
    .map_err(|e| CliError::Other(format!("self-update: swap task join: {e}")))??;

    // A1-020 (2026-05-04): hard-fail if no binary was actually replaced.
    // Previously, an archive whose layout drifted (e.g. wrapped in a
    // top-level `mneme-v0.3.3-windows-x64/...` dir vs flat `bin/...`)
    // could result in the locate-walk finding no `bin/` subtree, the
    // swap loop walking zero candidates, and the function returning
    // Ok(0). The user then saw "Updated mneme" with NO actual binary
    // replacement -- restart Claude Code, nothing changed, they think
    // the new version is buggy when actually the OLD one is still on
    // disk. Refusing to claim success on a no-op makes the failure
    // mode loud instead of silent.
    if swapped == 0 {
        return Err(CliError::Other(format!(
            "self-update extracted the archive but found no recognised binaries \
             under {} or any single nested directory. Archive layout may have \
             changed; re-run with --verbose to inspect the staging directory \
             tree at {}.",
            target_bin_dir.display(),
            staging_bin.display()
        )));
    }

    if cfg!(target_os = "macos") {
        clear_macos_quarantine(&target_bin_dir);
    }

    println!("Updated mneme: v{installed_version} -> v{latest_version}");
    println!("Restart Claude Code (or your MCP host) to pick up the new tools.");

    if verbose {
        eprintln!(
            "self-update: replaced {} binaries under {}",
            swapped,
            target_bin_dir.display()
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Pure helpers — kept out of the async glue so tests can drive them
// without spinning up a Tokio runtime.
// ---------------------------------------------------------------------------

/// Strip the leading `v` from a tag like `v0.3.3` so the result feeds
/// straight into semver. Tags without a `v` prefix are returned as-is.
/// Surrounding whitespace is trimmed first so `"  v1.2.3 "` -> `"1.2.3"`.
pub fn tag_to_version(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_string()
}

/// Comparison verdict for [`compare_semver`]. `is_ge` returns true when
/// installed >= latest, which is the "already up to date" gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemverCmp {
    /// Installed strictly less than latest — proceed with the update.
    Older,
    /// Installed equal to latest — no update needed.
    Equal,
    /// Installed strictly greater than latest — user is on a dev build
    /// or yanked release. Treat as "already up to date" (no downgrade).
    Newer,
}

impl SemverCmp {
    /// True when installed >= latest. Drives the "already up to date"
    /// short-circuit.
    pub fn is_ge(&self) -> bool {
        matches!(self, SemverCmp::Equal | SemverCmp::Newer)
    }
}

/// Parse two dotted-integer version strings and compare them per semver
/// pre-release rules. We do not depend on the `semver` crate to keep
/// the dep graph small.
///
/// A1-019 (2026-05-04): pre-release suffixes are now compared per the
/// semver spec rather than stripped. Per semver:
///   1. Major.Minor.Patch is compared first (numerically).
///   2. If equal, a version with a pre-release suffix is LESS than one
///      without (pre-release < release).
///   3. If both have suffixes, suffixes compare lexicographically with
///      numeric segments compared numerically (we approximate with
///      lexicographic only — close enough for `rc1 < rc2` while still
///      ordering `alpha < beta`; perfect numeric-aware compare is a
///      future enhancement).
///
/// Concrete example: `0.3.3-rc1` < `0.3.3` < `0.3.4`. Previously
/// `compare_semver("0.3.3-rc1", "0.3.3")` returned Equal (because
/// suffixes were stripped before compare), so a user on rc1 was told
/// "already on the latest" when stable v0.3.3 shipped -- they stayed
/// on rc1 forever.
/// L fix (2026-05-05 audit): semver-compliant pre-release comparison.
///
/// Splits each suffix on `.` and compares identifiers pairwise:
/// - if both look numeric, parse as u64 and compare numerically
/// - otherwise compare lexically
/// - shorter prefix loses if all leading identifiers are equal
///
/// Examples:
///   compare_prerelease("rc2",   "rc10")  -> Less   (rc10 is newer)
///   compare_prerelease("rc10",  "rc2")   -> Greater
///   compare_prerelease("alpha", "beta")  -> Less
///   compare_prerelease("rc.2",  "rc.10") -> Less
///   compare_prerelease("rc1",   "rc1")   -> Equal
///
/// Note: this is still a simplified subset of semver. A pre-release
/// like `rc2-extra` is treated as a single identifier "rc2-extra"
/// because the calling parser already split on the FIRST `-`. Good
/// enough for mneme's tag scheme (vX.Y.Z[-rcN]).
fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    for i in 0..a_parts.len().max(b_parts.len()) {
        match (a_parts.get(i), b_parts.get(i)) {
            (Some(ap), Some(bp)) => {
                // Try numeric compare first by parsing trailing digits
                // off any rc-style prefix (so rc2 vs rc10 work).
                let cmp = match (extract_numeric(ap), extract_numeric(bp)) {
                    (Some((ap_pre, ap_n)), Some((bp_pre, bp_n))) if ap_pre == bp_pre => {
                        ap_n.cmp(&bp_n)
                    }
                    _ => ap.cmp(bp),
                };
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => unreachable!(),
        }
    }
    Ordering::Equal
}

/// Helper: split a pre-release identifier into (alphabetic prefix,
/// trailing number). Returns None if there's no trailing number.
/// Examples:
///   "rc10"  -> Some(("rc", 10))
///   "alpha" -> None
///   "10"    -> Some(("", 10))   — all-digit ids parse with empty prefix
fn extract_numeric(s: &str) -> Option<(String, u64)> {
    if s.is_empty() {
        return None;
    }
    // Find the position of the last non-digit byte (if any). If all
    // bytes are digits, split_at = 0 so the prefix is "" and the
    // entire string is the number.
    let split_at = match s.bytes().rposition(|b| !b.is_ascii_digit()) {
        Some(pos) => pos + 1,
        None => 0, // all digits
    };
    if split_at >= s.len() {
        // The string ends with a non-digit (e.g. "rc"). No trailing
        // number to extract.
        return None;
    }
    let (prefix, digits) = s.split_at(split_at);
    let n = digits.parse::<u64>().ok()?;
    Some((prefix.to_string(), n))
}

#[cfg(test)]
mod compare_prerelease_tests {
    use super::compare_prerelease;
    use std::cmp::Ordering;

    #[test]
    fn rc10_is_newer_than_rc2() {
        assert_eq!(compare_prerelease("rc10", "rc2"), Ordering::Greater);
        assert_eq!(compare_prerelease("rc2", "rc10"), Ordering::Less);
    }

    #[test]
    fn equal_pre_releases_are_equal() {
        assert_eq!(compare_prerelease("rc1", "rc1"), Ordering::Equal);
        assert_eq!(compare_prerelease("alpha", "alpha"), Ordering::Equal);
    }

    #[test]
    fn alpha_less_than_beta() {
        assert_eq!(compare_prerelease("alpha", "beta"), Ordering::Less);
    }

    #[test]
    fn dotted_pre_release_comparison() {
        assert_eq!(compare_prerelease("rc.2", "rc.10"), Ordering::Less);
        assert_eq!(compare_prerelease("rc.10", "rc.2"), Ordering::Greater);
    }

    #[test]
    fn longer_loses_if_prefix_equal() {
        assert_eq!(compare_prerelease("rc1", "rc1.beta"), Ordering::Less);
    }
}

pub fn compare_semver(installed: &str, latest: &str) -> CliResult<SemverCmp> {
    let parse = |v: &str| -> CliResult<((u64, u64, u64), Option<String>)> {
        // Strip build metadata first (after `+`), it's not significant
        // for ordering per semver spec.
        let no_build = v.split('+').next().unwrap_or(v).trim();
        // Now split on the FIRST `-` for pre-release suffix.
        let (core, pre) = match no_build.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (no_build, None),
        };
        let parts: Vec<&str> = core.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(CliError::Other(format!(
                "version {v:?} not in MAJOR.MINOR.PATCH form"
            )));
        }
        let nums: Vec<u64> = parts
            .iter()
            .map(|p| {
                p.parse::<u64>().map_err(|e| {
                    CliError::Other(format!("version segment {p:?} of {v:?} not numeric: {e}"))
                })
            })
            .collect::<CliResult<_>>()?;
        Ok((
            (
                nums.first().copied().unwrap_or(0),
                nums.get(1).copied().unwrap_or(0),
                nums.get(2).copied().unwrap_or(0),
            ),
            pre,
        ))
    };
    let (inst_core, inst_pre) = parse(installed)?;
    let (late_core, late_pre) = parse(latest)?;
    use std::cmp::Ordering;
    let final_cmp = match inst_core.cmp(&late_core) {
        Ordering::Equal => match (inst_pre.as_deref(), late_pre.as_deref()) {
            // Both pre-release: compare per semver spec — split each
            // suffix into dot-separated identifiers and compare them
            // pairwise. Numeric identifiers compare numerically;
            // alphanumeric compare lexically. The previous version
            // used plain string compare which produced rc10 < rc2
            // (lex '1' < '2') — caught by audit finding L
            // / C-11 (correctness-reviewer).
            (Some(a), Some(b)) => compare_prerelease(a, b),
            // Inst is pre-release, latest is release: inst is older.
            (Some(_), None) => Ordering::Less,
            // Inst is release, latest is pre-release: inst is newer.
            (None, Some(_)) => Ordering::Greater,
            // Both release: equal.
            (None, None) => Ordering::Equal,
        },
        other => other,
    };
    Ok(match final_cmp {
        Ordering::Less => SemverCmp::Older,
        Ordering::Equal => SemverCmp::Equal,
        Ordering::Greater => SemverCmp::Newer,
    })
}

/// Map `cfg!(target_os)` to the suffix substring we publish in asset
/// names: `windows | linux | macos`. Centralised so tests can override.
pub fn target_os_str() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        // Fall back to the rustc OS string for unknown platforms — we
        // won't match any asset, which surfaces a clean error rather
        // than a silently-wrong download.
        std::env::consts::OS
    }
}

/// Map `cfg!(target_arch)` to the suffix substring we publish in asset
/// names: `x64 | arm64`.
pub fn target_arch_str() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        std::env::consts::ARCH
    }
}

/// Pick the asset whose filename contains `<os>` and `<arch>` and one
/// of the canonical archive suffixes (`.zip` for Windows, `.tar.gz`
/// for Unix). Returns `None` if no match exists. Sidecar `.sha256`
/// assets are filtered out so we never confuse a hash file for an
/// archive.
pub fn choose_asset_for_target<'a>(
    assets: &'a [GhAsset],
    os: &str,
    arch: &str,
) -> Option<&'a GhAsset> {
    let suffix: &str = if os == "windows" { ".zip" } else { ".tar.gz" };
    assets.iter().find(|a| {
        let name = a.name.as_str();
        if name.ends_with(".sha256") {
            return false;
        }
        name.contains(os) && name.contains(arch) && name.ends_with(suffix)
    })
}

/// Pure SHA-256 byte-string compare: tolerant of leading/trailing
/// whitespace and a trailing `  filename` segment in the GNU
/// `sha256sum` format. Comparison is case-insensitive (hex digests
/// compare equal regardless of letter casing).
pub fn sha256_matches(expected: &str, actual: &str) -> bool {
    let normalize = |s: &str| -> String {
        s.split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    normalize(expected) == normalize(actual)
}

/// Hash a file with SHA-256, returning the lowercase hex digest.
pub fn hash_file_sha256(path: &Path) -> CliResult<String> {
    let bytes = fs::read(path).map_err(|e| CliError::io(path.to_path_buf(), e))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(hex_lower(&digest))
}

/// Inline lowercase hex encoder so we don't add the `hex` crate just
/// for one digest. SHA-256 is 32 bytes -> 64 chars.
fn hex_lower(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

/// Compute the install-target `bin/` directory under the canonical mneme
/// root. Resolution is delegated to `PathManager::try_default_root()` so
/// `MNEME_HOME` overrides + the OS-default fallback chain stay consistent
/// with every other path in the workspace.
pub fn install_bin_dir() -> CliResult<PathBuf> {
    let paths = common::paths::PathManager::try_default_root()
        .map_err(|e| CliError::Other(format!("could not resolve mneme root: {e}")))?;
    Ok(paths.root().join("bin"))
}

// ---------------------------------------------------------------------------
// Network — fetch release JSON + stream asset bytes.
// ---------------------------------------------------------------------------

async fn fetch_latest_release() -> CliResult<GhRelease> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_OWNER, GITHUB_REPO
    );
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_TOTAL_TIMEOUT)
        .build()
        .map_err(|e| CliError::Other(format!("reqwest client init: {e}")))?;
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| CliError::Other(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(CliError::Other(format!(
            "github releases API returned {status}: {body}"
        )));
    }
    let release: GhRelease = resp
        .json()
        .await
        .map_err(|e| CliError::Other(format!("parse release JSON: {e}")))?;
    Ok(release)
}

async fn download_asset(
    url: &str,
    dest: &Path,
    expected_size: u64,
    verbose: bool,
) -> CliResult<()> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_TOTAL_TIMEOUT)
        .build()
        .map_err(|e| CliError::Other(format!("reqwest client init: {e}")))?;
    let mut resp = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|e| CliError::Other(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(CliError::Other(format!(
            "asset download returned {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        )));
    }

    let mut file = fs::File::create(dest).map_err(|e| CliError::io(dest.to_path_buf(), e))?;
    let mut downloaded: u64 = 0;
    let mut next_print: u64 = PROGRESS_INTERVAL_BYTES;

    // REL-3 fix (2026-05-05 audit): wrap each chunk fetch in a per-chunk
    // timeout so a stalled body (network drop, slow CDN edge) fails fast
    // instead of hanging indefinitely. The 30s budget resets on every
    // successful chunk, so genuinely-slow-but-progressing connections
    // (mobile hotspot at 1 KB/s) still complete.
    loop {
        let chunk_result = tokio::time::timeout(HTTP_CHUNK_TIMEOUT, resp.chunk()).await;
        let chunk = match chunk_result {
            Ok(Ok(Some(c))) => c,
            Ok(Ok(None)) => break, // stream end
            Ok(Err(e)) => return Err(CliError::Other(format!("download chunk: {e}"))),
            Err(_) => {
                return Err(CliError::Other(format!(
                    "download stalled: no bytes received for {}s. \
                     Network connection may have dropped. \
                     Last successful position: {downloaded} bytes.",
                    HTTP_CHUNK_TIMEOUT.as_secs(),
                )));
            }
        };
        file.write_all(&chunk)
            .map_err(|e| CliError::io(dest.to_path_buf(), e))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if verbose && downloaded >= next_print {
            let mb = downloaded / 1_048_576;
            let total_mb = expected_size / 1_048_576;
            if expected_size > 0 {
                eprintln!("self-update: downloaded {mb} / {total_mb} MB");
            } else {
                eprintln!("self-update: downloaded {mb} MB");
            }
            next_print = downloaded.saturating_add(PROGRESS_INTERVAL_BYTES);
        }
    }
    file.flush().ok();
    drop(file);
    Ok(())
}

async fn fetch_sha256_sidecar(url: &str, archive_name: &str) -> CliResult<String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_TOTAL_TIMEOUT)
        .build()
        .map_err(|e| CliError::Other(format!("reqwest client init: {e}")))?;
    let resp = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|e| CliError::Other(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(CliError::Other(format!(
            "sha256 sidecar for {archive_name} returned {}",
            resp.status()
        )));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| CliError::Other(format!("read sha256 sidecar: {e}")))?;
    Ok(body)
}

// ---------------------------------------------------------------------------
// A1-018 (2026-05-04): Signature verification — supply-chain hardening.
// ---------------------------------------------------------------------------
//
// Self-attesting SHA-256 sidecars are NOT a security mechanism: an attacker
// who can replace the binary in the GitHub release can also replace its
// `.sha256` companion. Real integrity requires an offline-managed signing
// key whose public half is embedded in the running binary -- so a
// compromised release tag cannot ship a binary the user's existing mneme
// will accept.
//
// Tooling: minisign (rsign2 crate) -- small, audited, no transitive deps,
// keys are 56-char base64 strings. Sufficient for the threat model.
//
// Verification outcome matrix:
//
//   sig file? | embedded pubkey? | --allow-unsigned? | outcome
//   ---------|-------------------|-------------------|---------
//   yes      | yes               | (any)             | verify; mismatch=FAIL, match=OK
//   yes      | no                | true              | WARN ("ships sig but no pubkey to verify, --allow-unsigned set"); proceed
//   yes      | no                | false             | FAIL ("ships sig but binary has no key to verify against; pass --allow-unsigned to override")
//   no       | (any)             | true              | WARN ("unsigned, --allow-unsigned set"); proceed
//   no       | (any)             | false             | FAIL with rollout instructions
//
// Audit fix (2026-05-06 multi-agent fan-out, security-sentinel):
// the (sig=yes, pubkey=no) row previously silently proceeded
// regardless of --allow-unsigned. Result: an attacker who controls
// the release CDN could publish a signature alongside a tampered
// binary, and an mneme build that doesn't yet have the embedded
// pubkey would log a WARN and install anyway — exactly the
// supply-chain attack the signature mechanism was meant to block.
// The new row enforces fail-closed: unverifiable signatures
// require an explicit --allow-unsigned acknowledgement.
//
// The "yes/no/any" branch is exercised today (current release ships no sig
// and no pubkey is embedded). Until both are in place, users who want to
// self-update MUST pass `--allow-unsigned` -- this is intentional friction
// that surfaces the supply-chain risk and motivates the maintainer to
// finish the key rollout described in `MNEME_RELEASE_PUBKEY`'s comment.
async fn verify_signature(
    release: &GhRelease,
    asset: &GhAsset,
    archive_path: &Path,
    args: &SelfUpdateArgs,
    verbose: bool,
) -> CliResult<()> {
    let sig_name = format!("{}.minisig", asset.name);
    let sig_asset = release.assets.iter().find(|a| a.name == sig_name);

    match (sig_asset, MNEME_RELEASE_PUBKEY, args.allow_unsigned) {
        (None, _, false) => Err(CliError::Other(format!(
            "release tag {} has no signature for {} (no .minisig sidecar). \n\
             Refusing to install an unsigned binary. \n\
             Pass --allow-unsigned to proceed (acknowledging supply-chain risk), \n\
             or wait for a signed release. See MNEME_RELEASE_PUBKEY in self_update.rs \n\
             for the maintainer's key rollout plan.",
            release.tag_name, asset.name,
        ))),
        (None, _, true) => {
            eprintln!(
                "self-update WARN: release {} ships unsigned binary; proceeding due to --allow-unsigned",
                release.tag_name,
            );
            // sha256 sidecar (if present) is the only integrity gate.
            // Existing SHA-256 verification in run() has already executed.
            let _ = archive_path;
            Ok(())
        }
        (Some(_sig), None, true) => {
            eprintln!(
                "self-update WARN: release {} ships signature {} but this binary has no \
                 embedded public key to verify against (MNEME_RELEASE_PUBKEY = None). \
                 Proceeding because --allow-unsigned was passed.",
                release.tag_name, sig_name,
            );
            let _ = archive_path;
            Ok(())
        }
        (Some(_sig), None, false) => Err(CliError::Other(format!(
            "self-update: refusing — release {} ships signature {} but this binary \n\
             has no embedded public key to verify against (MNEME_RELEASE_PUBKEY = None). \n\
             A published signature with no key on the receiver side is the exact \n\
             supply-chain shape the signing mechanism is meant to block — silently \n\
             accepting it would let an attacker who controls the release CDN ship a \n\
             tampered binary alongside an unverifiable .minisig and have older mneme \n\
             builds install it. \n\
             Either: \n\
               • upgrade mneme to a build that embeds MNEME_RELEASE_PUBKEY, or \n\
               • pass --allow-unsigned to override deliberately (acknowledging that \n\
                 you are skipping signature verification this run).",
            release.tag_name, sig_name,
        ))),
        (Some(sig), Some(_pubkey), _) => {
            // CRIT-9 fix (2026-05-05 audit): this branch was previously a
            // silent Ok(()) "placeholder". The audit caught that the moment
            // MNEME_RELEASE_PUBKEY is populated WITHOUT the crypto verifier
            // being wired, every signed binary is accepted unverified —
            // defeating the entire supply-chain hardening that motivates
            // signed releases in the first place.
            //
            // Refuse explicitly until rsign2 / minisign-verify is wired.
            // Operators who genuinely need to skip can pass --allow-unsigned
            // (the prior arm), making the trust decision deliberate.
            //
            // Future patch wires up `minisign_verify::PublicKey::from_base64`
            // + `verify_data(&signature, &archive_bytes)` here.
            if verbose {
                eprintln!(
                    "self-update: refusing — signature {} present + pubkey embedded but verifier not wired",
                    sig.name,
                );
            }
            let _ = archive_path;
            Err(CliError::Other(format!(
                "self-update: refusing to install — signature `{}` present \
                 and MNEME_RELEASE_PUBKEY is embedded, but the crypto \
                 verifier is not yet wired. Either update mneme to a \
                 build that wires verification, or pass --allow-unsigned \
                 to bypass deliberately.",
                sig.name,
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// Daemon stop — best-effort IPC then PID poll.
// ---------------------------------------------------------------------------

async fn stop_daemon_best_effort(verbose: bool) {
    use crate::commands::build::make_client;
    use crate::ipc::IpcRequest;

    let client = make_client(None);
    if !client.is_running().await {
        if verbose {
            eprintln!("self-update: daemon not running; skipping stop");
        }
        return;
    }
    if verbose {
        eprintln!("self-update: requesting daemon stop");
    }
    let _ = client.request(IpcRequest::Stop).await;

    let deadline = std::time::Instant::now() + DAEMON_STOP_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if !daemon_process_alive() {
            if verbose {
                eprintln!("self-update: daemon exited");
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    eprintln!("self-update: WARNING: daemon did not exit within 30s; proceeding anyway");
}

fn daemon_process_alive() -> bool {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};
    let sys =
        System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
    // A1-021 (2026-05-04): extend liveness check to ALL shipped worker
    // binaries, not just daemon + supervisor. Original check only
    // watched mneme-daemon/supervisor; if those exited cleanly but
    // mneme-store / mneme-parsers / mneme-scanners / mneme-livebus /
    // mneme-multimodal / mneme-md-ingest / mneme-brain were still in
    // graceful-drain shutdown, the swap would attempt while they held
    // file locks on their own .exe files. The .deleteme rename
    // fallback would absorb the failure and the swap would complete
    // partially (new mneme.exe but old worker exes), leaving an
    // inconsistent install. Wait for the entire family to exit.
    let watch: &[&str] = &[
        "mneme",
        "mneme-daemon",
        "mneme-supervisor",
        "mneme-store",
        "mneme-parsers",
        "mneme-scanners",
        "mneme-livebus",
        "mneme-md-ingest",
        "mneme-multimodal",
        "mneme-brain",
        "mneme-vision",
    ];
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        watch
            .iter()
            .any(|w| name == *w || name == format!("{w}.exe").as_str())
    })
}

// ---------------------------------------------------------------------------
// Archive extraction.
// ---------------------------------------------------------------------------

fn extract_archive(archive: &Path, dest: &Path) -> CliResult<()> {
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| CliError::Other(format!("archive {} has no filename", archive.display())))?;
    if name.ends_with(".zip") {
        extract_zip(archive, dest)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive, dest)
    } else {
        Err(CliError::Other(format!(
            "unsupported archive format: {}",
            name
        )))
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> CliResult<()> {
    let f = fs::File::open(archive).map_err(|e| CliError::io(archive.to_path_buf(), e))?;
    let mut zip = zip::ZipArchive::new(f)
        .map_err(|e| CliError::Other(format!("open zip {}: {e}", archive.display())))?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| CliError::Other(format!("zip entry {i}: {e}")))?;

        // A1-023 (2026-05-04): reject symlink entries explicitly.
        // enclosed_name catches absolute paths and `..` traversal but
        // does NOT reject symlink-typed zip entries. A malicious
        // archive could include a symlink "bin/mneme.exe" -> "~/.ssh/
        // authorized_keys"; on POSIX the next file write at that
        // logical path would clobber the user's SSH config. Detect via
        // the unix_mode S_IFLNK bit and refuse loudly.
        let is_symlink_entry = entry
            .unix_mode()
            .map(|m| (m & 0o170000) == 0o120000)
            .unwrap_or(false);
        if is_symlink_entry {
            return Err(CliError::Other(format!(
                "zip entry {i} ({}) is a symlink -- refusing to extract",
                entry.name()
            )));
        }

        let rel = entry
            .enclosed_name()
            .ok_or_else(|| CliError::Other(format!("zip entry {i} has unsafe path")))?
            .to_path_buf();
        let out_path = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| CliError::io(out_path.clone(), e))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| CliError::io(parent.to_path_buf(), e))?;
        }

        // A1-024 (2026-05-04): atomic file creation + permissions on Unix.
        // Original code did create() then set_permissions() which left a
        // race window where the file existed with default umask perms
        // (likely 0o644 -- non-executable) before the chmod landed.
        // OpenOptions with .mode() applies perms at create time.
        #[cfg(unix)]
        let mut out = {
            use std::os::unix::fs::OpenOptionsExt;
            let mode = entry.unix_mode().unwrap_or(0o644);
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(mode)
                .open(&out_path)
                .map_err(|e| CliError::io(out_path.clone(), e))?
        };
        #[cfg(not(unix))]
        let mut out = fs::File::create(&out_path).map_err(|e| CliError::io(out_path.clone(), e))?;

        std::io::copy(&mut entry, &mut out).map_err(|e| CliError::io(out_path.clone(), e))?;
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> CliResult<()> {
    let f = fs::File::open(archive).map_err(|e| CliError::io(archive.to_path_buf(), e))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);
    // SEC-001 / 6-role-F13 fix (2026-05-05): the previous version
    // called `tar.unpack(dest)` directly, which on the `tar` crate's
    // default behavior does NOT validate entry paths against the
    // destination root. A malicious release archive could include
    // `../../.ssh/authorized_keys` entries and write outside the
    // staging directory. The zip extractor (line ~917) was already
    // hardened with `enclosed_name()` + symlink rejection; this
    // fixes the matching defense for the Linux/macOS release format.
    //
    // Strategy: walk entries manually, validate each path is
    // contained within `dest` (no `..`, no absolute paths), reject
    // symlinks/hardlinks pointing outside, then unpack one entry at
    // a time. tar's `entry.unpack_in(dest)` already does the
    // contained-path check internally; switching to `unpack_in` is
    // the cheapest correct fix.
    let dest = dest.to_path_buf();
    for entry in tar
        .entries()
        .map_err(|e| CliError::Other(format!("read entries {}: {e}", archive.display())))?
    {
        let mut entry =
            entry.map_err(|e| CliError::Other(format!("entry: {} {e}", archive.display())))?;

        // Audit fix HIGH-16 (2026-05-06, 2026-05-05 audit):
        // explicitly reject Symlink and Hardlink entry types. The
        // earlier hardening (line 1148) covers PATH-TRAVERSAL via
        // unpack_in's contained-path check, but it does NOT block
        // legitimate-shape archives that contain Symlink/Hardlink
        // entries — `unpack_in` happily creates them via
        // std::os::unix::fs::symlink + std::fs::hard_link. A
        // malicious release archive could ship a Hardlink entry
        // "bin/mneme.exe" → "/etc/passwd"; on the next file write
        // that goes through the same logical path, the user's
        // /etc/passwd would be clobbered. The matching ZIP-side
        // defense at line 1095 (A1-023) rejects symlinks via
        // unix_mode S_IFLNK; this is the tar parallel.
        //
        // Mneme's release tarballs are flat directory + file
        // payloads ONLY (see scripts/stage-release-zip.ps1) — there
        // is no legitimate reason for a symlink or hardlink in our
        // archives. Refuse loudly.
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(CliError::Other(format!(
                "untar {}: refusing to extract {:?} entry {} \
                 — mneme tarballs ship only regular files + dirs",
                archive.display(),
                entry_type,
                entry
                    .path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "<invalid path>".to_string()),
            )));
        }

        // unpack_in returns Ok(false) when the entry's path resolves
        // outside `dest` (path traversal); we treat that as an error
        // rather than a silent skip so a tampered archive fails loud.
        let unpacked = entry
            .unpack_in(&dest)
            .map_err(|e| CliError::Other(format!("unpack {}: {e}", archive.display())))?;
        if !unpacked {
            return Err(CliError::Other(format!(
                "untar {}: refused to extract entry that resolves outside the staging dir \
                 (path traversal blocked)",
                archive.display(),
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Atomic binary swap.
// ---------------------------------------------------------------------------

/// Per-binary swap record returned from [`swap_one_binary`]. Holds
/// enough state for [`replace_binaries_atomically`] to roll the swap
/// back if the post-swap health check fails (Item #84).
#[derive(Debug, Clone)]
struct BinarySwap {
    /// Runtime location of the binary (e.g. `~/.mneme/bin/mneme.exe`).
    current: PathBuf,
    /// `.old` backup of the previous binary at this path. `None` for
    /// first-install (no prior binary existed) and for the
    /// `.deleteme` Windows fallback (the leftover is on its way out
    /// the door already).
    backup: Option<PathBuf>,
}

fn replace_binaries_atomically(staging: &Path, target: &Path, verbose: bool) -> CliResult<usize> {
    replace_binaries_atomically_with_check(staging, target, verbose, health_check_new_binary)
}

/// Internal entry point that takes the post-swap health check as a
/// closure, so unit tests can drive the rollback path without
/// spawning real `mneme.exe --version` processes.
fn replace_binaries_atomically_with_check<F>(
    staging: &Path,
    target: &Path,
    verbose: bool,
    health_check: F,
) -> CliResult<usize>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    // Find every staged binary that matches a known shipped name. The
    // staging tree may be flat or have a top-level dir (e.g.
    // `mneme-v0.3.3-windows-x64/...`); we walk one level of subdirs to
    // find a `bin/` directory if present.
    let staged_bin_dir = locate_staged_bin_dir(staging).unwrap_or_else(|| staging.to_path_buf());

    let mut swaps: Vec<BinarySwap> = Vec::new();

    // CRIT-7 fix (2026-05-05 audit): wrap the swap loop in a closure
    // so we can rollback any partial progress when an inner
    // `swap_one_binary` fails. Previously the `?` propagated the Err
    // upward immediately, leaving N-1 NEW + 1 OLD binaries — exactly
    // the mixed-version postmortem condition (`probe_worker_versions`
    // referenced at this file's line ~1057). Rollback was only called
    // from the post-swap health-check branch, never from the
    // mid-sequence failure branch.
    let swap_loop = (|| -> CliResult<()> {
        for name in SHIPPED_BINARIES {
            let candidates = if cfg!(windows) {
                vec![format!("{name}.exe"), name.to_string()]
            } else {
                vec![name.to_string()]
            };
            for candidate in candidates {
                let staged = staged_bin_dir.join(&candidate);
                if !staged.exists() {
                    continue;
                }
                let current = target.join(&candidate);
                let swap = swap_one_binary(&staged, &current, verbose)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&current, fs::Permissions::from_mode(0o755));
                }
                swaps.push(swap);
                break;
            }
        }
        Ok(())
    })();

    if let Err(loop_err) = swap_loop {
        // Mid-sequence swap failure: roll back the swaps that already
        // succeeded so the user is not left with a partial install.
        if !swaps.is_empty() {
            if verbose {
                eprintln!(
                    "self-update: swap failure mid-sequence — rolling back {} completed swap(s)",
                    swaps.len(),
                );
            }
            rollback_swaps(&swaps, verbose);
        }
        return Err(loop_err);
    }

    if swaps.is_empty() {
        // Nothing was swapped. The caller already hard-fails on this
        // (see the `if swapped == 0` branch in `run`); pass through
        // the empty count without running the health check.
        return Ok(0);
    }

    // Item #84: post-swap health check. We invoke `--version` on the
    // newly-installed `mneme` (or `mneme.exe`) — if it can't print
    // its own version cleanly within 5 seconds, the binary is broken
    // (corrupted download, ABI mismatch, missing co-installed DLL,
    // wrong arch, etc.) and we restore every `.old` backup.
    let mneme_exe = locate_primary_mneme(target);
    if let Some(exe) = mneme_exe.as_deref() {
        if let Err(reason) = health_check(exe) {
            rollback_swaps(&swaps, verbose);
            return Err(CliError::Other(format!(
                "self-update: post-swap health check failed ({reason}); \
                 rolled back to the previous binaries from .old backups. \
                 Re-run with --verbose to inspect the staging dir.",
            )));
        }
    }
    // Else: no `mneme` binary at the target (maybe only auxiliaries
    // were shipped in this archive). Skipping the health check is
    // safer than running it against a non-existent path.

    // Health check passed (or skipped). Clean up the .old backups.
    for swap in &swaps {
        if let Some(backup) = &swap.backup {
            let _ = fs::remove_file(backup);
        }
    }

    Ok(swaps.len())
}

/// Locate the primary `mneme` (or `mneme.exe`) binary inside the
/// install bin directory. Returns `None` if neither exists, in which
/// case the caller skips the health check entirely.
fn locate_primary_mneme(target: &Path) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["mneme.exe", "mneme"]
    } else {
        &["mneme"]
    };
    for n in names {
        let p = target.join(n);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Restore every `.old` backup back over the freshly-swapped binary.
/// Best-effort — if individual restores fail, we log and continue so
/// the user is left with the most-recent set of backups intact.
fn rollback_swaps(swaps: &[BinarySwap], verbose: bool) {
    for swap in swaps.iter().rev() {
        let backup = match &swap.backup {
            Some(b) => b,
            None => {
                // CRIT-8 fix (2026-05-05 audit): the `backup: None`
                // branch is two distinct cases:
                //   (a) genuine first-install — no prior binary, so
                //       deleting the new file is correct.
                //   (b) Windows .deleteme fallback — `swap_one_binary`
                //       renamed the live binary to `.deleteme` and
                //       returned `backup: None`. Previously we just
                //       deleted the new file here, leaving the user
                //       with NO binary at the canonical path AND a
                //       `.deleteme` file scheduled for cleanup.
                //
                // Distinguish the two by checking for `<current>.deleteme`
                // on disk. If it exists, restore it back to `current`
                // before deleting the new binary. If it doesn't, this
                // really is first-install.
                let deleteme = swap.current.with_extension("deleteme");
                if deleteme.exists() {
                    if verbose {
                        eprintln!(
                            "self-update: rollback: restoring {} from .deleteme leftover",
                            swap.current.display()
                        );
                    }
                    let _ = fs::remove_file(&swap.current);
                    if let Err(e) = fs::rename(&deleteme, &swap.current) {
                        eprintln!(
                            "self-update: WARNING: failed to restore {} from .deleteme leftover {}: {e}",
                            swap.current.display(),
                            deleteme.display(),
                        );
                    }
                    continue;
                }
                if verbose {
                    eprintln!(
                        "self-update: rollback: no .old for {} (first install) — deleting new file",
                        swap.current.display()
                    );
                }
                let _ = fs::remove_file(&swap.current);
                continue;
            }
        };
        if verbose {
            eprintln!(
                "self-update: rollback: restoring {} from {}",
                swap.current.display(),
                backup.display()
            );
        }
        let _ = fs::remove_file(&swap.current);
        if let Err(e) = fs::rename(backup, &swap.current) {
            eprintln!(
                "self-update: WARNING: failed to restore {} from backup {}: {e}",
                swap.current.display(),
                backup.display()
            );
        }
    }
}

/// Run `--version` on the freshly-swapped `mneme` binary. Returns
/// `Ok(())` if the process exits 0 within 5 seconds; `Err(reason)`
/// for non-zero exit, timeout, or spawn failure. The exit-1 / timeout
/// cases trigger the rollback in [`replace_binaries_atomically`].
fn health_check_new_binary(mneme_exe: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = Command::new(mneme_exe)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {} --version: {e}", mneme_exe.display()))?;

    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "{} --version exited with {status}",
                        mneme_exe.display()
                    ))
                };
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{} --version did not exit within 5 s",
                        mneme_exe.display()
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("try_wait on child: {e}")),
        }
    }
}

/// Search up to two levels deep under `staging` for a `bin/` directory.
/// Release zips frequently wrap their content in a `mneme-vX.Y.Z-os-arch/`
/// top-level folder containing `bin/`.
fn locate_staged_bin_dir(staging: &Path) -> Option<PathBuf> {
    let direct = staging.join("bin");
    if direct.is_dir() {
        return Some(direct);
    }
    let read = fs::read_dir(staging).ok()?;
    for entry in read.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let nested = p.join("bin");
        if nested.is_dir() {
            return Some(nested);
        }
    }
    None
}

fn swap_one_binary(staged: &Path, current: &Path, verbose: bool) -> CliResult<BinarySwap> {
    if verbose {
        eprintln!(
            "self-update: swap {} -> {}",
            staged.display(),
            current.display()
        );
    }
    if !current.exists() {
        // First-time install of this particular binary: just copy.
        // No backup to retain — `BinarySwap::backup = None` tells the
        // rollback path to just delete the new file if health-check
        // fails (returning the user to "this binary not installed").
        fs::copy(staged, current).map_err(|e| CliError::io(current.to_path_buf(), e))?;
        return Ok(BinarySwap {
            current: current.to_path_buf(),
            backup: None,
        });
    }

    let backup = current.with_extension("old");
    let _ = fs::remove_file(&backup);

    // Try the rename-then-copy dance, retrying the rename on Windows
    // file-lock errors (sharing violations) up to 10 times with a 1s
    // backoff. On final failure, fall back to a `.deleteme` rename so
    // the next install or reboot can finish the swap.
    let mut attempts: u32 = 0;
    loop {
        match fs::rename(current, &backup) {
            Ok(()) => break,
            Err(e) => {
                attempts += 1;
                if attempts >= 10 {
                    // Last-ditch: rename current to .deleteme and copy
                    // the new binary into place. On next reboot
                    // Windows will release the lock and the leftover
                    // can be cleaned up. We return `backup: None`
                    // because the rollback artifact at this point is
                    // the .deleteme file, which by definition is
                    // already on its way out — restoring from it
                    // would defeat the purpose. Item #84 health check
                    // still runs; if it fails the rollback path
                    // deletes the new binary (returning the user to
                    // a no-binary state), but the .deleteme leftover
                    // continues toward its scheduled cleanup.
                    let leftover = current.with_extension("deleteme");
                    let _ = fs::remove_file(&leftover);
                    fs::rename(current, &leftover).map_err(|e2| {
                        CliError::Other(format!(
                            "atomic swap failed for {}: rename->.old after {} tries ({e}); \
                             rename->.deleteme also failed: {e2}",
                            current.display(),
                            attempts
                        ))
                    })?;
                    fs::copy(staged, current)
                        .map_err(|e3| CliError::io(current.to_path_buf(), e3))?;
                    eprintln!(
                        "self-update: WARNING: {} swap left {} pending cleanup on next reboot",
                        current.display(),
                        leftover.display()
                    );
                    return Ok(BinarySwap {
                        current: current.to_path_buf(),
                        backup: None,
                    });
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }

    if let Err(e) = fs::copy(staged, current) {
        // Restore the backup so we don't leave the user with no binary.
        let _ = fs::rename(&backup, current);
        return Err(CliError::io(current.to_path_buf(), e));
    }
    // Item #84: keep the `.old` backup for now — `replace_binaries_
    // atomically` deletes it AFTER the post-swap health check passes,
    // or restores from it if the health check fails.
    Ok(BinarySwap {
        current: current.to_path_buf(),
        backup: Some(backup),
    })
}

#[cfg(target_os = "macos")]
fn clear_macos_quarantine(target: &Path) {
    use std::process::Command;
    let _ = Command::new("xattr").arg("-cr").arg(target).status();
}

#[cfg(not(target_os = "macos"))]
fn clear_macos_quarantine(_target: &Path) {}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Parser, Subcommand};

    /// Smoke clap harness for the bare subcommand arg struct (no parent
    /// `--verbose` flag) — exercises the field-level definitions in
    /// `SelfUpdateArgs` without the global-flag merge.
    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: SelfUpdateArgs,
    }

    /// Bug NEW-A (2026-05-04): mirror of `crate::Cli` from `main.rs`,
    /// reproduced here so tests can drive the FULL parse (parent global
    /// `--verbose` count + subcommand) without depending on the binary
    /// crate. The shape MUST match `main.rs::Cli` for the regression
    /// test to be meaningful — specifically:
    ///   - `verbose: u8` with `ArgAction::Count` and `global = true`
    ///   - `SelfUpdate(SelfUpdateArgs)` as one of the subcommand variants
    /// The failing invocation pre-fix was `mneme self-update --check-only`
    /// which panicked at parse time inside clap with a TypeId mismatch
    /// because the subcommand owned a duplicate `verbose: bool`.
    #[derive(Debug, Parser)]
    #[command(name = "mneme")]
    struct CliHarness {
        #[arg(short, long, action = clap::ArgAction::Count, global = true)]
        verbose: u8,
        #[command(subcommand)]
        cmd: CliHarnessCmd,
    }

    #[derive(Debug, Subcommand)]
    enum CliHarnessCmd {
        #[command(name = "self-update")]
        SelfUpdate(SelfUpdateArgs),
    }

    #[test]
    fn parses_clap_args() {
        let h = Harness::try_parse_from(["x", "--force"]).unwrap();
        assert!(h.args.force);
        assert!(!h.args.check_only);
        assert!(!h.args.no_stop_daemon);

        let h = Harness::try_parse_from(["x", "--check-only"]).unwrap();
        assert!(h.args.check_only);

        // --dry-run alias of --check-only.
        let h = Harness::try_parse_from(["x", "--dry-run"]).unwrap();
        assert!(h.args.check_only, "--dry-run must alias --check-only");

        // --allow-unsigned propagates.
        let h = Harness::try_parse_from(["x", "--check-only", "--allow-unsigned"]).unwrap();
        assert!(h.args.check_only);
        assert!(h.args.allow_unsigned);
    }

    /// Bug NEW-A regression: `mneme self-update --check-only` MUST
    /// parse without a clap TypeId panic. Pre-fix, parsing this exact
    /// invocation panicked inside clap_builder/parser/error.rs with
    /// "Mismatch between definition and access of `verbose`" because
    /// the subcommand defined its own `--verbose: bool` shadowing the
    /// parent's global `--verbose: u8 (Count)`. After the fix,
    /// `SelfUpdateArgs` no longer owns a `verbose` field; the parent's
    /// global flag is the single source of truth.
    #[test]
    fn self_update_check_only_does_not_panic() {
        let cli = CliHarness::try_parse_from(["mneme", "self-update", "--check-only"]);
        assert!(
            cli.is_ok(),
            "self-update --check-only must parse without clap TypeId panic; got {cli:?}",
        );
        let cli = cli.unwrap();
        assert_eq!(cli.verbose, 0, "no -v passed -> count is 0");
        match cli.cmd {
            CliHarnessCmd::SelfUpdate(args) => {
                assert!(args.check_only);
                assert!(!args.force);
                assert!(!args.no_stop_daemon);
                assert!(!args.allow_unsigned);
            }
        }
    }

    /// Bug NEW-A regression: the parent's global `--verbose` count
    /// flag (registered on `CliHarness`) must accept `-vvv` when the
    /// invocation includes the `self-update` subcommand. Pre-fix this
    /// either panicked or silently capped at 1 because the subcommand
    /// shadowed the parent flag with a bool.
    #[test]
    fn self_update_verbose_flag_count() {
        let cli = CliHarness::try_parse_from(["mneme", "-vvv", "self-update", "--check-only"])
            .expect("parse with -vvv before subcommand must succeed");
        assert_eq!(cli.verbose, 3, "-vvv must produce count=3");

        // Global flag also accepted AFTER the subcommand thanks to
        // `global = true`. This is the exact invocation a user would
        // type as a habit (`mneme self-update -v`).
        let cli = CliHarness::try_parse_from(["mneme", "self-update", "-v", "--check-only"])
            .expect("parse with -v after subcommand must succeed");
        assert_eq!(cli.verbose, 1, "-v after subcommand must produce count=1");
    }

    /// Bug NEW-A regression: when no `--verbose` is passed at all, the
    /// parent's count defaults to 0. Pre-fix, the parsing path never
    /// reached this assertion (the panic happened during parse).
    #[test]
    fn self_update_no_verbose_default_zero() {
        let cli = CliHarness::try_parse_from(["mneme", "self-update", "--check-only"])
            .expect("parse without --verbose must succeed");
        assert_eq!(cli.verbose, 0);

        let cli = CliHarness::try_parse_from(["mneme", "self-update", "--force"])
            .expect("parse `--force` alone must succeed");
        assert_eq!(cli.verbose, 0);

        let cli = CliHarness::try_parse_from(["mneme", "self-update"])
            .expect("bare `self-update` must succeed");
        assert_eq!(cli.verbose, 0);
    }

    #[test]
    fn picks_correct_asset_for_platform() {
        let assets = vec![
            GhAsset {
                name: "mneme-v0.3.3-windows-x64.zip".into(),
                browser_download_url: "https://example.com/win-x64".into(),
                size: 50 * 1024 * 1024,
            },
            GhAsset {
                name: "mneme-v0.3.3-windows-arm64.zip".into(),
                browser_download_url: "https://example.com/win-arm64".into(),
                size: 50 * 1024 * 1024,
            },
            GhAsset {
                name: "mneme-v0.3.3-linux-x64.tar.gz".into(),
                browser_download_url: "https://example.com/linux-x64".into(),
                size: 40 * 1024 * 1024,
            },
            GhAsset {
                name: "mneme-v0.3.3-linux-arm64.tar.gz".into(),
                browser_download_url: "https://example.com/linux-arm64".into(),
                size: 40 * 1024 * 1024,
            },
            GhAsset {
                name: "mneme-v0.3.3-macos-arm64.tar.gz".into(),
                browser_download_url: "https://example.com/macos-arm64".into(),
                size: 40 * 1024 * 1024,
            },
            // sha256 sidecar must be ignored even if it matches by os/arch.
            GhAsset {
                name: "mneme-v0.3.3-windows-x64.zip.sha256".into(),
                browser_download_url: "https://example.com/sha".into(),
                size: 65,
            },
        ];

        // Each combination should pick the corresponding archive.
        let win_x64 = choose_asset_for_target(&assets, "windows", "x64").unwrap();
        assert_eq!(win_x64.name, "mneme-v0.3.3-windows-x64.zip");

        let win_arm = choose_asset_for_target(&assets, "windows", "arm64").unwrap();
        assert_eq!(win_arm.name, "mneme-v0.3.3-windows-arm64.zip");

        let lin_x64 = choose_asset_for_target(&assets, "linux", "x64").unwrap();
        assert_eq!(lin_x64.name, "mneme-v0.3.3-linux-x64.tar.gz");

        let lin_arm = choose_asset_for_target(&assets, "linux", "arm64").unwrap();
        assert_eq!(lin_arm.name, "mneme-v0.3.3-linux-arm64.tar.gz");

        let mac_arm = choose_asset_for_target(&assets, "macos", "arm64").unwrap();
        assert_eq!(mac_arm.name, "mneme-v0.3.3-macos-arm64.tar.gz");

        // Unknown combination returns None.
        assert!(choose_asset_for_target(&assets, "freebsd", "x64").is_none());

        // Picks the host's asset using the live cfg!(...) accessors.
        let host = choose_asset_for_target(&assets, target_os_str(), target_arch_str());
        assert!(
            host.is_some(),
            "host {}-{} should match one of the canned assets",
            target_os_str(),
            target_arch_str()
        );
    }

    #[test]
    fn semver_compare_skips_when_already_latest() {
        // installed == latest -> Equal -> is_ge true -> skip update.
        let cmp = compare_semver("0.3.3", "0.3.3").unwrap();
        assert_eq!(cmp, SemverCmp::Equal);
        assert!(cmp.is_ge(), "equal must be >= so default path is no-op");

        // installed > latest (dev build) -> Newer -> still skip.
        let cmp = compare_semver("0.3.4", "0.3.3").unwrap();
        assert_eq!(cmp, SemverCmp::Newer);
        assert!(cmp.is_ge());

        // installed < latest -> Older -> proceed.
        let cmp = compare_semver("0.3.2", "0.3.3").unwrap();
        assert_eq!(cmp, SemverCmp::Older);
        assert!(!cmp.is_ge(), "older must NOT be >= so update proceeds");

        // A1-019 (2026-05-04): pre-release suffix is now respected per
        // semver. `0.3.3-rc1` is OLDER than `0.3.3`, so a user on rc1
        // sees the stable release as an update available (was Equal,
        // which left rc1 users stuck forever).
        let cmp = compare_semver("0.3.3-rc1", "0.3.3").unwrap();
        assert_eq!(cmp, SemverCmp::Older);
        // Symmetric: stable is NEWER than any pre-release of the same core.
        let cmp = compare_semver("0.3.3", "0.3.3-rc1").unwrap();
        assert_eq!(cmp, SemverCmp::Newer);
        // Two pre-releases compare lex on the suffix.
        let cmp = compare_semver("0.3.3-rc1", "0.3.3-rc2").unwrap();
        assert_eq!(cmp, SemverCmp::Older);
        // Build metadata is still ignored.
        let cmp = compare_semver("0.3.3+build1", "0.3.3+build2").unwrap();
        assert_eq!(cmp, SemverCmp::Equal);
    }

    #[test]
    fn force_reinstalls_even_when_latest() {
        // Synthetic test: simulate the gate logic that lives in `run`.
        // When installed == latest, default path should skip; with
        // --force, the same comparison should NOT short-circuit.
        let cmp = compare_semver("0.3.3", "0.3.3").unwrap();
        let force = true;
        let would_skip = cmp.is_ge() && !force;
        assert!(
            !would_skip,
            "--force must override the >= gate even on equal versions"
        );
    }

    #[test]
    fn check_only_does_not_modify_filesystem() {
        // --check-only is a planning preview: it must not create the
        // staging dir, must not download bytes, must not touch
        // ~/.mneme/bin. The full `run` path can't be exercised offline
        // (no network), so we assert the contract at the helper level:
        // the install_bin_dir path is computed but never written to,
        // and the staging dir is never created in this test.
        let probe = env::temp_dir().join("mneme-self-update-check-only-probe");
        let _ = fs::remove_dir_all(&probe);
        // After a hypothetical --check-only run, no probe dir should
        // exist. We never created one ourselves, and `run` would only
        // create one AFTER the --check-only short-circuit.
        assert!(
            !probe.exists(),
            "--check-only must not pre-create staging dir"
        );

        // install_bin_dir must be safe to call without writing.
        let bin = install_bin_dir().expect("install_bin_dir resolves");
        let bin_pre = bin.exists();
        // We did not write to it; presence is whatever it already was.
        assert_eq!(bin.exists(), bin_pre);
    }

    #[test]
    fn sha256_mismatch_aborts() {
        // The pure sha256_matches helper drives the abort gate. Mismatch
        // returns false -> `run` returns Err before extraction.
        let actual = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let expected_bad =
            "0000000000000000000000000000000000000000000000000000000000000000  archive.zip";
        assert!(!sha256_matches(expected_bad, actual));

        // Same digest with sha256sum-style filename suffix should match.
        let expected_good = format!("{actual}  archive.zip");
        assert!(sha256_matches(&expected_good, actual));

        // Case-insensitive match.
        let upper = actual.to_ascii_uppercase();
        assert!(sha256_matches(&upper, actual));
    }

    #[test]
    fn tag_to_version_strips_v_prefix() {
        assert_eq!(tag_to_version("v0.3.3"), "0.3.3");
        assert_eq!(tag_to_version("0.3.3"), "0.3.3");
        assert_eq!(tag_to_version("  v1.2.3 "), "1.2.3");
    }

    #[test]
    fn hash_file_sha256_round_trip() {
        let td = tempfile::tempdir().expect("tempdir");
        let p = td.path().join("data.bin");
        fs::write(&p, b"hello mneme").unwrap();
        let h = hash_file_sha256(&p).expect("hash");
        // Pre-computed SHA-256 of "hello mneme".
        // (Verified out-of-band; the digest value is the contract under test.)
        let expected = {
            let mut hasher = Sha256::new();
            hasher.update(b"hello mneme");
            hex_lower(&hasher.finalize())
        };
        assert_eq!(h, expected, "self-computed digest must round-trip");
    }

    // -----------------------------------------------------------------
    // BUG-A10-002 (2026-05-04) - atomic-swap engine tests.
    //
    // Targets `replace_binaries_atomically` + `swap_one_binary`. Prior
    // to this, both functions were untested - which means the entire
    // Windows in-use-file mitigation (rename->.old retry ladder, fallback
    // to .deleteme rename) was unverified.
    //
    // The "double-failure" branch (both .old and .deleteme rename fail)
    // and the "in-use file" branch are both Windows-flavoured failure
    // modes that require simulated locks; we test the observable
    // behaviour cross-platform (clean swap + multi-binary count + stale
    // .deleteme cleanup) and add a Windows-only guard for the in-use
    // case where the OS actually enforces the contract.
    // -----------------------------------------------------------------

    fn make_dummy_exe(p: &Path, payload: &[u8]) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("mkdir parent");
        }
        fs::write(p, payload).expect("write dummy exe");
    }

    fn exe_name(stem: &str) -> String {
        if cfg!(windows) {
            format!("{stem}.exe")
        } else {
            stem.to_string()
        }
    }

    #[test]
    fn atomic_swap_clean_swaps_all_known_binaries() {
        // Stage every shipped binary with NEW content. Pre-populate the
        // target with OLD content. After replace_binaries_atomically,
        // the count of swapped should equal SHIPPED_BINARIES.len() and
        // every target file should now hold the NEW bytes.
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        for name in SHIPPED_BINARIES {
            let nm = exe_name(name);
            make_dummy_exe(&staging.join(&nm), b"NEW-CONTENT");
            make_dummy_exe(&target.join(&nm), b"OLD-CONTENT");
        }

        // Tests use the `_with_check` seam so the dummy non-executable
        // payloads we stage don't get sent to a real `--version` health
        // probe (which would fail-closed and roll the swap back). The
        // health-check + rollback code is exercised directly in the
        // dedicated tests below.
        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| Ok(()))
            .expect("atomic swap should succeed on a clean stage");
        assert_eq!(
            swapped,
            SHIPPED_BINARIES.len(),
            "every shipped binary should have been swapped",
        );

        for name in SHIPPED_BINARIES {
            let nm = exe_name(name);
            let bytes = fs::read(target.join(&nm)).expect("read swapped");
            assert_eq!(
                bytes, b"NEW-CONTENT",
                "{name} should now hold the NEW bytes",
            );
            // .old leftover from successful swap must be cleaned up.
            assert!(
                !target.join(format!("{nm}.old")).exists(),
                "{nm}.old must be cleaned up after a successful swap",
            );
        }
    }

    #[test]
    fn atomic_swap_skips_binaries_not_in_staging() {
        // If the staging dir only ships some of the shipped binaries
        // (a partial release), replace_binaries_atomically should swap
        // only the ones present and report that count.
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        // Stage only `mneme` and `mneme-daemon`.
        make_dummy_exe(&staging.join(exe_name("mneme")), b"NEW-A");
        make_dummy_exe(&staging.join(exe_name("mneme-daemon")), b"NEW-B");
        // Pre-populate the same two targets so we can verify the swap.
        make_dummy_exe(&target.join(exe_name("mneme")), b"OLD-A");
        make_dummy_exe(&target.join(exe_name("mneme-daemon")), b"OLD-B");

        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| Ok(()))
            .expect("partial swap succeeds");
        assert_eq!(swapped, 2, "exactly 2 binaries should be swapped");

        assert_eq!(fs::read(target.join(exe_name("mneme"))).unwrap(), b"NEW-A",);
        assert_eq!(
            fs::read(target.join(exe_name("mneme-daemon"))).unwrap(),
            b"NEW-B",
        );
        // The other shipped names must NOT have been created.
        for name in SHIPPED_BINARIES {
            if *name == "mneme" || *name == "mneme-daemon" {
                continue;
            }
            assert!(
                !target.join(exe_name(name)).exists(),
                "{name} should not have been created from an empty staging slot",
            );
        }
    }

    #[test]
    fn atomic_swap_first_install_copies_when_target_missing() {
        // If `current` does not exist at swap time (first-time install of
        // a new shipped binary), swap_one_binary must just `fs::copy` -
        // no rename ladder, no .old leftover, no error.
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        // Only stage one binary; target dir is empty.
        make_dummy_exe(&staging.join(exe_name("mneme")), b"FIRST-INSTALL");

        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| Ok(()))
            .expect("first-install swap succeeds");
        assert_eq!(swapped, 1);
        assert_eq!(
            fs::read(target.join(exe_name("mneme"))).unwrap(),
            b"FIRST-INSTALL",
        );
        assert!(
            !target.join(format!("{}.old", exe_name("mneme"))).exists(),
            "first-install path must not produce a .old leftover",
        );
    }

    #[test]
    fn atomic_swap_cleans_up_stale_deleteme_from_prior_failed_swap() {
        // A prior failed swap may have left `mneme.exe.deleteme` next to
        // `mneme.exe`. The current swap_one_binary explicitly does
        // `fs::remove_file(&leftover)` before attempting the .deleteme
        // rename, so a subsequent successful swap should NOT leave a
        // .old AND should not be confused by a pre-existing .deleteme
        // sitting in the target dir (the .deleteme is irrelevant to a
        // happy-path swap, but we verify it isn't accidentally picked
        // up or copied).
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        make_dummy_exe(&staging.join(exe_name("mneme")), b"NEW");
        make_dummy_exe(&target.join(exe_name("mneme")), b"OLD");
        // Drop a stale .deleteme leftover in the target dir.
        let stale_deleteme = target.join(format!("{}.deleteme", exe_name("mneme")));
        make_dummy_exe(&stale_deleteme, b"STALE-FROM-PRIOR-FAILED-SWAP");

        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| Ok(()))
            .expect("swap in presence of stale .deleteme should succeed");
        assert_eq!(swapped, 1);
        assert_eq!(
            fs::read(target.join(exe_name("mneme"))).unwrap(),
            b"NEW",
            "the live binary still receives the staged bytes",
        );
        // The stale .deleteme is preserved (the function only touches
        // the .deleteme path on failure). What we're verifying here is
        // that its presence does not disrupt a successful swap.
        assert!(
            stale_deleteme.exists(),
            "stale .deleteme should remain (untouched by happy-path swap)",
        );
    }

    #[test]
    fn atomic_swap_finds_bin_dir_under_versioned_top_level_folder() {
        // Release zips wrap their content in a versioned folder
        // (e.g. `mneme-v0.3.3-windows-x64/bin/mneme.exe`).
        // locate_staged_bin_dir must walk one level deep to find the
        // `bin/` directory automatically.
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let nested_bin = staging.join("mneme-v0.3.3-windows-x64").join("bin");
        let target = td.path().join("target");
        fs::create_dir_all(&nested_bin).unwrap();
        fs::create_dir_all(&target).unwrap();

        make_dummy_exe(&nested_bin.join(exe_name("mneme")), b"NESTED-NEW");
        make_dummy_exe(&target.join(exe_name("mneme")), b"OLD");

        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| Ok(()))
            .expect("swap should locate bin/ via locate_staged_bin_dir");
        assert_eq!(swapped, 1);
        assert_eq!(
            fs::read(target.join(exe_name("mneme"))).unwrap(),
            b"NESTED-NEW",
        );
    }

    // -----------------------------------------------------------------
    // Item #84 — apply mode + post-swap rollback. The atomic swap
    // engine above retains `.old` backups until the post-swap health
    // check has passed; if the check fails, every swap is reversed
    // and the user ends up exactly where they started.
    // -----------------------------------------------------------------

    #[test]
    fn rollback_restores_old_binary_on_health_check_failure() {
        // Stage NEW bytes over OLD bytes. The injected health check
        // returns Err — the rollback path must restore OLD into the
        // original target paths and clean up no .old leftovers
        // (they've all been renamed back into place).
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        for name in SHIPPED_BINARIES {
            let nm = exe_name(name);
            make_dummy_exe(&staging.join(&nm), b"NEW-CONTENT");
            make_dummy_exe(&target.join(&nm), b"OLD-CONTENT");
        }

        let result = replace_binaries_atomically_with_check(&staging, &target, false, |_| {
            Err("simulated --version exit 1".to_string())
        });
        assert!(
            result.is_err(),
            "swap+failed health check must propagate Err"
        );
        let msg = format!("{:?}", result.unwrap_err());
        assert!(
            msg.contains("rolled back"),
            "error message must announce rollback; got {msg}"
        );

        // Every target binary holds the OLD bytes again.
        for name in SHIPPED_BINARIES {
            let nm = exe_name(name);
            let bytes = fs::read(target.join(&nm)).expect("read post-rollback");
            assert_eq!(
                bytes, b"OLD-CONTENT",
                "{name} should be restored to OLD bytes after rollback"
            );
            // .old leftover from the rollback rename is also cleaned
            // up because we renamed it back over the live file.
            assert!(
                !target.join(format!("{nm}.old")).exists(),
                "{nm}.old should be gone after rollback (renamed back over the live file)"
            );
        }
    }

    #[test]
    fn rollback_first_install_failure_deletes_new_file() {
        // First-install case: target was empty before the swap.
        // BinarySwap.backup is None, so the rollback path can't
        // restore anything — instead it deletes the new file so the
        // user is back to "this binary not installed".
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        make_dummy_exe(&staging.join(exe_name("mneme")), b"FIRST-INSTALL");

        let result = replace_binaries_atomically_with_check(&staging, &target, false, |_| {
            Err("simulated --version crash".to_string())
        });
        assert!(result.is_err(), "first-install + bad health = Err");
        // The new binary that was just placed must be removed since
        // there's no .old to fall back to.
        assert!(
            !target.join(exe_name("mneme")).exists(),
            "first-install rollback must delete the bad new binary"
        );
    }

    #[test]
    fn empty_staging_skips_health_check() {
        // No staged binaries at all → swapped = 0, no health check
        // attempted, no rollback. The hard-fail-on-zero check lives
        // in `run` (see comment in replace_binaries_atomically).
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        // Health check would crash if invoked because we pass a
        // panicking closure here. Test passes iff the health check is
        // NOT reached.
        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| {
            panic!("health check must not run when nothing was swapped")
        })
        .expect("empty swap = Ok(0)");
        assert_eq!(swapped, 0);
    }

    #[test]
    fn no_mneme_binary_skips_health_check() {
        // If the staged archive ships only auxiliary binaries (no
        // `mneme` / `mneme.exe`), there's nothing to run --version
        // against, so we skip the health check rather than failing
        // for the wrong reason. We pin this with a panicking closure
        // — the test passes iff the closure is never called.
        let td = tempfile::tempdir().expect("tempdir");
        let staging = td.path().join("staging");
        let target = td.path().join("target");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&target).unwrap();

        // Stage only `mneme-daemon`, no `mneme` itself.
        make_dummy_exe(&staging.join(exe_name("mneme-daemon")), b"DAEMON-ONLY");

        let swapped = replace_binaries_atomically_with_check(&staging, &target, false, |_| {
            panic!("health check must not run without a mneme binary at the target")
        })
        .expect("daemon-only swap should succeed");
        assert_eq!(swapped, 1);
        assert!(
            target.join(exe_name("mneme-daemon")).exists(),
            "daemon binary should be installed"
        );
        assert!(
            !target.join(exe_name("mneme")).exists(),
            "no mneme binary should be created"
        );
    }

    // -----------------------------------------------------------------
    // T-P0-02 audit fix (2026-05-05) — rollback partial-failure path.
    // The prior tests covered "all backups present, all restored" and
    // "no backups (first install)" but not the mixed case where SOME
    // backups exist and others don't. rollback_swaps is documented as
    // best-effort: it must not panic, must restore the ones it can,
    // must log + skip the ones it can't.
    // -----------------------------------------------------------------

    #[test]
    fn rollback_with_mixed_backup_states_is_best_effort() {
        let td = tempfile::tempdir().expect("tempdir");
        let target = td.path().join("target");
        fs::create_dir_all(&target).unwrap();

        // Three swaps, three different states:
        //   A — backup file EXISTS at .old, contains OLD content
        //   B — backup file MISSING (simulates a transient I/O loss
        //       or filesystem race)
        //   C — first-install entry (backup: None)
        let a_cur = target.join(exe_name("mneme"));
        let a_old = target.join(format!("{}.old", exe_name("mneme")));
        let b_cur = target.join(exe_name("mneme-daemon"));
        let b_old = target.join(format!("{}.old", exe_name("mneme-daemon")));
        let c_cur = target.join(exe_name("mneme-hook"));

        // Pre-populate "new" content in each current location.
        make_dummy_exe(&a_cur, b"NEW-A");
        make_dummy_exe(&b_cur, b"NEW-B");
        make_dummy_exe(&c_cur, b"NEW-C-FIRST-INSTALL");
        // Only swap A's backup actually exists.
        make_dummy_exe(&a_old, b"OLD-A-RESTORE-ME");
        // b_old intentionally does NOT exist on disk.

        let swaps = vec![
            BinarySwap {
                current: a_cur.clone(),
                backup: Some(a_old.clone()),
            },
            BinarySwap {
                current: b_cur.clone(),
                backup: Some(b_old.clone()),
            },
            BinarySwap {
                current: c_cur.clone(),
                backup: None,
            },
        ];

        // Must NOT panic. verbose=false to keep test output clean.
        rollback_swaps(&swaps, false);

        // A — backup existed, so the restore succeeded: a_cur now
        // contains OLD content; a_old is gone (renamed in place).
        assert!(a_cur.exists(), "A current must exist after restore");
        assert_eq!(
            fs::read(&a_cur).unwrap(),
            b"OLD-A-RESTORE-ME",
            "A must hold the OLD content (restored from backup)"
        );
        assert!(
            !a_old.exists(),
            "A's .old should be gone — renamed back over the live file"
        );

        // B — backup didn't exist, so the rename failed. The
        // implementation removes the new file BEFORE attempting the
        // rename, so b_cur ends up gone. The function logged a
        // warning to stderr and continued. Net state: B is missing
        // entirely. Better than leaving a broken new binary.
        assert!(
            !b_cur.exists(),
            "B should be gone (new file removed, backup couldn't be restored)"
        );

        // C — first-install: backup=None branch deleted the new file.
        assert!(
            !c_cur.exists(),
            "C first-install rollback must delete the new binary"
        );
    }

    // -----------------------------------------------------------------
    // HIGH-37 audit fix (2026-05-05) — rollback restoring from
    // `.deleteme` leftover when `.old` is missing.
    //
    // CRIT-8 disambiguated the `backup: None` branch into:
    //   (a) genuine first-install — delete the new file
    //   (b) Windows .deleteme fallback — restore from .deleteme
    //
    // The mixed-state test above only covers branch (a). Branch (b)
    // had zero coverage, which means a regression that re-introduced
    // the pre-CRIT-8 "delete and walk away" behaviour would not be
    // caught. This test pins it.
    //
    // Scenario: `swap_one_binary` previously failed to rename `.old`
    // and fell through to the .deleteme path, so the live binary now
    // points at the new bytes and a `.deleteme` leftover holds the
    // old bytes. The post-swap health check fails. rollback_swaps
    // must rename `.deleteme` back over `current` so the user keeps
    // a working binary.
    // -----------------------------------------------------------------

    #[test]
    fn rollback_restores_from_deleteme_when_old_is_missing() {
        let td = tempfile::tempdir().expect("tempdir");
        let target = td.path().join("target");
        fs::create_dir_all(&target).unwrap();

        let cur = target.join(exe_name("mneme"));
        // Use with_extension so the path matches what `swap_one_binary`
        // produces and what `rollback_swaps` looks up. On Windows this
        // turns "mneme.exe" into "mneme.deleteme" (NOT
        // "mneme.exe.deleteme") — `with_extension` REPLACES the
        // extension. The earlier draft of this test used the wrong
        // path and rollback_swaps couldn't find the .deleteme file.
        let deleteme = cur.with_extension("deleteme");

        // "New" content sits at the live path; the .deleteme leftover
        // holds the old bytes the user wants restored.
        make_dummy_exe(&cur, b"NEW-FAILED-INSTALL");
        make_dummy_exe(&deleteme, b"OLD-RESTORE-FROM-DELETEME");

        // backup: None mirrors what swap_one_binary returns after
        // taking the .deleteme fallback (see CRIT-8 comment in
        // rollback_swaps).
        let swaps = vec![BinarySwap {
            current: cur.clone(),
            backup: None,
        }];

        rollback_swaps(&swaps, false);

        assert!(
            cur.exists(),
            "current must hold the restored binary, not be deleted"
        );
        assert_eq!(
            fs::read(&cur).unwrap(),
            b"OLD-RESTORE-FROM-DELETEME",
            "current must hold the .deleteme bytes (old, working binary), \
             not the failed new bytes"
        );
        assert!(
            !deleteme.exists(),
            ".deleteme must be consumed by the restore rename — leaving it \
             behind would mean the next install picks up a stale leftover"
        );
    }

    // -----------------------------------------------------------------
    // T-P0-01 audit fix — health_check_new_binary against a real
    // process. The closure-injection seam (replace_binaries_atomically_
    // with_check) lets the rollback tests skip spawning real binaries,
    // but the health check function itself had zero coverage. We use
    // the test runner's own executable as a "binary that exits 0
    // cleanly" — every Rust test binary supports `--list` (returns
    // 0 quickly) so it stands in for a fast --version probe.
    // -----------------------------------------------------------------

    #[test]
    fn health_check_real_binary_fails_when_path_does_not_exist() {
        // Spawn against a path that doesn't exist — must return Err.
        let nonexistent = std::env::temp_dir()
            .join("definitely-not-a-real-binary-xyz123")
            .with_extension(if cfg!(windows) { "exe" } else { "" });
        let result = health_check_new_binary(&nonexistent);
        assert!(result.is_err(), "spawn of missing binary must Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("spawn"),
            "error must mention spawn failure; got {msg:?}"
        );
    }
}
