#!/usr/bin/env bash
#
# scripts/check-counts.sh
#
# Verify the count claims in README.md against the actual source code.
# Counts:
#   - Platform adapters in cli/src/platforms/
#   - Language enum variants in parsers/src/language.rs
#   - DbLayer shards in common/src/layer.rs
#   - Vision view files in vision/src/views/
#   - Scanners in scanners/src/scanners/
#   - MCP tools in mcp/src/tools/
#
# Usage:
#   scripts/check-counts.sh           # verify against README; exit non-zero on drift
#   scripts/check-counts.sh --print   # print actual counts; exit 0
#
# Tracked in issues.md WIDE-014. Wire into CI to keep README honest.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# --- Count helpers ---------------------------------------------------------

count_platform_variants() {
    # Count `Platform::Foo` variants in the public enum's `as_str` arm.
    awk '/Platform::[A-Z][a-zA-Z]+ =>/ { print $0 }' cli/src/platforms/mod.rs \
        | grep -oE 'Platform::[A-Z][a-zA-Z]+' \
        | sort -u \
        | wc -l \
        | tr -d ' '
}

count_languages() {
    # Variants inside `pub enum Language { ... }` block.
    awk '/^pub enum Language[[:space:]]*\{/,/^}/' parsers/src/language.rs \
        | grep -E '^[[:space:]]+[A-Z][a-zA-Z]*,?$' \
        | wc -l \
        | tr -d ' '
}

count_dblayer_shards() {
    # Variants inside `pub enum DbLayer { ... }` block.
    awk '/^pub enum DbLayer[[:space:]]*\{/,/^}/' common/src/layer.rs \
        | grep -E '^[[:space:]]+[A-Z][a-zA-Z]*,?$' \
        | wc -l \
        | tr -d ' '
}

count_vision_views() {
    find vision/src/views -maxdepth 1 -type f -name '*.tsx' 2>/dev/null \
        | grep -v 'index.tsx$' \
        | wc -l \
        | tr -d ' '
}

count_scanners() {
    find scanners/src/scanners -maxdepth 1 -type f -name '*.rs' 2>/dev/null \
        | grep -v '/mod\.rs$' \
        | wc -l \
        | tr -d ' '
}

count_mcp_tools() {
    find mcp/src/tools -maxdepth 1 -type f -name '*.ts' 2>/dev/null \
        | wc -l \
        | tr -d ' '
}

# --- Run --------------------------------------------------------------------

PLATFORMS="$(count_platform_variants)"
LANGUAGES="$(count_languages)"
SHARDS="$(count_dblayer_shards)"
VIEWS="$(count_vision_views)"
SCANNERS="$(count_scanners)"
MCP_TOOLS="$(count_mcp_tools)"

if [[ "${1:-}" == "--print" ]]; then
    echo "platforms=$PLATFORMS"
    echo "languages=$LANGUAGES"
    echo "shards=$SHARDS"
    echo "views=$VIEWS"
    echo "scanners=$SCANNERS"
    echo "mcp_tools=$MCP_TOOLS"
    exit 0
fi

# --- Verify against README --------------------------------------------------

README="README.md"
fail=0

# Helper: search README for "${count} ${noun}" or "<noun>${count}<" hard-coded
expect_count() {
    local label="$1"
    local actual="$2"
    local pattern="$3"
    if ! grep -qE "$pattern" "$README"; then
        echo "DRIFT: README is missing the expected $label count of $actual" >&2
        echo "       (looked for pattern: $pattern)" >&2
        fail=1
    fi
}

expect_count "platform"   "$PLATFORMS"  "platforms-${PLATFORMS}|${PLATFORMS} supported platforms|${PLATFORMS} AI"
expect_count "language"   "$LANGUAGES"  "Language enum variants = ${LANGUAGES}|>${LANGUAGES}<.*LANGUAGES"
expect_count "shard"      "$SHARDS"     "DbLayer.*shards = ${SHARDS}|>${SHARDS}<.*SQLITE SHARDS"
expect_count "view"       "$VIEWS"      "views/\\*\\.tsx</code> = ${VIEWS}|>${VIEWS}<.*LIVE VIEWS"
expect_count "scanner"    "$SCANNERS"   "scanners/\\*\\.rs</code> = ${SCANNERS}|>${SCANNERS}<.*SCANNERS"
expect_count "mcp tool"   "$MCP_TOOLS"  "tools/\\*\\.ts</code> = ${MCP_TOOLS}|>${MCP_TOOLS}<"

if [[ $fail -ne 0 ]]; then
    echo "" >&2
    echo "Actual counts (run with --print to copy/paste):" >&2
    echo "  platforms = $PLATFORMS" >&2
    echo "  languages = $LANGUAGES" >&2
    echo "  shards    = $SHARDS" >&2
    echo "  views     = $VIEWS" >&2
    echo "  scanners  = $SCANNERS" >&2
    echo "  mcp_tools = $MCP_TOOLS" >&2
    exit 1
fi

echo "OK — all README count claims match source."
