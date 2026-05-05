#!/usr/bin/env bash
# bump-version.sh — single-source-of-truth version bumper for the mneme
# workspace.
#
# WHY THIS EXISTS
# ---------------
# The mneme repo ships under one logical version but stores it in ~13
# different places: Cargo.toml workspace, pyproject.toml, package.json,
# plugin.json, marketplace.json, tauri.conf.json, the multi-arch-release
# workflow's hardcoded artifact filenames, etc. Bumping by hand is error-
# prone (see the v0.4.0 ship attempt where every binary still reported
# 0.3.2 in `--version` because we only updated the CHANGELOG).
#
# This script is the canonical bumper. When you ADD a new file that
# encodes the version, ADD IT HERE in TARGET_FILES below. The script is
# the index — if it isn't here, it isn't getting bumped on the next ship.
#
# USAGE
# -----
#   ./scripts/bump-version.sh <from> <to>           # apply
#   ./scripts/bump-version.sh <from> <to> --dry-run # preview only
#   ./scripts/bump-version.sh 0.3.2 0.4.0
#
# After running:
#   1. Inspect `git diff` to make sure no historical doc/comment got hit
#   2. Run pre-push gates (cargo fmt + check, bun tsc, etc.)
#   3. Commit with a clear "chore(release): bump <from> -> <to>" message
#   4. Tag: git tag v<to>
#   5. Push: git push && git push --tags
#   6. Create GitHub release: gh release create v<to> --generate-notes
#   7. Re-trigger workflow: gh workflow run multi-arch-release.yml -f tag=v<to>
#
# Authors: Anish Trivedi & Kruti Trivedi. Apache-2.0.

set -euo pipefail

