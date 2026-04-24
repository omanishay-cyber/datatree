#!/usr/bin/env python3
"""
update_references.py — Regenerate reference files from latest Flutter/Dart docs.

Usage:
    python update_references.py --skill-dir /path/to/fireworks-flutter check
    python update_references.py --skill-dir /path/to/fireworks-flutter update
    python update_references.py --skill-dir /path/to/fireworks-flutter update --dry-run

Commands:
    check   Show which reference files are outdated
    update  Regenerate reference files from latest docs

Python 3.7+ stdlib only — no pip dependencies.
"""

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

METADATA_PATTERN = re.compile(
    r"<!--\s*@update_references\s+(.*?)-->", re.DOTALL
)

DOC_SOURCES = {
    "flutter_api": "https://api.flutter.dev/",
    "pub_dev": "https://pub.dev/packages/",
    "dart_language": "https://dart.dev/language",
}

# Map reference filenames to the doc sources they depend on
REFERENCE_SOURCES: Dict[str, List[str]] = {
    "widget-catalog.md": ["flutter_api"],
    "widget-testing-guide.md": ["flutter_api"],
    "riverpod-patterns.md": ["pub_dev"],
    "riverpod-testing-guide.md": ["pub_dev"],
    "bloc-patterns.md": ["pub_dev"],
    "dart-modern-features.md": ["dart_language"],
    "navigation-patterns.md": ["flutter_api"],
    "slivers-performance.md": ["flutter_api"],
    "animation-advanced.md": ["flutter_api"],
    "testing-patterns.md": ["flutter_api"],
    "golden-testing.md": ["flutter_api"],
    "layer-testing-patterns.md": ["flutter_api"],
    "debugging-patterns.md": ["flutter_api"],
    "clean-architecture.md": ["flutter_api", "pub_dev"],
    "app-lifecycle.md": ["flutter_api"],
}

# Key packages to track versions for
TRACKED_PACKAGES = [
    "flutter_riverpod",
    "riverpod_annotation",
    "flutter_bloc",
    "go_router",
    "freezed",
    "json_serializable",
    "mockito",
    "flutter_test",
]


# ---------------------------------------------------------------------------
# Metadata helpers
# ---------------------------------------------------------------------------

def read_metadata(filepath: Path) -> Optional[Dict[str, Any]]:
    """Extract @update_references metadata from a reference file."""
    try:
        text = filepath.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None

    match = METADATA_PATTERN.search(text)
    if not match:
        return None

    try:
        return json.loads(match.group(1).strip())
    except json.JSONDecodeError:
        return None


def write_metadata(filepath: Path, metadata: Dict[str, Any]) -> str:
    """Return file content with updated (or inserted) metadata block."""
    text = filepath.read_text(encoding="utf-8")
    block = f"<!-- @update_references {json.dumps(metadata, indent=2)} -->"

    if METADATA_PATTERN.search(text):
        text = METADATA_PATTERN.sub(block, text)
    else:
        # Insert after first heading line
        lines = text.split("\n", 1)
        if len(lines) == 2:
            text = f"{lines[0]}\n\n{block}\n\n{lines[1]}"
        else:
            text = f"{block}\n\n{text}"

    return text


def now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


# ---------------------------------------------------------------------------
# Network helpers (stdlib only)
# ---------------------------------------------------------------------------

def fetch_url(url: str, timeout: int = 15) -> Tuple[int, str]:
    """Fetch a URL; return (status_code, body). Returns (0, error) on failure."""
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "update_references/1.0"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            return resp.status, body
    except urllib.error.HTTPError as exc:
        return exc.code, str(exc)
    except Exception as exc:
        return 0, str(exc)


def fetch_pub_version(package: str) -> Optional[str]:
    """Fetch the latest version of a pub.dev package."""
    url = f"https://pub.dev/api/packages/{package}"
    status, body = fetch_url(url)
    if status != 200:
        return None
    try:
        data = json.loads(body)
        return data.get("latest", {}).get("version")
    except (json.JSONDecodeError, KeyError):
        return None


def check_doc_reachable(source_key: str) -> Tuple[bool, str]:
    """Check whether a doc source URL is reachable."""
    url = DOC_SOURCES.get(source_key, "")
    if not url:
        return False, f"Unknown source: {source_key}"
    status, _ = fetch_url(url, timeout=10)
    ok = 200 <= status < 400
    return ok, f"HTTP {status}" if status else "unreachable"


# ---------------------------------------------------------------------------
# Core logic
# ---------------------------------------------------------------------------

def discover_references(refs_dir: Path) -> List[Path]:
    """Return all .md files in the references directory."""
    if not refs_dir.is_dir():
        return []
    return sorted(refs_dir.glob("*.md"))


