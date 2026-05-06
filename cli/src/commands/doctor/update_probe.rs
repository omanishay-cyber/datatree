//! Update-channel probe: reads `~/.mneme/run/update_check.json` (written
//! by the daemon's background update-check task) and renders a summary
//! box. Never makes a network call.

use super::render::line;

/// Wave 2.4: render the "update channel" box.
pub fn render_update_channel_box() {
    use mneme_daemon::update_check::{is_disabled_by_env, read_cached_result};

    println!();
    println!("┌─ update channel ──────────────────────────────────────────┐");
    println!("├───────────────────────────────────────────────────────────┤");

    if is_disabled_by_env() {
        line("status", "disabled (MNEME_NO_UPDATE_CHECK is set)");
        println!("└───────────────────────────────────────────────────────────┘");
        return;
    }

    let run_dir = crate::runtime_dir();
    let current = env!("CARGO_PKG_VERSION");

    line("current", &format!("v{current}"));

    match read_cached_result(&run_dir) {
        None => {
            line("available", "unknown — daemon hasn't checked yet");
            line("action", "start the daemon to enable background checks");
            line("last checked", "never");
        }
        Some(result) => {
            match (&result.latest_version, result.update_available) {
                (Some(v), Some(true)) => {
                    line("available", &format!("v{v}  (update ready)"));
                    line("action", "run `mneme self-update` to apply");
                }
                (Some(v), Some(false)) => {
                    line("available", &format!("v{v}  (up to date)"));
                }
                (Some(v), None) => {
                    line("available", &format!("v{v}  (comparison inconclusive)"));
                }
                (None, _) => {
                    if let Some(ref err) = result.last_error {
                        line("available", &format!("check failed — {err}"));
                    } else {
                        line("available", "unknown");
                    }
                }
            }

            let ts = result.last_checked_at.unwrap_or(result.last_attempt_at);
            let ts_str = ts.format("%Y-%m-%d %H:%M UTC").to_string();
            line(
                "last checked",
                &if result.last_checked_at.is_some() {
                    ts_str
                } else {
                    format!("{ts_str} (attempt — check failed)")
                },
            );
        }
    }

    println!("└───────────────────────────────────────────────────────────┘");
}
