#!/usr/bin/env python3
"""
scan_hardcoded_secrets.py - Scan Flutter projects for hardcoded secrets.

Detects API keys, passwords, tokens, private keys, OAuth secrets, JWT secrets,
and 20+ other secret types across lib/, android/, and ios/ directories.

Outputs structured JSON to stdout. Exit code 1 if findings, 0 if clean.
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Tuple

SCANNER_NAME = "scan_hardcoded_secrets"
VERSION = "1.0.0"

# ---------------------------------------------------------------------------
# Secret patterns: (name, regex, severity, category)
# ---------------------------------------------------------------------------
SECRET_PATTERNS: List[Tuple[str, str, str, str]] = [
    # Cloud provider keys
    ("AWS Access Key ID", r'(?:AKIA)[0-9A-Z]{16}', "CRITICAL", "cloud_credentials"),
    ("AWS Secret Access Key", r'(?i)aws[_\-]?secret[_\-]?access[_\-]?key[\s]*[=:]\s*["\']?([A-Za-z0-9/+=]{40})', "CRITICAL", "cloud_credentials"),
    ("Google API Key", r'AIza[0-9A-Za-z\-_]{35}', "HIGH", "api_key"),
    ("Google OAuth Client ID", r'[0-9]+-[a-z0-9_]{32}\.apps\.googleusercontent\.com', "HIGH", "oauth"),
    ("Google Cloud Service Account", r'"type"\s*:\s*"service_account"', "CRITICAL", "cloud_credentials"),
    ("Firebase API Key", r'(?i)firebase[_\-]?api[_\-]?key[\s]*[=:]\s*["\']([^"\']+)', "HIGH", "api_key"),

    # Payment processors
    ("Stripe Secret Key", r'sk_live_[0-9a-zA-Z]{24,}', "CRITICAL", "payment"),
    ("Stripe Publishable Key", r'pk_live_[0-9a-zA-Z]{24,}', "MEDIUM", "payment"),
    ("Stripe Test Secret Key", r'sk_test_[0-9a-zA-Z]{24,}', "MEDIUM", "payment"),

    # Auth tokens
    ("GitHub Token", r'gh[pousr]_[A-Za-z0-9_]{36,}', "CRITICAL", "token"),
    ("GitHub Personal Access Token (classic)", r'ghp_[A-Za-z0-9]{36}', "CRITICAL", "token"),
    ("Slack Token", r'xox[baprs]-[0-9A-Za-z\-]{10,}', "CRITICAL", "token"),
    ("Slack Webhook", r'https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[A-Za-z0-9]+', "HIGH", "webhook"),
    ("Discord Webhook", r'https://discord(?:app)?\.com/api/webhooks/\d+/[A-Za-z0-9_\-]+', "HIGH", "webhook"),

    # Generic secrets
    ("Private Key Block", r'-----BEGIN (?:RSA |EC |DSA )?PRIVATE KEY-----', "CRITICAL", "private_key"),
    ("JWT Secret", r'(?i)jwt[_\-]?secret[\s]*[=:]\s*["\']([^"\']{8,})', "CRITICAL", "jwt"),
    ("Bearer Token", r'(?i)bearer\s+[A-Za-z0-9\-._~+/]+=*', "HIGH", "token"),
    ("Basic Auth Header", r'(?i)basic\s+[A-Za-z0-9+/]{20,}={0,2}', "HIGH", "auth"),

    # Generic assignment patterns
    ("Hardcoded Password", r'(?i)(?:password|passwd|pwd)[\s]*[=:]\s*["\'](?![\s]*["\'])[^"\']{4,}["\']', "CRITICAL", "password"),
    ("Hardcoded Secret", r'(?i)(?:secret|secret_key|secretkey)[\s]*[=:]\s*["\'](?![\s]*["\'])[^"\']{4,}["\']', "CRITICAL", "secret"),
    ("Hardcoded API Key", r'(?i)(?:api[_\-]?key|apikey)[\s]*[=:]\s*["\'](?![\s]*["\'])[^"\']{8,}["\']', "HIGH", "api_key"),
    ("Hardcoded Token", r'(?i)(?:access[_\-]?token|auth[_\-]?token|api[_\-]?token)[\s]*[=:]\s*["\'](?![\s]*["\'])[^"\']{8,}["\']', "HIGH", "token"),
    ("Hardcoded Client Secret", r'(?i)client[_\-]?secret[\s]*[=:]\s*["\'](?![\s]*["\'])[^"\']{8,}["\']', "HIGH", "oauth"),

    # Database
    ("Database Connection String", r'(?i)(?:mongodb|postgres|mysql|redis)://[^\s"\']+:[^\s"\']+@', "CRITICAL", "database"),
    ("SQLite Password", r'(?i)pragma\s+key\s*=\s*["\'][^"\']+["\']', "HIGH", "database"),

    # Misc
    ("SendGrid API Key", r'SG\.[A-Za-z0-9_\-]{22}\.[A-Za-z0-9_\-]{43}', "CRITICAL", "api_key"),
    ("Twilio Auth Token", r'(?i)twilio[_\-]?auth[_\-]?token[\s]*[=:]\s*["\']([a-f0-9]{32})', "CRITICAL", "api_key"),
    ("Mailgun API Key", r'key-[0-9a-zA-Z]{32}', "HIGH", "api_key"),
    ("Heroku API Key", r'(?i)heroku[_\-]?api[_\-]?key[\s]*[=:]\s*["\']([0-9a-f\-]{36})', "HIGH", "api_key"),
    ("Generic Hex Secret (32+ chars)", r'(?i)(?:secret|key|token|password)[\s]*[=:]\s*["\']([0-9a-f]{32,})["\']', "MEDIUM", "generic"),
]

# ---------------------------------------------------------------------------
# False positive indicators
# ---------------------------------------------------------------------------
FALSE_POSITIVE_INDICATORS = [
    # Test / example values
    r'(?i)example',
    r'(?i)test[_\-]?key',
    r'(?i)dummy',
    r'(?i)placeholder',
    r'(?i)your[_\-]?api[_\-]?key',
    r'(?i)insert[_\-]?here',
    r'(?i)replace[_\-]?me',
    r'(?i)xxx+',
    r'(?i)todo',
    r'(?i)fixme',
    r'(?i)changeme',
    r'(?i)sample',
    # Localhost / loopback
    r'localhost',
    r'127\.0\.0\.1',
    r'10\.0\.2\.2',  # Android emulator localhost
    r'0\.0\.0\.0',
    # Empty / whitespace
    r'^[\s]*$',
    # Common non-secrets
    r'(?i)^(true|false|null|none|undefined)$',
    r'(?i)^(yes|no|on|off)$',
]

# File extensions to scan
SCAN_EXTENSIONS = {
    '.dart', '.java', '.kt', '.kts', '.swift', '.m', '.h',
    '.xml', '.plist', '.json', '.yaml', '.yml', '.properties',
    '.gradle', '.groovy', '.env', '.cfg', '.conf', '.ini', '.toml',
}

# Files to skip
SKIP_PATTERNS = [
    r'\.g\.dart$',           # Generated dart files
    r'\.freezed\.dart$',     # Freezed generated
    r'\.mocks\.dart$',       # Mock files
    r'pubspec\.lock$',       # Lock file
    r'\.flutter-plugins',    # Flutter plugins
    r'GeneratedPluginRegistrant', # Auto-generated
    r'node_modules',
    r'\.git/',
    r'build/',
    r'\.dart_tool/',
]

SCAN_DIRS = ['lib', 'android', 'ios', 'test', 'integration_test']


def should_skip_file(filepath: str) -> bool:
    """Check if a file should be skipped."""
    for pattern in SKIP_PATTERNS:
        if re.search(pattern, filepath.replace('\\', '/')):
            return True
    _, ext = os.path.splitext(filepath)
    return ext.lower() not in SCAN_EXTENSIONS


def estimate_false_positive(match_text: str, line: str, filepath: str) -> str:
    """Estimate false positive risk: HIGH, MEDIUM, LOW."""
    text_to_check = match_text + " " + line

    for fp_pattern in FALSE_POSITIVE_INDICATORS:
        if re.search(fp_pattern, text_to_check):
            return "HIGH"

    # Test directories are more likely false positives
    norm_path = filepath.replace('\\', '/')
    if '/test/' in norm_path or '/test_' in norm_path or '_test.dart' in norm_path:
        return "MEDIUM"

    # Comments are often examples
    stripped = line.strip()
    if stripped.startswith('//') or stripped.startswith('/*') or stripped.startswith('*'):
        return "MEDIUM"

    return "LOW"


def scan_file(filepath: str, verbose: bool = False) -> List[Dict[str, Any]]:
    """Scan a single file for hardcoded secrets."""
    findings: List[Dict[str, Any]] = []

    try:
        with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
            lines = f.readlines()
    except (IOError, OSError):
        return findings

    for line_num, line in enumerate(lines, start=1):
        for pattern_name, pattern_regex, severity, category in SECRET_PATTERNS:
            matches = list(re.finditer(pattern_regex, line))
            for match in matches:
                match_text = match.group(0)
                fp_risk = estimate_false_positive(match_text, line, filepath)

                # Skip findings with HIGH false positive risk unless verbose
                if fp_risk == "HIGH" and not verbose:
                    continue

                # Redact the actual secret in output
                redacted = match_text[:8] + "..." if len(match_text) > 11 else match_text[:4] + "..."

                findings.append({
                    "file": filepath,
                    "line": line_num,
                    "severity": severity,
                    "category": category,
                    "description": f"{pattern_name} detected: {redacted}",
                    "false_positive_risk": fp_risk,
                    "pattern_name": pattern_name,
                    "line_content": line.strip()[:120],
                })

    return findings


def collect_files(project_dir: str) -> List[str]:
    """Collect all files to scan from target directories."""
    files: List[str] = []

    for scan_dir_name in SCAN_DIRS:
        scan_dir = os.path.join(project_dir, scan_dir_name)
        if not os.path.isdir(scan_dir):
            continue
        for root, _dirs, filenames in os.walk(scan_dir):
            for fname in filenames:
                full_path = os.path.join(root, fname)
                if not should_skip_file(full_path):
                    files.append(full_path)

    # Also scan root-level config files
    for fname in os.listdir(project_dir):
        full_path = os.path.join(project_dir, fname)
        if os.path.isfile(full_path):
            _, ext = os.path.splitext(fname)
            if ext.lower() in {'.env', '.yaml', '.yml', '.json', '.properties', '.toml'}:
                if not should_skip_file(full_path):
                    files.append(full_path)

    return files


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Scan a Flutter project for hardcoded secrets, API keys, tokens, and credentials.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python scan_hardcoded_secrets.py --project-dir /path/to/flutter/app\n"
            "  python scan_hardcoded_secrets.py --verbose\n"
            "  python scan_hardcoded_secrets.py --project-dir . | jq '.findings[]'\n"
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

    files = collect_files(project_dir)
    all_findings: List[Dict[str, Any]] = []

    for filepath in files:
        file_findings = scan_file(filepath, verbose=args.verbose)
        all_findings.extend(file_findings)

    # Build summary
    severity_counts: Dict[str, int] = {"CRITICAL": 0, "HIGH": 0, "MEDIUM": 0}
    category_counts: Dict[str, int] = {}
    for f in all_findings:
        severity_counts[f["severity"]] = severity_counts.get(f["severity"], 0) + 1
        category_counts[f["category"]] = category_counts.get(f["category"], 0) + 1

    result = {
        "scanner_name": SCANNER_NAME,
        "version": VERSION,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "project_dir": project_dir,
        "files_scanned": len(files),
        "findings": all_findings,
        "summary": {
            "total_findings": len(all_findings),
            "by_severity": severity_counts,
            "by_category": category_counts,
            "status": "FAIL" if all_findings else "PASS",
        },
    }

    print(json.dumps(result, indent=2))
    return 1 if all_findings else 0


if __name__ == "__main__":
    sys.exit(main())
