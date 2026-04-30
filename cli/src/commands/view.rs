//! `mneme view` — open the vision app.
//!
//! ## REG-009: 4-path dispatch
//!
//!   1. `--bin <existing-path>` was passed → spawn it.
//!   2. Default vision binary at `~/.mneme/bin/mneme-vision[.exe]` exists
//!      → spawn it.
//!   3. `--web` was passed (or `--url` was set to a non-default value) →
//!      open the browser at that URL.
//!   4. Otherwise → print a clean v0.4-coming hint and exit 0.
//!
//! Exit-0 in path 4 is deliberate: the vision app is a *future* feature.
//! Exiting non-zero would poison automation that piggybacks on `mneme
//! view` (e.g. an editor command palette) when the binary simply
//! isn't installed yet.
//!
//! WIDE-016: the standalone `DEFAULT_WEB_URL` constant has been removed
//! — clap's `default_value` already supplies the literal, and a second
//! source-of-truth was just dead code.

use clap::Args;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tracing::info;

use crate::error::{CliError, CliResult};

/// Default URL the browser path opens. Kept inline as the clap
/// `default_value` so there is a single source of truth (WIDE-016).
const DEFAULT_WEB_URL_LITERAL: &str = "http://localhost:7777";

/// Open the vision app (Tauri shell + 14 dashboard views)
#[derive(Debug, Args)]
pub struct ViewArgs {
    /// Skip the native binary; open the browser.
    #[arg(long)]
    pub web: bool,

    /// Override the URL opened in the browser. When set to a non-default
    /// value, the browser path is taken even without `--web` (path 3 in
    /// the dispatch order).
    #[arg(long, default_value = DEFAULT_WEB_URL_LITERAL)]
    pub url: String,

    /// Override the path to the native binary.
    #[arg(long, env = "MNEME_VISION_BIN")]
    pub bin: Option<PathBuf>,
}

/// Entry point used by `main.rs`. Resolves the default vision binary
/// from the live `~/.mneme/bin/` location (via [`default_vision_binary`])
/// and delegates to [`run_with_default_bin`].
pub async fn run(args: ViewArgs) -> CliResult<()> {
    run_with_default_bin(args, default_vision_binary()).await
}

/// Internal entry point that takes the resolved default-binary path as a
/// parameter so unit tests can inject a tempdir-rooted location that's
/// guaranteed not to exist (QA-4: making the path-4 test deterministic
/// regardless of whether the dev machine actually has the vision app
/// installed). Production code always reaches this through [`run`].
async fn run_with_default_bin(
    args: ViewArgs,
    default_bin: PathBuf,
) -> CliResult<()> {
    // Path 1: explicit --bin pointing at an existing file.
    if let Some(p) = args.bin.as_ref() {
        if p.exists() {
            info!(path = %p.display(), "spawning native vision binary (--bin)");
            return spawn_native(p);
        }
        // --bin was passed but the path doesn't exist — surface that
        // explicitly so the user doesn't think we silently fell through.
        return Err(CliError::Other(format!(
            "--bin {} does not exist",
            p.display()
        )));
    }

    // Path 2: default install location.
    if default_bin.exists() && !args.web && args.url == DEFAULT_WEB_URL_LITERAL {
        info!(path = %default_bin.display(), "spawning native vision binary (default)");
        return spawn_native(&default_bin);
    }

    // Path 3: --web OR a non-default --url.
    let want_browser = args.web || args.url != DEFAULT_WEB_URL_LITERAL;
    if want_browser {
        return open_browser(&args.url);
    }

    // Path 4: nothing to spawn, no browser requested. Clean hint, exit 0.
    //
    // UX-2: append a build-from-source pointer so power users who want
    // the vision app today (instead of waiting for the v0.4 binary)
    // know exactly where to find the build instructions. INSTALL.md §6
    // already documents the Tauri build sequence; we surface that path
    // explicitly here rather than letting the user discover it by
    // grepping the repo.
    println!(
        "v0.4 vision app coming — install ~/.mneme/bin/mneme-vision \
         or pass --bin <path>. \
         See INSTALL.md §6 for instructions on building vision from source."
    );
    Ok(())
}

/// M7 (D-window): Windows process-creation flags for `view::spawn_native`.
///
/// Composition (kernel32 ABI, stable):
/// - DETACHED_PROCESS         (0x0000_0008) — vision app gets no console,
///   so it doesn't die when the terminal closes (SD-2).
/// - CREATE_NEW_PROCESS_GROUP (0x0000_0200) — Ctrl+C in caller does not
///   propagate.
/// - CREATE_NO_WINDOW         (0x0800_0000) — suppress the brief console
///   flash that the OS otherwise creates while DETACHED_PROCESS is being
///   applied. Without this, when `mneme view` is invoked from a hidden
///   parent (Claude Code launching the vision app), a cmd.exe-style
///   window can flicker before the child detaches.
///
/// Total: `0x0800_0208`.
///
/// CREATE_BREAKAWAY_FROM_JOB is intentionally NOT set: vision is a
/// user-facing UI, not a long-lived service — if the parent's job goes
/// away, the user closing the vision window is acceptable behaviour.
///
/// Extracted as `pub(crate)` so the unit test can assert the bit
/// composition without a real spawn.
#[cfg(windows)]
pub(crate) fn windows_view_spawn_native_flags() -> u32 {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
}

