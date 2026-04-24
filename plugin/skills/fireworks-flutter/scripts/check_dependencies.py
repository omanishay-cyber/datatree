#!/usr/bin/env python3
"""
check_dependencies.py - Check pubspec.yaml for dependency security issues.

Detects 'any' version constraints, deprecated/known-vulnerable packages,
unpinned versions, and optionally runs `flutter pub outdated --json`.

Outputs structured JSON to stdout. Exit code 1 if issues, 0 if clean.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

SCANNER_NAME = "check_dependencies"
VERSION = "1.0.0"

# ---------------------------------------------------------------------------
# Known vulnerable / deprecated packages (maintained list)
# ---------------------------------------------------------------------------
KNOWN_VULNERABLE: Dict[str, Dict[str, str]] = {
    "http": {
        "note": "Versions <0.13.0 have security issues; prefer latest or use dio",
        "severity": "MEDIUM",
    },
    "crypto": {
        "note": "Older versions had weak algorithm defaults; ensure latest",
        "severity": "MEDIUM",
    },
    "jaguar_jwt": {
        "note": "Unmaintained; consider dart_jsonwebtoken or jose",
        "severity": "HIGH",
    },
    "flutter_webview_plugin": {
        "note": "Deprecated in favor of webview_flutter; has known security issues",
        "severity": "HIGH",
    },
    "webview_flutter": {
        "note": "Versions <4.0.0 have XSS vulnerabilities; upgrade to latest",
        "severity": "MEDIUM",
    },
    "firebase_dynamic_links": {
        "note": "Firebase Dynamic Links is deprecated as of August 2025",
        "severity": "MEDIUM",
    },
    "sqflite": {
        "note": "Does not encrypt data at rest; consider sqflite_sqlcipher for sensitive data",
        "severity": "MEDIUM",
    },
    "shared_preferences": {
        "note": "Stores data in plaintext; do not store secrets here",
        "severity": "MEDIUM",
    },
    "url_launcher": {
        "note": "Versions <6.1.0 on Android could be exploited via intent injection",
        "severity": "MEDIUM",
    },
    "path_provider": {
        "note": "Ensure external storage paths are not used for sensitive data",
        "severity": "LOW",
    },
    "dio": {
        "note": "Versions <5.0.0 had certificate validation bypass potential; upgrade",
        "severity": "MEDIUM",
    },
    "get": {
        "note": "GetX has had community governance concerns; evaluate alternatives",
        "severity": "LOW",
    },
    "flutter_html": {
        "note": "Renders raw HTML - XSS risk if content is user-supplied",
        "severity": "HIGH",
    },
    "html": {
        "note": "Parsing untrusted HTML can lead to XSS; sanitize output",
        "severity": "MEDIUM",
    },
}

# Packages that should never be in production
DEV_ONLY_PACKAGES = {
    "flutter_test",
    "build_runner",
    "json_serializable",
    "freezed",
    "mockito",
    "fake_async",
    "test",
    "integration_test",
    "flutter_lints",
    "very_good_analysis",
}


def parse_pubspec_yaml(filepath: str) -> Optional[Dict[str, Any]]:
    """
    Minimal YAML parser for pubspec.yaml - no external deps needed.
    Handles the common flat key:value and nested dependency structures.
    """
    if not os.path.isfile(filepath):
        return None

    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    result: Dict[str, Any] = {}
    current_section: Optional[str] = None
    current_deps: Dict[str, str] = {}

    for line in content.split('\n'):
        # Skip comments and empty lines
        stripped = line.strip()
        if not stripped or stripped.startswith('#'):
            continue

        # Top-level key (no indentation)
        if not line.startswith(' ') and not line.startswith('\t'):
            if current_section and current_deps:
                result[current_section] = dict(current_deps)
                current_deps = {}

            match = re.match(r'^(\w[\w\-]*):\s*(.*)', line)
            if match:
                key = match.group(1)
                value = match.group(2).strip()
                if key in ('dependencies', 'dev_dependencies', 'dependency_overrides'):
                    current_section = key
                    current_deps = {}
                else:
                    current_section = None
                    if value:
                        result[key] = value
            continue

        # Indented line under a section
        if current_section:
            # Package with inline version: "  http: ^0.13.0"
            dep_match = re.match(r'^  (\w[\w\-]*):\s*(.*)', line)
            if dep_match:
                pkg = dep_match.group(1)
                ver = dep_match.group(2).strip()
                # Could be a nested block (git:, path:, hosted:)
                if ver in ('', None):
                    ver = "<complex>"
                current_deps[pkg] = ver
            # Sub-key under a package (git, path, version, etc.)
            elif re.match(r'^    ', line):
                sub_match = re.match(r'^\s+version:\s*(.*)', line)
                if sub_match and current_deps:
                    last_pkg = list(current_deps.keys())[-1]
                    current_deps[last_pkg] = sub_match.group(1).strip()

    if current_section and current_deps:
        result[current_section] = dict(current_deps)

    return result


def check_version_constraint(pkg: str, version: str) -> List[Dict[str, Any]]:
    """Check a single dependency version constraint for issues."""
    issues: List[Dict[str, Any]] = []

    # 'any' constraint
    if version.lower() in ('any', '"any"', "'any'"):
        issues.append({
            "severity": "CRITICAL",
            "category": "unpinned_version",
            "description": f"Package '{pkg}' uses 'any' version constraint - allows ANY version including breaking/vulnerable ones",
        })

    # No version specified at all
    elif version == "<complex>":
        # Git/path dependencies - flag but lower severity
        issues.append({
            "severity": "MEDIUM",
            "category": "complex_dependency",
            "description": f"Package '{pkg}' uses git/path/hosted dependency - version not pinned in pubspec",
        })

    # Completely unpinned (just package name, no version)
    elif not version or version == '""' or version == "''":
        issues.append({
            "severity": "HIGH",
            "category": "unpinned_version",
            "description": f"Package '{pkg}' has no version constraint specified",
        })

    # Very loose constraint like ">0.0.0" or ">=0.0.1"
    elif re.match(r'^[>]=?\s*0\.0\.\d+$', version):
        issues.append({
            "severity": "HIGH",
            "category": "loose_version",
            "description": f"Package '{pkg}' has an extremely loose version constraint: {version}",
        })

    return issues


def run_flutter_pub_outdated(project_dir: str) -> Optional[Dict[str, Any]]:
    """Try to run flutter pub outdated --json and return parsed results."""
    # Detect FVM
    fvm_path = shutil.which('fvm')
    flutter_cmd = ['fvm', 'flutter'] if fvm_path else ['flutter']

    flutter_path = shutil.which(flutter_cmd[0])
    if not flutter_path:
        return None

    try:
        result = subprocess.run(
            flutter_cmd + ['pub', 'outdated', '--json'],
            cwd=project_dir,
            capture_output=True,
            text=True,
            timeout=120,
        )
        if result.returncode == 0 and result.stdout.strip():
            return json.loads(result.stdout)
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, json.JSONDecodeError, FileNotFoundError):
        pass

    return None


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check Flutter pubspec.yaml for dependency security issues.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python check_dependencies.py --project-dir /path/to/flutter/app\n"
            "  python check_dependencies.py --verbose\n"
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
        help="Include LOW severity findings and detailed outdated package info",
    )
    args = parser.parse_args()

    project_dir = os.path.abspath(args.project_dir)
    pubspec_path = os.path.join(project_dir, "pubspec.yaml")

    if not os.path.isfile(pubspec_path):
        result = {
            "scanner_name": SCANNER_NAME,
            "version": VERSION,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "project_dir": project_dir,
            "error": "pubspec.yaml not found",
            "findings": [],
            "summary": {"total_findings": 0, "status": "ERROR"},
        }
        print(json.dumps(result, indent=2))
        return 1

    pubspec = parse_pubspec_yaml(pubspec_path)
    if pubspec is None:
        print(json.dumps({"error": "Failed to parse pubspec.yaml"}), file=sys.stderr)
        return 1

    findings: List[Dict[str, Any]] = []
    deps = pubspec.get("dependencies", {})
    dev_deps = pubspec.get("dev_dependencies", {})

    # ---- Check production dependencies ----
    for pkg, version in deps.items():
        if isinstance(version, dict):
            version = "<complex>"

        # Version constraint issues
        for issue in check_version_constraint(pkg, str(version)):
            findings.append({
                "file": pubspec_path,
                "line": 0,
                "false_positive_risk": "LOW",
                **issue,
            })

        # Known vulnerable packages
        if pkg in KNOWN_VULNERABLE:
            info = KNOWN_VULNERABLE[pkg]
            if info["severity"] != "LOW" or args.verbose:
                findings.append({
                    "file": pubspec_path,
                    "line": 0,
                    "severity": info["severity"],
                    "category": "known_vulnerable",
                    "description": f"Package '{pkg}': {info['note']}",
                    "false_positive_risk": "MEDIUM",
                })

        # Dev-only packages in production dependencies
        if pkg in DEV_ONLY_PACKAGES:
            findings.append({
                "file": pubspec_path,
                "line": 0,
                "severity": "MEDIUM",
                "category": "dev_in_production",
                "description": f"Package '{pkg}' is typically dev-only but listed under dependencies (not dev_dependencies)",
                "false_positive_risk": "LOW",
            })

    # ---- Check dev dependencies for version issues too ----
    for pkg, version in dev_deps.items():
        if isinstance(version, dict):
            version = "<complex>"
        ver_str = str(version)
        if ver_str.lower() in ('any', '"any"', "'any'"):
            findings.append({
                "file": pubspec_path,
                "line": 0,
                "severity": "HIGH",
                "category": "unpinned_version",
                "description": f"Dev package '{pkg}' uses 'any' version constraint",
                "false_positive_risk": "LOW",
            })

    # ---- Try flutter pub outdated ----
    outdated_data = run_flutter_pub_outdated(project_dir)
    outdated_count = 0
    if outdated_data and "packages" in outdated_data:
        for pkg_info in outdated_data["packages"]:
            pkg_name = pkg_info.get("package", "unknown")
            current = pkg_info.get("current", {}).get("version", "?")
            resolvable = pkg_info.get("resolvable", {}).get("version", "?")
            latest = pkg_info.get("latest", {}).get("version", "?")

            if current != latest:
                outdated_count += 1
                severity = "MEDIUM" if current != resolvable else "LOW"
                if severity != "LOW" or args.verbose:
                    findings.append({
                        "file": pubspec_path,
                        "line": 0,
                        "severity": severity,
                        "category": "outdated_package",
                        "description": f"Package '{pkg_name}' is outdated: current={current}, resolvable={resolvable}, latest={latest}",
                        "false_positive_risk": "MEDIUM",
                    })

    # ---- Build summary ----
    severity_counts: Dict[str, int] = {"CRITICAL": 0, "HIGH": 0, "MEDIUM": 0, "LOW": 0}
    category_counts: Dict[str, int] = {}
    for f in findings:
        sev = f.get("severity", "MEDIUM")
        severity_counts[sev] = severity_counts.get(sev, 0) + 1
        cat = f.get("category", "other")
        category_counts[cat] = category_counts.get(cat, 0) + 1

    has_issues = any(
        f["severity"] in ("CRITICAL", "HIGH", "MEDIUM") for f in findings
    )

    result = {
        "scanner_name": SCANNER_NAME,
        "version": VERSION,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "project_dir": project_dir,
        "total_dependencies": len(deps),
        "total_dev_dependencies": len(dev_deps),
        "flutter_pub_outdated_available": outdated_data is not None,
        "outdated_count": outdated_count,
        "findings": findings,
        "summary": {
            "total_findings": len(findings),
            "by_severity": severity_counts,
            "by_category": category_counts,
            "status": "FAIL" if has_issues else "PASS",
        },
    }

    print(json.dumps(result, indent=2))
    return 1 if has_issues else 0


if __name__ == "__main__":
    sys.exit(main())
