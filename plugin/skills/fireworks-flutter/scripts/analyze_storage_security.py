#!/usr/bin/env python3
"""
analyze_storage_security.py - Scan Flutter projects for insecure data storage.

Detects sensitive data in SharedPreferences, unencrypted file writes,
SQLite without encryption, SQL injection via string interpolation,
and Android backup misconfiguration.

Outputs structured JSON to stdout. Exit code 1 if issues, 0 if clean.
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone
from typing import Any, Dict, List, Tuple

SCANNER_NAME = "analyze_storage_security"
VERSION = "1.0.0"

# ---------------------------------------------------------------------------
# Sensitive data keywords (used to detect secrets stored insecurely)
# ---------------------------------------------------------------------------
SENSITIVE_KEYWORDS = [
    'password', 'passwd', 'pwd', 'secret', 'token', 'api_key', 'apikey',
    'api-key', 'auth_token', 'access_token', 'refresh_token', 'private_key',
    'privatekey', 'credential', 'ssn', 'social_security', 'credit_card',
    'creditcard', 'card_number', 'cvv', 'pin', 'otp', 'mfa', 'totp',
    'session_id', 'sessionid', 'jwt', 'bearer', 'encryption_key',
    'master_key', 'signing_key', 'client_secret',
]

SENSITIVE_PATTERN = re.compile(
    r'(?i)(?:' + '|'.join(re.escape(kw) for kw in SENSITIVE_KEYWORDS) + r')',
)

# ---------------------------------------------------------------------------
# Storage security patterns for Dart files
# ---------------------------------------------------------------------------
DART_PATTERNS: List[Tuple[str, str, str, str, str]] = [
    # SharedPreferences with sensitive data
    (
        "SharedPreferences sensitive write",
        r'(?:prefs|preferences|sharedPreferences|sp)\s*\.\s*set(?:String|Int|Bool|Double|StringList)\s*\(\s*["\'](?:[^"\']*(?:'
        + '|'.join(re.escape(kw) for kw in SENSITIVE_KEYWORDS)
        + r')[^"\']*)["\']',
        "HIGH",
        "insecure_shared_prefs",
        "Sensitive data stored in SharedPreferences (plaintext on disk) - use flutter_secure_storage instead",
    ),
    (
        "SharedPreferences key with sensitive name",
        r'''getString\s*\(\s*["'](?:[^"']*(?:'''
        + '|'.join(re.escape(kw) for kw in SENSITIVE_KEYWORDS[:15])
        + r''')[^"']*)['"]\s*\)''',
        "HIGH",
        "insecure_shared_prefs",
        "Reading potentially sensitive data from SharedPreferences - should use encrypted storage",
    ),

    # Unencrypted file writes
    (
        "File write with sensitive data",
        r'(?:writeAsString|writeAsBytes|writeAsStringSync|writeAsBytesSync)\s*\(',
        "MEDIUM",
        "unencrypted_file_write",
        "File write detected - verify sensitive data is encrypted before writing",
    ),

    # SQLite without encryption
    (
        "sqflite without sqlcipher",
        r'(?:openDatabase|getDatabasesPath)\s*\(',
        "MEDIUM",
        "unencrypted_database",
        "SQLite database opened - if storing sensitive data, use sqflite_sqlcipher or encrypt fields",
    ),

    # SQL injection via string interpolation
    (
        "SQL injection - string interpolation",
        r'''(?:rawQuery|rawInsert|rawUpdate|rawDelete|execute)\s*\(\s*(?:["']|\'\'\'|""")[^"\']*\$(?:\{[^}]+\}|[a-zA-Z_]\w*)''',
        "CRITICAL",
        "sql_injection",
        "SQL query uses string interpolation - vulnerable to SQL injection; use parameterized queries",
    ),
    (
        "SQL injection - string concatenation",
        r'''(?:rawQuery|rawInsert|rawUpdate|rawDelete|execute)\s*\(\s*[^,)]*\+\s*(?:[a-zA-Z_]\w*)''',
        "CRITICAL",
        "sql_injection",
        "SQL query uses string concatenation - vulnerable to SQL injection; use parameterized queries",
    ),

    # Hive without encryption
    (
        "Hive box without encryption",
        r'Hive\.openBox\s*[<(]',
        "MEDIUM",
        "unencrypted_database",
        "Hive box opened without encryption - use encryptionCipher parameter for sensitive data",
    ),

    # Insecure random for security
    (
        "Math.Random for security",
        r'Random\(\)',
        "MEDIUM",
        "weak_crypto",
        "Random() without .secure() - use Random.secure() for cryptographic purposes",
    ),

    # flutter_secure_storage without options
    (
        "Secure storage without Android options",
        r'FlutterSecureStorage\(\s*\)',
        "MEDIUM",
        "storage_config",
        "FlutterSecureStorage created without AndroidOptions - defaults may use deprecated KeyStore on old devices",
    ),

    # Logging sensitive data
    (
        "Print/log with sensitive data",
        r'(?:print|log|debugPrint|logger)\s*\(\s*[^)]*(?:'
        + '|'.join(re.escape(kw) for kw in SENSITIVE_KEYWORDS[:10])
        + r')[^)]*\)',
        "HIGH",
        "sensitive_logging",
        "Potentially logging sensitive data - ensure secrets are not printed in production",
    ),

    # Clipboard with sensitive data
    (
        "Clipboard with sensitive data",
        r'Clipboard\.setData\s*\(\s*ClipboardData\s*\([^)]*(?:'
        + '|'.join(re.escape(kw) for kw in SENSITIVE_KEYWORDS[:10])
        + r')[^)]*\)',
        "HIGH",
        "clipboard_leak",
        "Sensitive data copied to clipboard - clipboard is accessible by other apps",
    ),

    # WebView cookie / localStorage access
    (
        "WebView JavaScript enabled",
        r'javascriptMode\s*:\s*JavascriptMode\.unrestricted',
        "MEDIUM",
        "webview_security",
        "WebView with unrestricted JavaScript - ensure content is trusted to prevent XSS",
    ),
]