# ---------------------------------------------------------------------------
# Args
# ---------------------------------------------------------------------------
if [[ $# -lt 2 ]]; then
  echo "usage: $0 <from-version> <to-version> [--dry-run]" >&2
  echo "example: $0 0.3.2 0.4.0" >&2
  exit 2
fi

FROM="$1"
TO="$2"
DRY_RUN=0
if [[ "${3:-}" == "--dry-run" ]]; then
  DRY_RUN=1
fi

# Validate semver-ish format (X.Y.Z, no leading 'v')
if ! [[ "$FROM" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: <from-version> must be MAJOR.MINOR.PATCH (no leading 'v'). got: $FROM" >&2
  exit 2
fi
if ! [[ "$TO" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: <to-version> must be MAJOR.MINOR.PATCH (no leading 'v'). got: $TO" >&2
  exit 2
fi

# Cd to repo root regardless of where the user invoked us from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# TARGET FILES — the canonical index.
#
# Format: each entry is "PATH|DESCRIPTION|MATCH_PATTERN"
# MATCH_PATTERN uses {VER} as the literal placeholder for $FROM during the
# search and $TO during the replacement.
#
# ADD NEW VERSION-TIED FILES HERE. If a future PR introduces a file that
# hardcodes the version, append it to this list — the script becomes
# wrong-but-loud rather than silently incomplete.
# ---------------------------------------------------------------------------
TARGET_FILES=(
  # ---- Cargo workspace + crates that don't use version.workspace = true ----
  'Cargo.toml|workspace.package.version|version = "{VER}"'
  'Cargo.toml|workspace path-dep mneme-common|mneme-common = { path = "common", version = "{VER}" }'
  'Cargo.toml|workspace path-dep mneme-store|mneme-store = { path = "store", version = "{VER}" }'
  'vision/tauri/Cargo.toml|tauri standalone (excluded from workspace)|version = "{VER}"'

  # ---- Python packaging ----
  'pip/pyproject.toml|pip wrapper PyPI version|version = "{VER}"'
  'sdk/python/pyproject.toml|sdk/python PyPI version|version = "{VER}"'

  # ---- Node packaging ----
  'mcp/package.json|MCP server npm version|"version": "{VER}"'
  'vision/package.json|vision SPA package version|"version": "{VER}"'
  'vscode/package.json|VS Code extension version|"version": "{VER}"'
  'sdk/js/package.json|sdk/js npm version|"version": "{VER}"'

  # ---- Claude Code plugin manifest + marketplace ----
  'plugin/plugin.json|plugin manifest version|"version": "{VER}"'
  'plugin/marketplace.json|plugin marketplace version|"version": "{VER}"'
  'marketplace.json|root marketplace version|"version": "{VER}"'

  # ---- Tauri config ----
  'vision/tauri/tauri.conf.json|tauri app version|"version": "{VER}"'
)

# ---------------------------------------------------------------------------
# WORKFLOW PATCHES — special-cased because the artifact filenames carry
# the v-prefixed version embedded in matrix values, and the input default
# also needs to flip. Treated separately so the regex stays narrow.
# ---------------------------------------------------------------------------
WORKFLOW_FILES=(
  '.github/workflows/multi-arch-release.yml'
  '.github/workflows/release.yml'
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
section() { echo ""; echo "== $1 =="; }
ok() { echo "  ok  $1"; }
miss() { echo "  --  $1 (no $FROM occurrence)"; }
fail() { echo "  XX  $1" >&2; }

# Portable in-place sed: GNU sed wants `-i` with no arg, BSD/macOS sed
# wants `-i ''`. Detect once.
SED_INPLACE=(-i)
if sed --version 2>/dev/null | grep -q 'GNU sed'; then
  : # GNU; use -i with no arg
else
  # macOS BSD sed needs an empty backup-suffix arg
  SED_INPLACE=(-i '')
fi

apply_replacement() {
  # $1 = file path
  # $2 = description
  # $3 = pattern with literal {VER} placeholder
  local file="$1"
  local desc="$2"
  local pat="$3"
  if [[ ! -f "$file" ]]; then
    fail "$file (file missing)"
    return
  fi
  local from_pat="${pat//\{VER\}/$FROM}"
  local to_pat="${pat//\{VER\}/$TO}"
  # Escape regex metacharacters in the FROM pattern so sed treats it
  # literally — the patterns above are intentionally plain strings, not
  # regexes.
  local escaped_from
  escaped_from=$(printf '%s\n' "$from_pat" | sed 's/[][\.*^$/]/\\&/g')
  local escaped_to
  escaped_to=$(printf '%s\n' "$to_pat" | sed 's/[\&/]/\\&/g')
  if ! grep -qF "$from_pat" "$file"; then
    miss "$file ($desc)"
    return
  fi
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "  >>  $file ($desc)"
    echo "      - $from_pat"
    echo "      + $to_pat"
  else
    sed "${SED_INPLACE[@]}" -E "s/${escaped_from}/${escaped_to}/g" "$file"
    ok "$file ($desc)"
  fi
}

apply_workflow_replacement() {
  # Workflows have multiple v-prefixed occurrences (artifact names + the
  # default tag input). One global s/v$FROM/v$TO/g is correct here because
  # the workflow files don't reference historical versions in comments
  # except in the file header banner — which we leave alone since the
  # bump script's audit step prints the diff for review.
  local file="$1"
  if [[ ! -f "$file" ]]; then
    fail "$file (file missing)"
    return
  fi
  if ! grep -qF "v$FROM" "$file"; then
    miss "$file (no v$FROM occurrences)"
    return
  fi
  local count
  count=$(grep -cF "v$FROM" "$file" || true)
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "  >>  $file ($count occurrences of v$FROM -> v$TO)"
  else
    sed "${SED_INPLACE[@]}" "s/v${FROM}/v${TO}/g" "$file"
    ok "$file ($count occurrences swapped)"
  fi
}

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
section "Pre-flight"
echo "  repo root: $REPO_ROOT"
echo "  from:      $FROM"
echo "  to:        $TO"
echo "  mode:      $([[ $DRY_RUN -eq 1 ]] && echo "DRY-RUN" || echo "APPLY")"
echo "  targets:   ${#TARGET_FILES[@]} file entries + ${#WORKFLOW_FILES[@]} workflow files"

# ---------------------------------------------------------------------------
# Apply
# ---------------------------------------------------------------------------
section "Update version-tied files"
for entry in "${TARGET_FILES[@]}"; do
  IFS='|' read -r path desc pat <<<"$entry"
  apply_replacement "$path" "$desc" "$pat"
done

section "Update workflow YAMLs"
for wf in "${WORKFLOW_FILES[@]}"; do
  apply_workflow_replacement "$wf"
done

# ---------------------------------------------------------------------------
# Lockfile regeneration reminders (don't auto-run — bun install can take
# ~60s and the user may want to control when network calls happen).
# ---------------------------------------------------------------------------
section "Post-bump steps (run manually)"
if [[ $DRY_RUN -eq 1 ]]; then
  cat <<'EOF'
  Dry-run done. To apply, re-run without --dry-run.
EOF
else
  cat <<EOF
  1. Regenerate lockfiles:
       (cd vision && bun install)
       (cd vscode && bun install)
       (cd mcp    && bun install --frozen-lockfile false)
       (cd sdk/js && bun install)
  2. Sanity gates:
       cargo fmt --all -- --check
       cargo check --workspace
       bash scripts/check-home-dir-discipline.sh
       (cd vision && bunx tsc --noEmit)
  3. Inspect the diff:
       git diff --stat
       git diff
  4. Commit:
       git add -A
       git commit -m "chore(release): bump $FROM -> $TO"
  5. Tag + push:
       git tag v$TO
       git push origin main:main && git push origin v$TO
  6. Create the release (if it doesn't exist):
       gh release create v$TO --title "v$TO" --notes-file CHANGELOG.md
       # OR --generate-notes for auto from commits
  7. Re-trigger multi-arch builds:
       gh workflow run multi-arch-release.yml --ref main -f tag=v$TO
EOF
fi
echo ""
echo "Done."