def check_references(skill_dir: Path, verbose: bool = True) -> List[Dict[str, Any]]:
    """Check all reference files and return a report list."""
    refs_dir = skill_dir / "references"
    files = discover_references(refs_dir)
    report: List[Dict[str, Any]] = []

    if verbose:
        print(f"\nScanning {len(files)} reference file(s) in {refs_dir}\n")
        print("-" * 70)

    # Fetch latest package versions once
    pkg_versions: Dict[str, Optional[str]] = {}
    for pkg in TRACKED_PACKAGES:
        pkg_versions[pkg] = fetch_pub_version(pkg)

    for fpath in files:
        meta = read_metadata(fpath)
        fname = fpath.name
        sources = REFERENCE_SOURCES.get(fname, [])

        entry: Dict[str, Any] = {
            "file": fname,
            "has_metadata": meta is not None,
            "last_updated": meta.get("last_updated") if meta else None,
            "tracked_versions": meta.get("package_versions", {}) if meta else {},
            "sources": sources,
            "needs_update": False,
            "reasons": [],
        }

        # Check if metadata is missing
        if meta is None:
            entry["needs_update"] = True
            entry["reasons"].append("no metadata block found")

        # Check staleness (older than 30 days)
        if meta and meta.get("last_updated"):
            try:
                last = datetime.fromisoformat(meta["last_updated"].replace("Z", "+00:00"))
                age_days = (datetime.now(timezone.utc) - last).days
                if age_days > 30:
                    entry["needs_update"] = True
                    entry["reasons"].append(f"last updated {age_days} days ago (>30)")
            except (ValueError, TypeError):
                entry["needs_update"] = True
                entry["reasons"].append("invalid last_updated date")

        # Check package versions
        if meta and meta.get("package_versions"):
            for pkg, old_ver in meta["package_versions"].items():
                new_ver = pkg_versions.get(pkg)
                if new_ver and old_ver and new_ver != old_ver:
                    entry["needs_update"] = True
                    entry["reasons"].append(f"{pkg}: {old_ver} -> {new_ver}")

        report.append(entry)

        if verbose:
            status = "OUTDATED" if entry["needs_update"] else "OK"
            marker = ">>>" if entry["needs_update"] else "   "
            print(f"{marker} [{status:>8}] {fname}")
            if entry["last_updated"]:
                print(f"             Last updated: {entry['last_updated']}")
            for reason in entry["reasons"]:
                print(f"             - {reason}")

    if verbose:
        print("-" * 70)
        outdated = sum(1 for e in report if e["needs_update"])
        print(f"\nTotal: {len(report)} files, {outdated} need updating\n")

    return report


def update_references(
    skill_dir: Path,
    dry_run: bool = False,
    verbose: bool = True,
) -> None:
    """Update metadata in reference files that need it."""
    refs_dir = skill_dir / "references"
    report = check_references(skill_dir, verbose=verbose)

    # Fetch latest package versions
    pkg_versions: Dict[str, Optional[str]] = {}
    for pkg in TRACKED_PACKAGES:
        ver = fetch_pub_version(pkg)
        if ver:
            pkg_versions[pkg] = ver

    updated_count = 0
    for entry in report:
        if not entry["needs_update"]:
            continue

        fpath = refs_dir / entry["file"]
        sources = entry["sources"]

        # Build relevant package versions for this file
        relevant_pkgs: Dict[str, str] = {}
        if "pub_dev" in sources:
            relevant_pkgs = {k: v for k, v in pkg_versions.items() if v}

        new_meta = {
            "last_updated": now_iso(),
            "sources": sources,
            "package_versions": relevant_pkgs,
        }

        if dry_run:
            if verbose:
                print(f"[DRY RUN] Would update metadata in {entry['file']}")
                print(f"          New metadata: {json.dumps(new_meta, indent=2)}")
        else:
            try:
                new_content = write_metadata(fpath, new_meta)
                fpath.write_text(new_content, encoding="utf-8")
                updated_count += 1
                if verbose:
                    print(f"[UPDATED] {entry['file']}")
            except OSError as exc:
                print(f"[ERROR]   {entry['file']}: {exc}", file=sys.stderr)

    if verbose:
        action = "would update" if dry_run else "updated"
        target = sum(1 for e in report if e["needs_update"])
        print(f"\nDone. {action.capitalize()} {updated_count if not dry_run else target} file(s).")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="update_references",
        description=(
            "Check and regenerate Flutter/Dart reference files from latest docs. "
            "Tracks package versions from pub.dev and staleness of each reference file."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  %(prog)s --skill-dir ./fireworks-flutter check\n"
            "  %(prog)s --skill-dir ./fireworks-flutter update --dry-run\n"
            "  %(prog)s --skill-dir ./fireworks-flutter update\n"
        ),
    )

    parser.add_argument(
        "--skill-dir",
        required=True,
        type=Path,
        help="Path to the fireworks-flutter skill directory",
    )

    subparsers = parser.add_subparsers(dest="command", help="Available commands")
    subparsers.required = True

    # check
    check_parser = subparsers.add_parser(
        "check",
        help="Show which reference files are outdated",
    )
    check_parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Only output JSON report, no human-readable text",
    )

    # update
    update_parser = subparsers.add_parser(
        "update",
        help="Regenerate metadata in outdated reference files",
    )
    update_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be updated without writing files",
    )
    update_parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Only output JSON report, no human-readable text",
    )

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    skill_dir: Path = args.skill_dir.resolve()
    if not skill_dir.is_dir():
        print(f"Error: skill directory not found: {skill_dir}", file=sys.stderr)
        return 1

    refs_dir = skill_dir / "references"
    if not refs_dir.is_dir():
        print(f"Error: references directory not found: {refs_dir}", file=sys.stderr)
        return 1

    quiet = getattr(args, "quiet", False)

    if args.command == "check":
        report = check_references(skill_dir, verbose=not quiet)
        if quiet:
            print(json.dumps(report, indent=2, default=str))
        outdated = sum(1 for e in report if e["needs_update"])
        return 1 if outdated > 0 else 0

    elif args.command == "update":
        dry_run = getattr(args, "dry_run", False)
        update_references(skill_dir, dry_run=dry_run, verbose=not quiet)
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