def scan_dart_files(project_dir: str, verbose: bool) -> List[Dict[str, Any]]:
    """Scan Dart source files for insecure storage patterns."""
    findings: List[Dict[str, Any]] = []

    scan_dirs = ['lib', 'test', 'integration_test']
    for scan_dir_name in scan_dirs:
        scan_dir = os.path.join(project_dir, scan_dir_name)
        if not os.path.isdir(scan_dir):
            continue

        for root, _dirs, filenames in os.walk(scan_dir):
            for fname in filenames:
                if not fname.endswith('.dart'):
                    continue
                # Skip generated files
                if '.g.dart' in fname or '.freezed.dart' in fname or '.mocks.dart' in fname:
                    continue

                filepath = os.path.join(root, fname)
                try:
                    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
                        lines = f.readlines()
                except (IOError, OSError):
                    continue

                # Track imports for context
                has_secure_storage = False
                has_sqflite = False
                has_hive = False
                has_shared_prefs = False

                for line in lines:
                    if 'flutter_secure_storage' in line:
                        has_secure_storage = True
                    if 'sqflite' in line:
                        has_sqflite = True
                    if 'hive' in line.lower():
                        has_hive = True
                    if 'shared_preferences' in line:
                        has_shared_prefs = True

                for line_num, line in enumerate(lines, start=1):
                    for pattern_name, pattern_regex, severity, category, description in DART_PATTERNS:
                        match = re.search(pattern_regex, line, re.IGNORECASE)
                        if not match:
                            continue

                        # Context-aware false positive detection
                        fp_risk = "LOW"
                        stripped = line.strip()

                        # Comments
                        if stripped.startswith('//') or stripped.startswith('/*') or stripped.startswith('*'):
                            fp_risk = "HIGH"
                            if not verbose:
                                continue

                        # Test files
                        norm_path = filepath.replace('\\', '/')
                        if '/test/' in norm_path or '_test.dart' in norm_path:
                            fp_risk = "MEDIUM"

                        # For file write detection, check if line actually has sensitive content
                        if category == "unencrypted_file_write":
                            # Look at surrounding context (same line and nearby) for sensitive keywords
                            context_window = ''.join(lines[max(0, line_num - 3):min(len(lines), line_num + 2)])
                            if not SENSITIVE_PATTERN.search(context_window):
                                fp_risk = "HIGH"
                                if not verbose:
                                    continue

                        # For database patterns, only flag if no encryption is detected nearby
                        if category == "unencrypted_database" and 'sqlcipher' in ''.join(lines).lower():
                            fp_risk = "HIGH"
                            if not verbose:
                                continue

                        # For Random(), check if it's actually used for security
                        if 'Random()' in line and category == "weak_crypto":
                            context = ''.join(lines[max(0, line_num - 5):min(len(lines), line_num + 5)])
                            security_words = ['token', 'key', 'secret', 'nonce', 'salt', 'iv', 'otp', 'code', 'password']
                            if not any(sw in context.lower() for sw in security_words):
                                fp_risk = "HIGH"
                                if not verbose:
                                    continue

                        findings.append({
                            "file": filepath,
                            "line": line_num,
                            "severity": severity,
                            "category": category,
                            "description": f"{pattern_name}: {description}",
                            "false_positive_risk": fp_risk,
                            "line_content": stripped[:120],
                        })

    return findings