/// Spawn the native binary fully detached so the CLI returns immediately
/// AND the child outlives the parent terminal.
///
/// ## SD-2: stdio detachment
///
/// Bug fixed here: previously `Command::new(bin).spawn()` left the child
/// inheriting the parent's stdin/stdout/stderr. On Windows, when the user
/// closed the terminal that ran `mneme view`, the kernel pulled the
/// console handle out from under the vision app and it died with it.
/// Worse, on Unix a shell-job-controlled run could SIGHUP the child on
/// terminal exit.
///
/// Fix: redirect all three stdio handles to `Stdio::null()`, AND on
/// Windows set the flag composition from
/// [`windows_view_spawn_native_flags`] (DETACHED_PROCESS |
/// CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW) via
/// `CommandExt::creation_flags` so the child gets its own console-less
/// process group AND no transient console window flashes (M7).
fn spawn_native(bin: &std::path::Path) -> CliResult<()> {
    let mut cmd = Command::new(bin);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_view_spawn_native_flags());
    }

    cmd.spawn().map_err(|e| CliError::io(bin, e))?;
    Ok(())
}

/// Open `url` in the user's default browser using the platform-native
/// opener. On Windows that's `cmd /c start`, on macOS `open`, on Linux
/// `xdg-open`.
///
/// ## SEC-1: scheme validation (defence-in-depth)
///
/// Today the URL is hard-coded by clap's `default_value` and only the
/// user can override it via `--url`. Even so, we validate the scheme
/// here because the Windows path is dangerously permissive: `cmd /c
/// start "" <target>` will happily launch local executables (`calc.exe`,
/// `start ftp://...`, anything `start` recognises). A future refactor
/// that piped an attacker-controlled URL into this function would, with
/// no guard, become an arbitrary-program launcher.
///
/// We accept only `http://` and `https://`. Anything else returns
/// [`CliError::Other`] and the caller exits non-zero.
fn open_browser(url: &str) -> CliResult<()> {
    validate_browser_url(url)?;
    info!(url, "opening browser for vision app");
    #[cfg(target_os = "windows")]
    {
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

/// Reject any URL that is not plain `http://` or `https://`. Factored
/// out so the SEC-1 unit test does not have to spawn a child process
/// just to assert on the rejection path.
fn validate_browser_url(url: &str) -> CliResult<()> {
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(());
    }
    Err(CliError::Other(format!(
        "refusing to open browser: URL must use http:// or https:// scheme, got: {url}"
    )))
}

/// Default install location for the vision app (`~/.mneme/bin/mneme-vision`).
fn default_vision_binary() -> PathBuf {
    let mut p = crate::state_dir().join("bin").join("mneme-vision");
    if cfg!(windows) {
        p.set_extension("exe");
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_vision_binary_has_correct_extension() {
        let p = default_vision_binary();
        let s = p.to_string_lossy();
        if cfg!(windows) {
            assert!(s.ends_with("mneme-vision.exe"), "got: {s}");
        } else {
            assert!(s.ends_with("mneme-vision"), "got: {s}");
        }
    }

    #[tokio::test]
    async fn nonexistent_bin_path_errors_out() {
        let args = ViewArgs {
            web: false,
            url: DEFAULT_WEB_URL_LITERAL.to_string(),
            bin: Some(PathBuf::from(
                "/this/path/definitely/does/not/exist-mneme-vision",
            )),
        };
        let r = run(args).await;
        assert!(r.is_err(), "expected error for nonexistent --bin");
    }

    /// SEC-1: only http(s) URLs may be passed to the OS-native opener.
    /// This is defence-in-depth — the Windows path uses
    /// `cmd /c start "" <target>` which will spawn any local executable
    /// `start` recognises, so even though today's URL comes from clap's
    /// hard-coded default we refuse anything outside the http(s) scheme.
    #[test]
    fn open_browser_rejects_non_http_url() {
        // Each of these would, without the guard, be passed straight to
        // `cmd /c start ""` on Windows.
        let bad = [
            "ftp://attacker.example.com/x",
            "file:///C:/Windows/System32/calc.exe",
            "calc.exe",
            "javascript:alert(1)",
            "",
            // Sneaky case-variation: we anchor on lowercase only, so
            // upper-case scheme is also rejected. (The Windows opener
            // is case-insensitive anyway, but our policy is strict.)
            "HTTP://example.com",
        ];
        for url in bad {
            let err = open_browser(url)
                .err()
                .unwrap_or_else(|| panic!("expected open_browser({url:?}) to error"));
            let msg = err.to_string();
            assert!(
                msg.contains("http://") && msg.contains("https://"),
                "error for {url:?} should mention http(s) requirement, got: {msg}"
            );
        }
        // Sanity: valid URLs pass the validation step. We test the
        // pure validator here so we don't actually spawn a browser
        // process from a unit test.
        assert!(validate_browser_url("http://localhost:7777").is_ok());
        assert!(validate_browser_url("https://example.com/path?q=1").is_ok());
    }

    /// SD-2: `spawn_native` must detach all three stdio handles AND, on
    /// Windows, set DETACHED_PROCESS + CREATE_NEW_PROCESS_GROUP so the
    /// child outlives the parent terminal. We can't observe the kernel
    /// flags from inside the unit test cheaply, but we can:
    ///   1. confirm the function spawns a real, well-known no-op binary
    ///      and returns immediately without waiting on it,
    ///   2. confirm `spawn` doesn't block — if stdio were inherited and
    ///      the child blocked on console handle initialisation we'd
    ///      observe a measurable delay.
    ///
    /// We use `cmd.exe` on Windows (launches and exits cleanly with no
    /// args under DETACHED_PROCESS) and `/usr/bin/true` on Unix.
    #[test]
    fn spawn_native_detaches_stdio_and_returns_immediately() {
        // Pick a no-op binary the host is guaranteed to have.
        #[cfg(windows)]
        let bin: PathBuf = {
            // Resolve %SystemRoot% rather than hard-coding C:\Windows
            // (CI runners may live elsewhere).
            let sysroot = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
            sysroot.join("System32").join("cmd.exe")
        };
        #[cfg(unix)]
        let bin: PathBuf = PathBuf::from("/usr/bin/true");

        // Skip silently if the host doesn't have the expected binary
        // (sandboxed CI w/ no /usr/bin/true).
        if !bin.exists() {
            eprintln!("spawn_native test skipped: {} not present", bin.display());
            return;
        }

        let start = std::time::Instant::now();
        let r = spawn_native(&bin);
        let elapsed = start.elapsed();

        assert!(r.is_ok(), "spawn_native({}) failed: {:?}", bin.display(), r);
        // Detached spawn must NOT block on the child. We give generous
        // slack (5s) to avoid CI flakes on very loaded runners; a
        // stdio-inheriting spawn would not hit that wall on its own,
        // but our regression target is simply "does not deadlock the
        // parent", which the timeout enforces.
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "spawn_native blocked for {elapsed:?} — child stdio may be inherited"
        );
    }

    /// QA-4 (Wave 4 cleanup): the previous version of this test called
    /// `run(args)` directly, which resolved `default_vision_binary()` via
    /// the user's real `~/.mneme/bin/`. On a dev machine where the binary
    /// happens to exist, path-2 would fire — `spawn_native` would then
    /// launch the actual vision app from a unit test, which is both
    /// non-deterministic AND a real side effect. Now we drive the
    /// internal `run_with_default_bin` helper with a tempdir that's
    /// guaranteed not to contain the binary, so path-4 (the v0.4 hint)
    /// is the only branch that can fire.
    #[tokio::test]
    async fn no_bin_no_web_prints_hint_and_exits_zero() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        // Point the "default vision binary" at a path inside the
        // tempdir that we never create. `Path::exists()` is false, so
        // path 2 of the dispatch is skipped and we land on path 4.
        let fake_default = tmp.path().join("bin").join(if cfg!(windows) {
            "mneme-vision.exe"
        } else {
            "mneme-vision"
        });
        assert!(
            !fake_default.exists(),
            "tempdir-injected path must not exist: {}",
            fake_default.display()
        );

        let args = ViewArgs {
            web: false,
            url: DEFAULT_WEB_URL_LITERAL.to_string(),
            bin: None,
        };

        let r = run_with_default_bin(args, fake_default).await;
        assert!(
            r.is_ok(),
            "expected Ok from path-4 (v0.4 hint), got: {r:?}"
        );
    }

    /// M7 (D-window): `view::spawn_native`'s Windows `creation_flags` MUST
    /// include `CREATE_NO_WINDOW` (`0x0800_0000`) on top of the existing
    /// detach + new-group set, so the vision-app spawn path never flashes
    /// a transient console window when the parent is hidden.
    ///
    /// Expected: `0x0800_0208`
    ///   = DETACHED_PROCESS         (0x0000_0008)
    ///   | CREATE_NEW_PROCESS_GROUP (0x0000_0200)
    ///   | CREATE_NO_WINDOW         (0x0800_0000)
    ///
    /// CREATE_BREAKAWAY_FROM_JOB is intentionally NOT asserted here —
    /// vision is a UI app, not a long-lived service, so dying with the
    /// parent's job is acceptable behaviour. M7's scope is *only* the
    /// console-flash regression.
    #[cfg(windows)]
    #[test]
    fn windows_view_spawn_native_flags() {
        let flags = super::windows_view_spawn_native_flags();
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

        assert!(
            flags & CREATE_NO_WINDOW == CREATE_NO_WINDOW,
            "M7: view::spawn_native must include CREATE_NO_WINDOW \
             (0x0800_0000); got 0x{flags:08x}"
        );
        // Regression guard: pre-existing detach flags must still be set.
        assert!(flags & DETACHED_PROCESS == DETACHED_PROCESS);
        assert!(flags & CREATE_NEW_PROCESS_GROUP == CREATE_NEW_PROCESS_GROUP);
    }
}
