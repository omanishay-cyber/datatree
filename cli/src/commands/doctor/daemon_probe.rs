//! Daemon liveness probes: PID-file state, socket-writability checks,
//! and the `which_on_path` PATH-search helper shared by the toolchain
//! probe.

use std::path::{Path, PathBuf};

// ─── writability ─────────────────────────────────────────────────────────────

/// Return `true` if `path` is a writable directory (creates it if
/// absent). Uses a probe file write+delete to confirm actual write
/// access, not just permission bits.
pub fn is_writable(path: &Path) -> bool {
    std::fs::create_dir_all(path)
        .and_then(|_| {
            let probe = path.join(".mneme-probe");
            std::fs::write(&probe, b"")?;
            std::fs::remove_file(&probe)
        })
        .is_ok()
}

// ─── PID-file state ──────────────────────────────────────────────────────────

/// B-017/B-018 (D:\Mneme Dome cycle, 2026-04-30): three-state result of
/// inspecting `~/.mneme/run/daemon.pid`. Drives the doctor's pre-IPC
/// gate so we never block on a wedged stale named pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonPidState {
    /// `daemon.pid` is missing entirely — supervisor has never run, or
    /// uninstall already removed the runtime dir.
    Missing,
    /// `daemon.pid` exists but the PID inside is no longer alive (the
    /// supervisor crashed or was killed without cleanup). The pipe at
    /// `~/.mneme/supervisor.pipe` may still be present and may even
    /// accept connects — DO NOT IPC.
    Stale,
    /// `daemon.pid` exists and the PID is alive. A short-budget IPC
    /// probe is safe (and required) to confirm liveness.
    AliveProbeFresh,
}

/// B-017/B-018: read `~/.mneme/run/daemon.pid` and classify its state.
///
/// Pure-stdlib + `sysinfo` (already a workspace dep). Never panics;
/// any IO error degrades to `Missing` so we still attempt IPC if a
/// transient FS hiccup hides the pid file.
pub fn check_daemon_pid_liveness(state_dir: &Path) -> DaemonPidState {
    let pid_file = state_dir.join("run").join("daemon.pid");
    let raw = match std::fs::read_to_string(&pid_file) {
        Ok(s) => s,
        Err(_) => return DaemonPidState::Missing,
    };
    let pid: u32 = match raw.trim().parse() {
        Ok(p) => p,
        Err(_) => return DaemonPidState::Stale,
    };
    if is_pid_alive(pid) {
        DaemonPidState::AliveProbeFresh
    } else {
        DaemonPidState::Stale
    }
}

/// Cross-platform liveness probe for a numeric PID. Returns `true` if a
/// process with that PID currently exists AND the executable name looks
/// like a mneme binary (defends against Windows PID reuse).
///
/// A1-007 (2026-05-04): Windows reuses PIDs aggressively. Filtering by
/// `name.starts_with("mneme")` rules out the false positive cleanly.
/// Full hardening (PID + start_time + exe_name in daemon.pid) is a
/// v0.3.3 follow-up requiring supervisor-side changes to the .pid
/// writer. This intermediate fix gets ~95% of the way there.
fn is_pid_alive(pid: u32) -> bool {
    use sysinfo::{Pid, System};
    let mut sys = System::new();
    sys.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
    );
    match sys.process(Pid::from_u32(pid)) {
        Some(proc_) => {
            let name = proc_.name().to_string_lossy().to_lowercase();
            // Accept any mneme-named process. Empty name is rare but
            // possible on transient Windows kernel-mode states; treat as
            // alive (better false-positive than false-negative).
            name.is_empty() || name.starts_with("mneme") || name.starts_with("mneme-")
        }
        None => false,
    }
}

// ─── PATH search ─────────────────────────────────────────────────────────────

/// Search PATH for `name` (with platform-appropriate extensions) and
/// return the first hit, or `None` if not present.
///
/// Pure-stdlib so we don't need the `which` crate just for the doctor
/// probe.
pub fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .ok()
            .map(|s| s.split(';').map(|e| e.to_string()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![".EXE".into(), ".CMD".into(), ".BAT".into(), ".COM".into()])
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path_var) {
        for ext in &exts {
            let candidate = if ext.is_empty() {
                dir.join(name)
            } else {
                // Skip if the name already has an extension on Windows.
                if cfg!(windows) && Path::new(name).extension().is_some() {
                    dir.join(name)
                } else {
                    dir.join(format!("{name}{ext}"))
                }
            };
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
