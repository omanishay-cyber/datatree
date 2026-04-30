//! K10 chaos-test-only fault-injection hooks.
//!
//! This module is compiled into the daemon ONLY when one of:
//!   * `cfg(test)` (in-tree unit tests)
//!   * `cfg(feature = "test-hooks")` (CLI integration tests under
//!     `cli/tests/chaos_tests.rs` build with this feature enabled)
//!
//! Production binaries never compile this code in. Every call site
//! that reads the counter is gated by the same `#[cfg]` so a stripped
//! release build cannot accidentally invoke the panic path.
//!
//! ## Counter semantics
//!
//! `INJECT_CRASH_COUNTDOWN` is a process-global atomic:
//!   * `0` ⇒ disabled. `crash_if_armed()` is a no-op.
//!   * `N > 0` ⇒ armed. Each call to `crash_if_armed()` decrements
//!     the counter; when the decrement returns `1` (i.e. this was the
//!     Nth call), `panic!()` fires. Subsequent calls observe `0` and
//!     are no-ops, so the supervisor restart loop sees a single panic
//!     and respawns the worker exactly once per arm.
//!
//! ## Why an atomic countdown rather than a one-shot bool
//!
//! The chaos test wants "panic on the Nth job", which lets the test
//! exercise the supervisor's recovery path on a non-trivial dispatch
//! sequence. Using an atomic countdown means we don't need any sync
//! primitives in the hot dispatch path beyond the `fetch_sub` itself.

use std::sync::atomic::{AtomicU64, Ordering};

/// Process-global countdown for the `--inject-crash` chaos hook.
///
/// `0` means disabled. Set by the daemon binary's `Start` arm at boot
/// when `--inject-crash <N>` is passed; read on every job dispatch.
static INJECT_CRASH_COUNTDOWN: AtomicU64 = AtomicU64::new(0);

/// Arm the countdown to fire on the `n`-th call to [`crash_if_armed`].
///
/// `n == 0` disables the hook. Subsequent calls overwrite the counter,
/// which is the documented contract: only one `--inject-crash` value
/// is honored per supervisor process. Tests that need multiple panics
/// must restart the supervisor between scenarios.
pub fn set_inject_crash(n: u64) {
    INJECT_CRASH_COUNTDOWN.store(n, Ordering::SeqCst);
}

/// Read the current countdown without decrementing. For diagnostic /
/// log-line use; the dispatcher uses [`crash_if_armed`] which performs
/// the atomic countdown itself.
pub fn current_countdown() -> u64 {
    INJECT_CRASH_COUNTDOWN.load(Ordering::SeqCst)
}

/// Decrement the countdown and panic when it hits zero.
///
/// Called inside `ChildManager::dispatch_to_pool` on every dispatch.
/// When the countdown is `0`, this is a single relaxed atomic load
/// — measurable but negligible cost. When armed, it uses `fetch_sub`
/// to ensure only one caller in a multi-worker race triggers the panic.
///
/// The panic propagates as a normal Rust panic. With the workspace's
/// `panic = "abort"` profile in release builds, this aborts the
/// process. Inside the supervisor the dispatcher runs on a tokio task
/// that the per-child monitor wraps — the abort is observed as an
/// unexpected child exit, which the restart loop respawns.
pub fn crash_if_armed() {
    // Fast path: not armed.
    if INJECT_CRASH_COUNTDOWN.load(Ordering::Relaxed) == 0 {
        return;
    }
    // Armed path: decrement under SeqCst so concurrent dispatchers
    // race on the SAME counter and only one observes the "1 → 0"
    // transition (the one that returns `1` from `fetch_sub`).
    let prev = INJECT_CRASH_COUNTDOWN.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        // This was the Nth call. Panic.
        panic!(
            "K10 test hook fired: --inject-crash countdown reached zero \
             (this panic is intentional and is captured by the supervisor \
             restart loop)"
        );
    }
    // If `prev == 0` we under-flowed because the counter was already
    // exhausted; saturating-clamp to 0 so subsequent calls don't
    // re-trigger via wraparound.
    if prev == 0 {
        INJECT_CRASH_COUNTDOWN.store(0, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    /// Reset the global counter so each test starts from a known state.
    /// Tests run sequentially within this module via `#[serial_test]`-
    /// style explicit reset since we don't depend on `serial_test`.
    fn reset() {
        INJECT_CRASH_COUNTDOWN.store(0, Ordering::SeqCst);
    }

    #[test]
    fn disabled_is_noop() {
        reset();
        for _ in 0..10 {
            crash_if_armed(); // must not panic
        }
        assert_eq!(current_countdown(), 0);
    }

    #[test]
    #[should_panic(expected = "K10 test hook fired")]
    fn arms_and_panics_on_nth_call() {
        reset();
        set_inject_crash(3);
        crash_if_armed(); // 3 -> 2
        crash_if_armed(); // 2 -> 1
        crash_if_armed(); // 1 -> 0, panic
    }

    #[test]
    fn decrements_then_settles() {
        reset();
        set_inject_crash(5);
        crash_if_armed();
        assert_eq!(current_countdown(), 4);
        crash_if_armed();
        assert_eq!(current_countdown(), 3);
    }
}
