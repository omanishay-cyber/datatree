#!/usr/bin/env bash
# check-doc-counts.sh — CI guard for doc-vs-code count drift
#
# Greps the repo's user-facing docs for fragile literal counts that should
# track first-class facts in the codebase. Fails non-zero on drift.
#
# Currently checks:
#   - "47 MCP tools" / "47/47 MCP tools" / "46 MCP tools" claims  vs.
#     mcp/src/tools/index.ts::STATIC_TOOL_FILES.length
#
# Usage:  scripts/test/check-doc-counts.sh
# Exit:   0 if all counts match, 1 if any drift detected.

set -euo pipefail

# Resolve the repo root regardless of where the script is invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

INDEX_TS="$REPO_ROOT/mcp/src/tools/index.ts"

if [[ ! -f "$INDEX_TS" ]]; then
  echo "ERROR: $INDEX_TS not found — cannot determine canonical tool count" >&2
  exit 2
fi

# Count entries in the STATIC_TOOL_FILES array. Each entry is a quoted
# string on its own line, so we count quoted lines between the array
# opener and closer. Comment lines starting with `//` are excluded.
STATIC_COUNT="$(awk '
  /^const STATIC_TOOL_FILES = \[/ { capture=1; next }
  capture && /^\];/             { capture=0 }
  capture && /^\s*"/            { count++ }
  END                           { print count }
' "$INDEX_TS")"

if [[ -z "$STATIC_COUNT" || "$STATIC_COUNT" -lt 1 ]]; then
  echo "ERROR: failed to parse STATIC_TOOL_FILES from $INDEX_TS" >&2
  exit 2
fi

echo "canonical tool count from STATIC_TOOL_FILES: $STATIC_COUNT"

# Documents to check. Add new files here as fragile claims land.
DOCS=(
  "$REPO_ROOT/README.md"
  "$REPO_ROOT/CLAUDE.md"
  "$REPO_ROOT/INSTALL.md"
  "$REPO_ROOT/docs/INSTALL.md"
)

drift=0
for doc in "${DOCS[@]}"; do
  if [[ ! -f "$doc" ]]; then
    continue
  fi

  # Patterns that LIE if the count is wrong. Catch every "<N> MCP tools",
  # "<N>/<N> MCP tools", "(<N> tools)" form.
  while IFS= read -r line; do
    # Skip blank lines and historical CHANGELOG/release-notes entries.
    [[ -z "$line" ]] && continue

    # Strip URL-encoded artefacts (%20 = space, %2F = "/"), full URLs and
    # markdown image refs so badge URLs do not produce false positives
    # (e.g. shields.io "MCP%20tools-48%2F48%20wired" should not parse
    # the "20" from "%20" as a tool count).
    sanitised="$(echo "$line" \
      | sed -E 's|https?://[^[:space:]"]+||g' \
      | sed -E 's/%[0-9A-Fa-f]{2}/ /g' \
      | sed -E 's/&nbsp;/ /g' \
      | sed -E 's/&amp;/ /g')"

    # Loop over EVERY "<N> ... MCP tools|tools" occurrence on the line
    # so a single line with multiple counts (e.g. "47 → 48") can flag
    # all stale numbers.
    while read -r match; do
      [[ -z "$match" ]] && continue
      found_n="$(echo "$match" | grep -oE '^[0-9]+' || true)"
      [[ -z "$found_n" ]] && continue

      if [[ "$found_n" != "$STATIC_COUNT" ]]; then
        echo "DRIFT: $doc claims $found_n but STATIC_TOOL_FILES = $STATIC_COUNT" >&2
        echo "       offending line: $line" >&2
        drift=1
      fi
    done < <(echo "$sanitised" | grep -oE '\b[0-9]+(\s*/\s*[0-9]+)?\s*(MCP tools|MCP[ -]server\s*\([0-9]+\s*tools\))' || true)
  done < <(grep -E 'MCP tools|MCP server \([0-9]+ tools\)' "$doc" || true)
done

if [[ "$drift" -ne 0 ]]; then
  echo "FAIL: doc-count drift detected. Update the docs to match STATIC_TOOL_FILES.length=$STATIC_COUNT." >&2
  exit 1
fi

echo "OK: all doc counts agree with STATIC_TOOL_FILES.length=$STATIC_COUNT"
exit 0
