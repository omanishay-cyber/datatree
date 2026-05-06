//! HIGH-17 regression: shell-metacharacter sanitisation of MNEME_HOME.
//!
//! Threat: the Windows detached uninstall path historically interpolated
//! mneme_dir.display() into a shell-parsed cmd /c string. If MNEME_HOME
//! contained shell metacharacters (& | ^ < > "), the cmd.exe parser
//! interpreted them as command separators and ran arbitrary code as the
//! uninstalling user.
//!
//! Fix:
//!   1. sanitize_mneme_home rejects any of the dangerous chars at the
//!      boundary (defence in depth, Option B).
//!   2. The recursive-delete step is moved out of cmd /c into the
//!      PowerShell -EncodedCommand body, where the path is wrapped in
//!      a single-quoted literal and consumed by Remove-Item -LiteralPath
//!      (Option A).

use mneme_cli::commands::uninstall::sanitize_mneme_home;
use std::path::Path;

#[test]
#[cfg(windows)]
fn rejects_ampersand_command_chain() {
    // Windows-only: tests cmd.exe metachar attack via MNEME_HOME path.
    // On POSIX, the legacy path doesn't shell-out so the test is moot.
    let evil = Path::new(r"C:\Users\Public\foo & calc.exe");
    let result = sanitize_mneme_home(evil);
    assert!(result.is_err());
    let msg = result.err().unwrap();
    assert!(msg.contains("metacharacter"));
}

#[test]
fn rejects_pipe_command_chain() {
    let evil = Path::new(r"C:\path | net user attacker pwn /add");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_caret_escape() {
    let evil = Path::new(r"C:\path^&calc.exe");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_output_redirect() {
    let evil = Path::new(r"C:\foo>C:\Windows\System32\drivers\etc\hosts");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_input_redirect() {
    let evil = Path::new(r"C:\foo<C:\secret.txt");
    assert!(sanitize_mneme_home(evil).is_err());
}
#[test]
fn rejects_double_quote() {
    let evil = Path::new("C:\\foo\" & calc.exe & \"C:\\bar");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_single_quote() {
    let evil = Path::new("C:\\foo'; calc.exe; '");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_backtick() {
    let evil = Path::new("C:\\foo`%calc.exe");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_percent_env_expansion() {
    let evil = Path::new("C:\\foo%PATH%");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_dollar_var_expansion() {
    let evil = Path::new(r"C:\foo$env:USERPROFILE");
    assert!(sanitize_mneme_home(evil).is_err());
}
#[test]
fn rejects_newline() {
    let evil_str = "C:\\foo\ncalc.exe";
    let evil = Path::new(evil_str);
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_carriage_return() {
    let evil_str = "C:\\foo\rcalc.exe";
    let evil = Path::new(evil_str);
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_nul_byte() {
    let evil_str = "C:\\foo\0calc.exe";
    let evil = Path::new(evil_str);
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
#[cfg(windows)]
fn rejects_wildcard_asterisk() {
    // Windows-only: PowerShell glob wildcards via Remove-Item path.
    // POSIX paths permit literal asterisks so the test is Windows-scoped.
    let evil = Path::new(r"C:\Users\*\Documents");
    let r = sanitize_mneme_home(evil);
    assert!(r.is_err());
    assert!(r.err().unwrap().contains("wildcard"));
}

#[test]
fn rejects_wildcard_question() {
    let evil = Path::new(r"C:\Users\?dmin\Documents");
    assert!(sanitize_mneme_home(evil).is_err());
}

#[test]
fn rejects_relative_path() {
    let evil = Path::new(".mneme");
    let r = sanitize_mneme_home(evil);
    assert!(r.is_err());
    assert!(r.err().unwrap().contains("absolute"));
}

#[test]
fn rejects_empty_path() {
    let evil = Path::new("");
    assert!(sanitize_mneme_home(evil).is_err());
}
#[test]
#[cfg(windows)]
fn accepts_canonical_windows_install_path() {
    let canonical = Path::new(r"C:\Users\Anish\.mneme");
    let r = sanitize_mneme_home(canonical);
    assert!(
        r.is_ok(),
        "canonical install path must be accepted; got {:?}",
        r
    );
}

#[test]
#[cfg(not(windows))]
fn accepts_canonical_posix_install_path() {
    let canonical = Path::new("/home/anish/.mneme");
    let r = sanitize_mneme_home(canonical);
    assert!(
        r.is_ok(),
        "canonical install path must be accepted; got {:?}",
        r
    );
}

#[test]
fn accepts_path_with_spaces() {
    #[cfg(windows)]
    let path = Path::new(r"C:\Users\First Last\.mneme");
    #[cfg(not(windows))]
    let path = Path::new("/home/first last/.mneme");
    let r = sanitize_mneme_home(path);
    assert!(r.is_ok(), "paths with spaces must be accepted: {:?}", r);
}

#[test]
#[cfg(windows)]
fn accepts_program_data_override() {
    let path = Path::new(r"C:\ProgramData\mneme");
    assert!(sanitize_mneme_home(path).is_ok());
}

#[test]
#[cfg(not(windows))]
fn accepts_var_lib_default() {
    let path = Path::new("/var/lib/mneme");
    assert!(sanitize_mneme_home(path).is_ok());
}

#[test]
fn rejects_audit_high17_example_verbatim() {
    let evil = Path::new(r"C:\Users\Public\Pwned & calc.exe");
    let r = sanitize_mneme_home(evil);
    assert!(r.is_err());
}