def check_android_backup_config(project_dir: str) -> List[Dict[str, Any]]:
    """Check Android manifest for backup misconfiguration."""
    findings: List[Dict[str, Any]] = []

    manifest_path = os.path.join(
        project_dir, "android", "app", "src", "main", "AndroidManifest.xml"
    )
    if not os.path.isfile(manifest_path):
        return findings

    try:
        with open(manifest_path, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()

        # allowBackup="true" (or not set, which defaults to true)
        if 'android:allowBackup="true"' in content:
            findings.append({
                "file": manifest_path,
                "line": 0,
                "severity": "HIGH",
                "category": "backup_misconfiguration",
                "description": "android:allowBackup=\"true\" - app data can be extracted via ADB backup. "
                               "Set to false or configure backup rules to exclude sensitive data.",
                "false_positive_risk": "LOW",
            })
        elif 'android:allowBackup' not in content:
            findings.append({
                "file": manifest_path,
                "line": 0,
                "severity": "MEDIUM",
                "category": "backup_misconfiguration",
                "description": "android:allowBackup not explicitly set (defaults to true on API <31). "
                               "Explicitly set to false or configure backup rules.",
                "false_positive_risk": "MEDIUM",
            })

        # Check for fullBackupContent reference (good practice)
        if 'android:fullBackupContent' not in content and 'android:dataExtractionRules' not in content:
            if 'android:allowBackup="true"' in content:
                findings.append({
                    "file": manifest_path,
                    "line": 0,
                    "severity": "MEDIUM",
                    "category": "backup_misconfiguration",
                    "description": "Backup enabled but no fullBackupContent or dataExtractionRules defined - "
                                   "all app data will be included in backups",
                    "false_positive_risk": "LOW",
                })

        # Check for debuggable
        if 'android:debuggable="true"' in content:
            findings.append({
                "file": manifest_path,
                "line": 0,
                "severity": "CRITICAL",
                "category": "debug_enabled",
                "description": "android:debuggable=\"true\" in release manifest - "
                               "allows attaching debugger and extracting data",
                "false_positive_risk": "LOW",
            })

    except (IOError, OSError):
        pass

    return findings


def check_ios_data_protection(project_dir: str) -> List[Dict[str, Any]]:
    """Check iOS configuration for data protection issues."""
    findings: List[Dict[str, Any]] = []

    # Check entitlements for data protection level
    entitlements_glob_dirs = [
        os.path.join(project_dir, "ios", "Runner"),
    ]

    for ent_dir in entitlements_glob_dirs:
        if not os.path.isdir(ent_dir):
            continue
        for fname in os.listdir(ent_dir):
            if not fname.endswith('.entitlements'):
                continue
            ent_path = os.path.join(ent_dir, fname)
            try:
                with open(ent_path, 'r', encoding='utf-8', errors='ignore') as f:
                    content = f.read()

                # NSFileProtectionNone is the weakest level
                if 'NSFileProtectionNone' in content:
                    findings.append({
                        "file": ent_path,
                        "line": 0,
                        "severity": "HIGH",
                        "category": "data_protection",
                        "description": "iOS data protection set to NSFileProtectionNone - "
                                       "files accessible even when device is locked",
                        "false_positive_risk": "LOW",
                    })
            except (IOError, OSError):
                pass

    return findings


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Scan a Flutter project for insecure data storage patterns.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python analyze_storage_security.py --project-dir /path/to/flutter/app\n"
            "  python analyze_storage_security.py --verbose\n"
        ),
    )
    parser.add_argument(
        "--project-dir",
        default=os.getcwd(),
        help="Path to the Flutter project root (default: current directory)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Include findings with HIGH false-positive risk in output",
    )
    args = parser.parse_args()

    project_dir = os.path.abspath(args.project_dir)
    if not os.path.isdir(project_dir):
        print(json.dumps({"error": f"Directory not found: {project_dir}"}), file=sys.stderr)
        return 1

    all_findings: List[Dict[str, Any]] = []

    # Scan Dart files for storage patterns
    all_findings.extend(scan_dart_files(project_dir, args.verbose))

    # Check Android backup config
    all_findings.extend(check_android_backup_config(project_dir))

    # Check iOS data protection
    all_findings.extend(check_ios_data_protection(project_dir))

    # Filter HIGH false-positive unless verbose
    if not args.verbose:
        all_findings = [f for f in all_findings if f.get("false_positive_risk") != "HIGH"]

    # Build summary
    severity_counts: Dict[str, int] = {"CRITICAL": 0, "HIGH": 0, "MEDIUM": 0}
    category_counts: Dict[str, int] = {}
    for f in all_findings:
        sev = f.get("severity", "MEDIUM")
        severity_counts[sev] = severity_counts.get(sev, 0) + 1
        cat = f.get("category", "other")
        category_counts[cat] = category_counts.get(cat, 0) + 1

    has_issues = any(
        f["severity"] in ("CRITICAL", "HIGH", "MEDIUM") for f in all_findings
    )

    files_scanned = 0
    for scan_dir_name in ['lib', 'test', 'integration_test', 'android', 'ios']:
        scan_dir = os.path.join(project_dir, scan_dir_name)
        if os.path.isdir(scan_dir):
            for root, _dirs, filenames in os.walk(scan_dir):
                files_scanned += sum(1 for fn in filenames if fn.endswith('.dart') or fn.endswith('.xml') or fn.endswith('.plist'))

    result = {
        "scanner_name": SCANNER_NAME,
        "version": VERSION,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "project_dir": project_dir,
        "files_scanned": files_scanned,
        "findings": all_findings,
        "summary": {
            "total_findings": len(all_findings),
            "by_severity": severity_counts,
            "by_category": category_counts,
            "status": "FAIL" if has_issues else "PASS",
        },
    }

    print(json.dumps(result, indent=2))
    return 1 if has_issues else 0


if __name__ == "__main__":
    sys.exit(main())
