//! Rendering helpers: box-drawing, colorised value cells, banner,
//! timestamp. Everything that writes to stdout but holds no probe logic.

/// Inside-width of the banner box (chars between the two `║`).
pub(super) const BANNER_WIDTH: usize = 62;

/// Single source of truth for the copyright line printed in the banner.
/// Canonical names confirmed 2026-04-25. Closes I-14.
pub(super) const COPYRIGHT: &str = "© 2026 Anish Trivedi & Kruti Trivedi";

// ─── primary cell renderer ───────────────────────────────────────────────────

/// Render one `│ label : value │` box row, padding the right border to
/// a fixed column. The value cell is colourised based on its leading
/// status word; width arithmetic uses the unstyled string so ANSI
/// escape bytes don't shift the border.
pub fn line(label: &str, value: &str) {
    let padded_label = format!("{label:<17}");
    let unstyled = format!("│ {padded_label}: {value}");
    let visible_len = unstyled.chars().count();
    let target = 59;
    let pad = if visible_len < target {
        " ".repeat(target - visible_len)
    } else {
        String::new()
    };
    let styled_value = colorize_status_value(value);
    let styled_line = format!("│ {padded_label}: {styled_value}");
    println!("{styled_line}{pad}│");
}

// ─── colourisation ────────────────────────────────────────────────────────────

