# check-doc-counts.ps1 — CI guard for doc-vs-code count drift.
#
# Greps the repo's user-facing docs for fragile literal counts that should
# track first-class facts in the codebase. Exits non-zero on drift.
#
# Usage:
#   pwsh -NoProfile -File scripts/test/check-doc-counts.ps1
#
# Currently checks:
#   - "<N> MCP tools" claims  vs.  mcp/src/tools/index.ts::STATIC_TOOL_FILES
#
# Exit codes:
#   0 = all doc counts agree with the canonical count
#   1 = drift detected (offending lines printed to stderr)
#   2 = setup error (missing source file, parse failure, etc.)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

# ---------------------------------------------------------------------------
# Resolve repo root regardless of where the script is invoked from.
# ---------------------------------------------------------------------------
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = (Resolve-Path (Join-Path $ScriptDir '..\..')).Path
$IndexTs   = Join-Path $RepoRoot 'mcp\src\tools\index.ts'

# ---------------------------------------------------------------------------
# Helper: parse STATIC_TOOL_FILES.length out of mcp/src/tools/index.ts.
# Each entry is a quoted string on its own line between the array
# opener and the closing `];`. Comment lines (`//`) and blank lines
# do not count.
# ---------------------------------------------------------------------------
function Get-StaticToolCount {
    param([string]$Path)
    if (-not (Test-Path $Path)) {
        Write-Error "STATIC_TOOL_FILES source not found at $Path"
        exit 2
    }
    $lines  = Get-Content -LiteralPath $Path
    $inside = $false
    $count  = 0
    foreach ($line in $lines) {
        if ($line -match '^const\s+STATIC_TOOL_FILES\s*=\s*\[') {
            $inside = $true
            continue
        }
        if ($inside -and ($line -match '^\];')) {
            $inside = $false
            break
        }
        if ($inside -and ($line -match '^\s*"')) {
            $count++
        }
    }
    if ($count -lt 1) {
        Write-Error "Failed to parse STATIC_TOOL_FILES entries from $Path"
        exit 2
    }
    return $count
}

# ---------------------------------------------------------------------------
# Helper: scrub a doc line of URL-encoded artefacts and full URLs so the
# numeric matcher does not pick up "%20" -> "20" or shields.io fragments.
# ---------------------------------------------------------------------------
function ConvertTo-Sanitised {
    param([string]$Line)
    $s = $Line
    # Strip URLs (http(s), data:) — they contain "%20", "%2F" badges.
    $s = [regex]::Replace($s, 'https?://\S+', ' ')
    # Drop URL-encoded bytes.
    $s = [regex]::Replace($s, '%[0-9A-Fa-f]{2}', ' ')
    # Strip common HTML entities that pad numbers.
    $s = $s -replace '&nbsp;', ' ' -replace '&amp;', ' '
    return $s
}

# ---------------------------------------------------------------------------
# Helper: scan one doc for "<N> MCP tools" claims that disagree with the
# canonical count. Returns the list of offending records.
# ---------------------------------------------------------------------------
function Find-ToolCountDrift {
    param(
        [string]$DocPath,
        [int]$CanonicalCount
    )
    if (-not (Test-Path $DocPath)) { return @() }
    $hits = @()
    $rx   = '\b(?<n>\d+)(?:\s*/\s*\d+)?\s*(?:MCP tools|MCP[ -]server\s*\(\d+\s*tools\))'
    $linesRaw = Get-Content -LiteralPath $DocPath
    foreach ($line in $linesRaw) {
        if ($line -notmatch 'MCP tools|MCP server \(\d+ tools\)') { continue }
        $sanitised = ConvertTo-Sanitised -Line $line
        $matches = [regex]::Matches($sanitised, $rx)
        foreach ($m in $matches) {
            $n = [int]$m.Groups['n'].Value
            if ($n -ne $CanonicalCount) {
                $hits += [pscustomobject]@{
                    Doc      = $DocPath
                    Found    = $n
                    Expected = $CanonicalCount
                    Line     = $line
                }
            }
        }
    }
    return $hits
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
$canonical = Get-StaticToolCount -Path $IndexTs
Write-Host "canonical tool count from STATIC_TOOL_FILES: $canonical"

$docs = @(
    (Join-Path $RepoRoot 'README.md'),
    (Join-Path $RepoRoot 'CLAUDE.md'),
    (Join-Path $RepoRoot 'INSTALL.md'),
    (Join-Path $RepoRoot 'docs\INSTALL.md')
)

$allDrift = @()
foreach ($doc in $docs) {
    $allDrift += Find-ToolCountDrift -DocPath $doc -CanonicalCount $canonical
}

if ($allDrift.Count -gt 0) {
    foreach ($d in $allDrift) {
        Write-Error ("DRIFT: {0} claims {1} but STATIC_TOOL_FILES = {2}`n       offending line: {3}" -f $d.Doc, $d.Found, $d.Expected, $d.Line) -ErrorAction Continue
    }
    Write-Error ("FAIL: doc-count drift detected. Update the docs to match STATIC_TOOL_FILES.length={0}." -f $canonical) -ErrorAction Continue
    exit 1
}

Write-Host ("OK: all doc counts agree with STATIC_TOOL_FILES.length={0}" -f $canonical)
exit 0
