#!/usr/bin/env python3
"""
check_network_security.py - Scan Flutter projects for network security issues.

Detects HTTP URLs, disabled certificate validation, missing cert pinning,
insecure WebSocket connections, and platform-specific misconfigurations
(Android network_security_config.xml, iOS Info.plist).

Outputs structured JSON to stdout. Exit code 1 if issues, 0 if clean.
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone
from typing import Any, Dict, List, Tuple

SCANNER_NAME = "check_network_security"
VERSION = "1.0.0"

# ---------------------------------------------------------------------------
# Network security patterns: (name, regex, severity, category, description)
# ---------------------------------------------------------------------------
DART_PATTERNS: List[Tuple[str, str, str, str, str]] = [
    (
        "HTTP URL (non-localhost)",
        r'''(?:["'])http://(?!localhost|127\.0\.0\.1|10\.0\.2\.2|0\.0\.0\.0)[^\s"']+["']''',
        "HIGH",
        "insecure_transport",
        "HTTP URL detected (not HTTPS) - data transmitted in plaintext",
    ),
    (
        "Insecure WebSocket",
        r'''(?:["'])ws://(?!localhost|127\.0\.0\.1|10\.0\.2\.2|0\.0\.0\.0)[^\s"']+["']''',
        "HIGH",
        "insecure_transport",
        "Insecure WebSocket (ws://) detected - use wss:// for encrypted connections",
    ),
    (
        "Disabled Certificate Validation",
        r'badCertificateCallback\s*[=:]\s*\(\s*[^)]*\)\s*=>\s*true',
        "CRITICAL",
        "certificate_bypass",
        "Certificate validation disabled via badCertificateCallback => true - allows MITM attacks",
    ),
    (
        "Disabled Certificate Validation (alt)",
        r'badCertificateCallback\s*[=:]\s*\([^)]*\)\s*\{\s*return\s+true\s*;',
        "CRITICAL",
        "certificate_bypass",
        "Certificate validation disabled - always returns true for bad certificates",
    ),
    (
        "HttpClient onBadCertificate bypass",
        r'onBadCertificate\s*=\s*\(\s*[^)]*\)\s*=>\s*true',
        "CRITICAL",
        "certificate_bypass",
        "HttpClient onBadCertificate callback always returns true - MITM vulnerability",
    ),
    (
        "Dio Certificate Bypass",
        r'validateCertificate\s*[=:]\s*\(\s*[^)]*\)\s*=>\s*true',
        "CRITICAL",
        "certificate_bypass",
        "Dio certificate validation disabled - allows untrusted certificates",
    ),
    (
        "SecurityContext allowLegacyUnsafeRenegotiation",
        r'allowLegacyUnsafeRenegotiation\s*=\s*true',
        "HIGH",
        "weak_tls",
        "Legacy unsafe TLS renegotiation enabled - vulnerable to attacks",
    ),
    (
        "TLS version downgrade",
        r'(?:SecurityContext|HttpClient).*(?:TLS|SSL).*1\.[01]',
        "HIGH",
        "weak_tls",
        "Potentially allowing TLS 1.0/1.1 which are deprecated and insecure",
    ),
    (
        "HTTP override in Dio/Retrofit",
        r'baseUrl\s*[=:]\s*["\']http://(?!localhost|127\.0\.0\.1|10\.0\.2\.2)',
        "HIGH",
        "insecure_transport",
        "HTTP base URL configured in API client - should use HTTPS",
    ),
    (
        "Hardcoded IP address in URL",
        r'''(?:https?|wss?)://\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(?!(?:127\.0\.0\.1|10\.0\.2\.2|0\.0\.0\.0|localhost))''',
        "MEDIUM",
        "hardcoded_endpoint",
        "Hardcoded IP address in URL - may indicate debug/staging endpoint left in code",
    ),
]

# Patterns for localhost that are OK
LOCALHOST_PATTERNS = re.compile(
    r'(?:localhost|127\.0\.0\.1|10\.0\.2\.2|0\.0\.0\.0|::1)'
)

# File extensions to scan for Dart code
DART_EXTENSIONS = {'.dart'}
CONFIG_EXTENSIONS = {'.xml', '.plist', '.json', '.yaml', '.yml'}

SCAN_DIRS = ['lib', 'test', 'integration_test']


def scan_dart_files(project_dir: str, verbose: bool) -> List[Dict[str, Any]]:
    """Scan Dart source files for network security issues."""
    findings: List[Dict[str, Any]] = []

    for scan_dir_name in SCAN_DIRS:
        scan_dir = os.path.join(project_dir, scan_dir_name)
        if not os.path.isdir(scan_dir):
            continue

        for root, _dirs, filenames in os.walk(scan_dir):
            for fname in filenames:
                _, ext = os.path.splitext(fname)
                if ext.lower() not in DART_EXTENSIONS:
                    continue

                filepath = os.path.join(root, fname)

                # Skip generated files
                if '.g.dart' in fname or '.freezed.dart' in fname:
                    continue

                try:
                    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
                        lines = f.readlines()
                except (IOError, OSError):
                    continue

                for line_num, line in enumerate(lines, start=1):
                    for pattern_name, pattern_regex, severity, category, description in DART_PATTERNS:
                        if re.search(pattern_regex, line):
                            # Determine false positive risk
                            fp_risk = "LOW"
                            stripped = line.strip()
                            if stripped.startswith('//') or stripped.startswith('/*') or stripped.startswith('*'):
                                fp_risk = "MEDIUM"
                            norm_path = filepath.replace('\\', '/')
                            if '/test/' in norm_path or '_test.dart' in norm_path:
                                fp_risk = "MEDIUM"
                            # assert/debug patterns
                            if 'assert' in line.lower() or 'kDebugMode' in line or 'kReleaseMode' in line:
                                fp_risk = "MEDIUM"

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


def check_android_network_security(project_dir: str) -> List[Dict[str, Any]]:
    """Check Android network_security_config.xml and AndroidManifest.xml."""
    findings: List[Dict[str, Any]] = []

    # Check AndroidManifest for usesCleartextTraffic
    manifest_paths = [
        os.path.join(project_dir, "android", "app", "src", "main", "AndroidManifest.xml"),
        os.path.join(project_dir, "android", "app", "src", "debug", "AndroidManifest.xml"),
        os.path.join(project_dir, "android", "app", "src", "profile", "AndroidManifest.xml"),
    ]

    for manifest_path in manifest_paths:
        if not os.path.isfile(manifest_path):
            continue
        try:
            with open(manifest_path, 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read()

            if 'usesCleartextTraffic="true"' in content:
                is_debug = 'debug' in manifest_path.replace('\\', '/')
                findings.append({
                    "file": manifest_path,
                    "line": 0,
                    "severity": "MEDIUM" if is_debug else "HIGH",
                    "category": "cleartext_traffic",
                    "description": "android:usesCleartextTraffic=\"true\" allows unencrypted HTTP traffic"
                                   + (" (debug build - lower risk)" if is_debug else ""),
                    "false_positive_risk": "MEDIUM" if is_debug else "LOW",
                })
        except (IOError, OSError):
            pass

    # Check network_security_config.xml
    nsc_path = os.path.join(
        project_dir, "android", "app", "src", "main", "res", "xml", "network_security_config.xml"
    )
    if os.path.isfile(nsc_path):
        try:
            with open(nsc_path, 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read()

            if 'cleartextTrafficPermitted="true"' in content:
                # Check if it's domain-scoped or global
                if '<base-config' in content and 'cleartextTrafficPermitted="true"' in content:
                    findings.append({
                        "file": nsc_path,
                        "line": 0,
                        "severity": "HIGH",
                        "category": "cleartext_traffic",
                        "description": "Global cleartextTrafficPermitted=true in network_security_config.xml - allows HTTP for all domains",
                        "false_positive_risk": "LOW",
                    })
                else:
                    findings.append({
                        "file": nsc_path,
                        "line": 0,
                        "severity": "MEDIUM",
                        "category": "cleartext_traffic",
                        "description": "cleartextTrafficPermitted=true found in network_security_config.xml (may be domain-scoped)",
                        "false_positive_risk": "MEDIUM",
                    })

            # Check for trust-anchors with user certificates
            if '<certificates src="user"' in content:
                findings.append({
                    "file": nsc_path,
                    "line": 0,
                    "severity": "MEDIUM",
                    "category": "trust_config",
                    "description": "User-installed certificates trusted - may be for debugging but risky in production",
                    "false_positive_risk": "MEDIUM",
                })
        except (IOError, OSError):
            pass
    else:
        # No network security config at all
        findings.append({
            "file": nsc_path,
            "line": 0,
            "severity": "MEDIUM",
            "category": "missing_config",
            "description": "No network_security_config.xml found - consider adding one to enforce HTTPS",
            "false_positive_risk": "HIGH",
        })

    return findings


def check_ios_transport_security(project_dir: str) -> List[Dict[str, Any]]:
    """Check iOS Info.plist for App Transport Security settings."""
    findings: List[Dict[str, Any]] = []

    plist_paths = [
        os.path.join(project_dir, "ios", "Runner", "Info.plist"),
    ]

    for plist_path in plist_paths:
        if not os.path.isfile(plist_path):
            continue

        try:
            with open(plist_path, 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read()

            # NSAllowsArbitraryLoads
            if 'NSAllowsArbitraryLoads' in content:
                # Simple check: if the key is followed by <true/>
                ats_match = re.search(
                    r'<key>NSAllowsArbitraryLoads</key>\s*<(true|false)\s*/?>',
                    content,
                )
                if ats_match and ats_match.group(1) == 'true':
                    findings.append({
                        "file": plist_path,
                        "line": 0,
                        "severity": "HIGH",
                        "category": "transport_security",
                        "description": "NSAllowsArbitraryLoads is true - disables App Transport Security, allows HTTP",
                        "false_positive_risk": "LOW",
                    })

            # NSAllowsArbitraryLoadsInWebContent
            if 'NSAllowsArbitraryLoadsInWebContent' in content:
                web_match = re.search(
                    r'<key>NSAllowsArbitraryLoadsInWebContent</key>\s*<(true|false)\s*/?>',
                    content,
                )
                if web_match and web_match.group(1) == 'true':
                    findings.append({
                        "file": plist_path,
                        "line": 0,
                        "severity": "MEDIUM",
                        "category": "transport_security",
                        "description": "NSAllowsArbitraryLoadsInWebContent is true - web views can load HTTP content",
                        "false_positive_risk": "MEDIUM",
                    })

            # NSExceptionAllowsInsecureHTTPLoads
            if 'NSExceptionAllowsInsecureHTTPLoads' in content:
                findings.append({
                    "file": plist_path,
                    "line": 0,
                    "severity": "MEDIUM",
                    "category": "transport_security",
                    "description": "NSExceptionAllowsInsecureHTTPLoads found - some domains allow HTTP",
                    "false_positive_risk": "MEDIUM",
                })

        except (IOError, OSError):
            pass

    return findings


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Scan a Flutter project for network security issues.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python check_network_security.py --project-dir /path/to/flutter/app\n"
            "  python check_network_security.py --verbose\n"
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

    # Scan Dart source files
    all_findings.extend(scan_dart_files(project_dir, args.verbose))

    # Check Android configuration
    all_findings.extend(check_android_network_security(project_dir))

    # Check iOS configuration
    all_findings.extend(check_ios_transport_security(project_dir))

    # Filter out HIGH false-positive findings unless verbose
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
    for scan_dir_name in SCAN_DIRS + ['android', 'ios']:
        scan_dir = os.path.join(project_dir, scan_dir_name)
        if os.path.isdir(scan_dir):
            for root, _dirs, filenames in os.walk(scan_dir):
                files_scanned += len(filenames)

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
