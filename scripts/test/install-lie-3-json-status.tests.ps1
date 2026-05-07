# scripts/test/install-lie-3-json-status.tests.ps1
#
# LIE-3 fix verification.
#
# Pre-fix (v0.3.2 baseline):
#   install.ps1 step 7/8 invoked `mneme register-mcp --platform claude-code`
#   and printed `Write-OK "Claude Code MCP registration complete"` purely
#   from `$LASTEXITCODE -eq 0`. The hook-registration count was never
#   inspected, so the banner could (and on Bug-B repro, did) lie about a
#   half-installed state where the MCP entry landed but persistent-memory
#   hooks did not -- degrading mneme to a query-only surface without
#   surfacing that fact.
#
# Post-fix:
#   install.ps1 invokes `mneme register-mcp --platform claude-code --json`
#   which emits a single-line JSON status object on stdout:
#     { ok, hooks_registered, hooks_expected, mcp_entry_written,
#       settings_json_path, mcp_config_path, errors[] }
#   install.ps1 parses the JSON with ConvertFrom-Json and reports
#   per-field. The old "complete" line is gated behind `$status.ok`.
#
# This test mocks `mneme.exe` with a stub script that emits a JSON line
# claiming `hooks_registered: 0` and verifies install.ps1's banner does
# NOT report "complete" -- i.e. the install banner is now telling the
# truth.
#
# Runs in two modes:
#   1. Pester 5+ -- Describe/It/Should -Be (preferred).
#   2. No Pester -- pure-PowerShell asserts. Exit 0 on pass, 1 on fail.
#
# Usage (from repo root):
#   pwsh -File scripts/test/install-lie-3-json-status.tests.ps1

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Test fixture: extract just the step-7/8 block from install.ps1 so we can
# drive it with a mocked `$MnemeBin` without running the whole installer.
# Same brace-walk pattern as install-native-probe.tests.ps1.
# ---------------------------------------------------------------------------

$installScript = Join-Path $PSScriptRoot '..\install.ps1'
$installScript = (Resolve-Path $installScript).Path
$src = Get-Content -Raw -LiteralPath $installScript

$startMarker = 'Write-Step "step 7/8 - registering MCP with Claude Code"'
$startIdx = $src.IndexOf($startMarker)
if ($startIdx -lt 0) {
    Write-Host "FAIL: step 7/8 marker not found in $installScript" -ForegroundColor Red
    exit 1
}
$endMarker = '# Step 8 - Done'
$endIdx = $src.IndexOf($endMarker, $startIdx)
if ($endIdx -lt 0) {
    Write-Host "FAIL: step 8 marker not found after step 7/8" -ForegroundColor Red
    exit 1
}
$blockSrc = $src.Substring($startIdx, $endIdx - $startIdx)

# ---------------------------------------------------------------------------
# Pure-PS Write-* shims that capture into a buffer (the real install.ps1
# helpers print to console only; we need the strings).
# ---------------------------------------------------------------------------
$script:CapturedLines = New-Object System.Collections.ArrayList

function Write-Step { param([string]$Message, [string]$Color = 'Cyan')
    [void]$script:CapturedLines.Add(("STEP: $Message")) }
function Write-Info { param([string]$Message)
    [void]$script:CapturedLines.Add(("INFO: $Message")) }
function Write-Warn { param([string]$Message)
    [void]$script:CapturedLines.Add(("WARN: $Message")) }
function Write-OK   { param([string]$Message)
    [void]$script:CapturedLines.Add(("OK:   $Message")) }
function Write-Fail { param([string]$Message)
    [void]$script:CapturedLines.Add(("FAIL: $Message")) }

function Reset-Capture { $script:CapturedLines = New-Object System.Collections.ArrayList }

function Get-MnemeStubScript {
    param(
        [Parameter(Mandatory=$true)][int]$ExitCode,
        [Parameter(Mandatory=$true)][string]$JsonLine
    )
    # Build a one-liner cmd /c script that echoes the JSON line then
    # exits with the requested code. We use a real .cmd file so the
    # `& $MnemeBin ... 2>&1` invocation in install.ps1 hits the same
    # native-process semantics it does at install time.
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("mneme-stub-{0}.cmd" -f ([guid]::NewGuid().ToString('N')))
    $stubBody = "@echo off`r`necho $JsonLine`r`nexit /b $ExitCode`r`n"
    [System.IO.File]::WriteAllText($tmp, $stubBody, [System.Text.UTF8Encoding]::new($false))
    return $tmp
}