/// Wrap a doctor value cell in `console::style` based on its leading
/// status word. Only well-known banner words (OK, PASS, FAIL, MISSING,
/// WARN, READY) get tint; everything else passes through plain so we
/// don't over-paint values that happen to start with a coloured
/// substring.
fn colorize_status_value(value: &str) -> String {
    let trimmed = value.trim_start();
    let (color_kind, _len) = leading_status_word(trimmed);
    match color_kind {
        StatusColor::Ok => console::style(value).green().to_string(),
        StatusColor::Warn => console::style(value).yellow().to_string(),
        StatusColor::Fail => console::style(value).red().to_string(),
        StatusColor::Info => console::style(value).cyan().to_string(),
        StatusColor::None => value.to_string(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum StatusColor {
    Ok,
    Warn,
    Fail,
    Info,
    None,
}

pub(super) fn leading_status_word(s: &str) -> (StatusColor, usize) {
    // Match longer prefixes first so "MISSING" doesn't match "M".
    const PATTERNS: &[(&str, StatusColor)] = &[
        ("OK", StatusColor::Ok),
        ("PASS", StatusColor::Ok),
        ("READY", StatusColor::Ok),
        ("CONNECTED", StatusColor::Ok),
        ("FOUND", StatusColor::Ok),
        ("RUNNING", StatusColor::Ok),
        ("ON", StatusColor::Ok),
        ("YES", StatusColor::Ok),
        ("FAIL", StatusColor::Fail),
        ("FAILED", StatusColor::Fail),
        ("ERROR", StatusColor::Fail),
        ("MISSING", StatusColor::Fail),
        ("DOWN", StatusColor::Fail),
        ("UNREACHABLE", StatusColor::Fail),
        ("WARN", StatusColor::Warn),
        ("WARNING", StatusColor::Warn),
        ("DEGRADED", StatusColor::Warn),
        ("STALE", StatusColor::Warn),
        ("OFF", StatusColor::Warn),
        ("NO", StatusColor::Warn),
        ("INFO", StatusColor::Info),
        ("UNKNOWN", StatusColor::Info),
    ];
    for (needle, color) in PATTERNS {
        if s.starts_with(needle) {
            // Confirm word boundary — next char (if any) is non-alnum.
            let rest = &s[needle.len()..];
            let bound = rest
                .chars()
                .next()
                .map(|c| !c.is_alphanumeric() && c != '_')
                .unwrap_or(true);
            if bound {
                return (*color, needle.len());
            }
        }
    }
    (StatusColor::None, 0)
}

// ─── banner ──────────────────────────────────────────────────────────────────

/// Print the boxed banner. Version line + copyright line use dynamic
/// padding so longer pre-release versions don't overflow the right
/// border. Closes NEW-026 + I-14.
///
/// A1-006 (2026-05-04): on terminals narrower than 64 columns the box
/// wraps illegibly. Detect width via env var and emit a single-line
/// fallback so log scrapers / piped contexts see clean output.
pub fn print_banner() {
    let term_too_narrow = std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|c| c < 64)
        .unwrap_or(false);
    if term_too_narrow {
        println!(
            "mneme doctor v{} -- 100% local Apache-2.0 -- (c) 2026 Anish & Kruti Trivedi",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                                                              ║");
    println!("║   ███╗   ███╗███╗   ██╗███████╗███╗   ███╗███████╗           ║");
    println!("║   ████╗ ████║████╗  ██║██╔════╝████╗ ████║██╔════╝           ║");
    println!("║   ██╔████╔██║██╔██╗ ██║█████╗  ██╔████╔██║█████╗             ║");
    println!("║   ██║╚██╔╝██║██║╚██╗██║██╔══╝  ██║╚██╔╝██║██╔══╝             ║");
    println!("║   ██║ ╚═╝ ██║██║ ╚████║███████╗██║ ╚═╝ ██║███████╗           ║");
    println!("║   ╚═╝     ╚═╝╚═╝  ╚═══╝╚══════╝╚═╝     ╚═╝╚══════╝           ║");
    println!("║                                                              ║");
    // A1-005 (2026-05-04): drop hardcoded tool count from banner — live
    // count is reported in render_mcp_tool_probe_box.
    println!("║   persistent memory * code graph * drift detector            ║");
    print_banner_line(&format!(
        "   v{} · 100% local · Apache-2.0",
        env!("CARGO_PKG_VERSION")
    ));
    println!("║                                                              ║");
    print_banner_line(&format!("   {COPYRIGHT}"));
    println!("║                                                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}

/// Render one inside-the-box line, padding (or truncating) to the
/// banner width so the right border always lands in the same column.
pub fn print_banner_line(content: &str) {
    let visible = content.chars().count();
    if visible >= BANNER_WIDTH {
        let mut out = String::new();
        for (i, ch) in content.chars().enumerate() {
            if i + 1 >= BANNER_WIDTH {
                break;
            }
            out.push(ch);
        }
        out.push('…');
        println!("║{out}║");
    } else {
        let pad = " ".repeat(BANNER_WIDTH - visible);
        println!("║{content}{pad}║");
    }
}

// ─── timestamp ───────────────────────────────────────────────────────────────

/// `YYYY-MM-DD HH:MM:SS UTC` without pulling chrono.
pub fn utc_now_readable() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let s = secs % 86_400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    let (y, m, d) = ymd(days);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}

fn ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 {
        (mp + 3) as u32
    } else {
        (mp - 9) as u32
    };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

// ─── update-channel "available" row formatter ─────────────────────────────────

/// Unit-testable: compose the rendered value string for the "available"
/// row of the update channel box. Pure function — no I/O.
#[allow(dead_code)]
pub fn format_available_row(
    latest: Option<&str>,
    update_available: Option<bool>,
    last_error: Option<&str>,
) -> String {
    match (latest, update_available) {
        (Some(v), Some(true)) => format!("v{v}  (update ready)"),
        (Some(v), Some(false)) => format!("v{v}  (up to date)"),
        (Some(v), None) => format!("v{v}  (comparison inconclusive)"),
        (None, _) => {
            if let Some(err) = last_error {
                format!("check failed — {err}")
            } else {
                "unknown".to_string()
            }
        }
    }
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod color_tests {
    use super::{leading_status_word, StatusColor};

    #[test]
    fn ok_words_resolve_to_ok_color() {
        assert_eq!(leading_status_word("OK").0, StatusColor::Ok);
        assert_eq!(leading_status_word("PASS").0, StatusColor::Ok);
        assert_eq!(leading_status_word("READY").0, StatusColor::Ok);
        assert_eq!(leading_status_word("RUNNING (pid 1234)").0, StatusColor::Ok);
    }

    #[test]
    fn fail_words_resolve_to_fail_color() {
        assert_eq!(leading_status_word("FAIL").0, StatusColor::Fail);
        assert_eq!(
            leading_status_word("MISSING (no socket)").0,
            StatusColor::Fail
        );
        assert_eq!(leading_status_word("ERROR opening db").0, StatusColor::Fail);
    }

    #[test]
    fn warn_words_resolve_to_warn_color() {
        assert_eq!(leading_status_word("WARN").0, StatusColor::Warn);
        assert_eq!(leading_status_word("DEGRADED").0, StatusColor::Warn);
    }

    #[test]
    fn unrelated_text_resolves_to_none() {
        assert_eq!(leading_status_word("v0.4.0").0, StatusColor::None);
        assert_eq!(leading_status_word("hello").0, StatusColor::None);
    }

    #[test]
    fn substring_match_requires_word_boundary() {
        // "OKAY" must NOT match "OK" because OKAY isn't in our list
        // and the next char is alphanumeric — the boundary check
        // protects against this kind of accidental tint.
        assert_eq!(leading_status_word("OKAY").0, StatusColor::None);
        // "FAILURE" likewise — we want exact known words.
        assert_eq!(leading_status_word("FAILURE").0, StatusColor::None);
    }
}