function Invoke-Step7Block {
    param([string]$MnemeBinPath)
    Reset-Capture
    $MnemeBin = $MnemeBinPath
    # Drive the extracted step-7/8 block with our mocked $MnemeBin in scope.
    Invoke-Expression $blockSrc
}

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

$tests = @(
    @{
        name = 'reports per-field truth when hooks_registered is 0'
        run  = {
            $stub = Get-MnemeStubScript -ExitCode 0 -JsonLine `
                '{"ok":false,"hooks_registered":0,"hooks_expected":8,"mcp_entry_written":true,"settings_json_path":"C:\\fake\\settings.json","mcp_config_path":"C:\\fake\\.claude.json","errors":["hooks failed"]}'
            try {
                Invoke-Step7Block -MnemeBinPath $stub
                $joined = [string]::Join("`n", @($script:CapturedLines))

                # The lie was "Claude Code MCP registration complete"
                # printed unconditionally on exit 0. Post-fix that line
                # only fires when $status.ok is true.
                if ($joined -match 'Claude Code MCP registration complete \(verified per-field\)') {
                    return "FAIL: install.ps1 still prints 'complete' even though ok=false"
                }
                # And the banner MUST report the per-field truth.
                if ($joined -notmatch 'hooks_registered: 0/8') {
                    return "FAIL: hooks_registered field not reported. captured=$joined"
                }
                if ($joined -notmatch 'mcp_entry_written: yes') {
                    return "FAIL: mcp_entry_written field not reported. captured=$joined"
                }
                if ($joined -notmatch 'INCOMPLETE') {
                    return "FAIL: incomplete-state warning not surfaced. captured=$joined"
                }
                return $null
            } finally {
                Remove-Item -LiteralPath $stub -ErrorAction SilentlyContinue
            }
        }
    },
    @{
        name = 'reports verified-complete when ok=true and all fields green'
        run  = {
            $stub = Get-MnemeStubScript -ExitCode 0 -JsonLine `
                '{"ok":true,"hooks_registered":0,"hooks_expected":0,"mcp_entry_written":true,"settings_json_path":"","mcp_config_path":"C:\\fake\\.claude.json","errors":[]}'
            try {
                Invoke-Step7Block -MnemeBinPath $stub
                $joined = [string]::Join("`n", @($script:CapturedLines))
                if ($joined -notmatch 'Claude Code MCP registration complete \(verified per-field\)') {
                    return "FAIL: ok=true case did not surface verified-complete banner. captured=$joined"
                }
                return $null
            } finally {
                Remove-Item -LiteralPath $stub -ErrorAction SilentlyContinue
            }
        }
    },
    @{
        name = 'falls back without claiming complete when JSON is missing'
        run  = {
            # Stub that exits 0 but emits NO JSON line -- the regression
            # surface for callers built before LIE-3 (or a bug emitting
            # nothing). install.ps1 must NOT print the "complete" line.
            $stub = Get-MnemeStubScript -ExitCode 0 -JsonLine 'no-json-here'
            try {
                Invoke-Step7Block -MnemeBinPath $stub
                $joined = [string]::Join("`n", @($script:CapturedLines))
                if ($joined -match 'Claude Code MCP registration complete \(verified per-field\)') {
                    return "FAIL: no-JSON path falsely printed verified-complete. captured=$joined"
                }
                if ($joined -notmatch 'no JSON status') {
                    return "FAIL: no-JSON fallback message missing. captured=$joined"
                }
                return $null
            } finally {
                Remove-Item -LiteralPath $stub -ErrorAction SilentlyContinue
            }
        }
    }
)

# ---------------------------------------------------------------------------
# Pester adapter (preferred when available).
# ---------------------------------------------------------------------------
$hasPester = (Get-Module -ListAvailable -Name Pester | Select-Object -First 1) -ne $null

if ($hasPester) {
    Describe 'install.ps1 step 7/8 LIE-3 JSON status reporting' {
        foreach ($t in $tests) {
            It $t.name {
                $err = & $t.run
                $err | Should -BeNullOrEmpty
            }
        }
    }
    return
}

# ---------------------------------------------------------------------------
# Pester-less fallback.
# ---------------------------------------------------------------------------
$failed = 0
foreach ($t in $tests) {
    $err = & $t.run
    if ($err) {
        Write-Host ("[FAIL] {0} - {1}" -f $t.name, $err) -ForegroundColor Red
        $failed++
    } else {
        Write-Host ("[PASS] {0}" -f $t.name) -ForegroundColor Green
    }
}
if ($failed -gt 0) {
    Write-Host ("{0} test(s) failed" -f $failed) -ForegroundColor Red
    exit 1
}
Write-Host "all LIE-3 install.ps1 JSON status tests passed" -ForegroundColor Green
exit 0
