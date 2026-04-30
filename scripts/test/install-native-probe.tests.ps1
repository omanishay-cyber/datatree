# scripts/test/install-native-probe.tests.ps1
#
# Tests for the generic `Invoke-NativeProbe` helper introduced in install.ps1
# to fix B-006 follow-on (G7 `cargo tauri --version` aborting under
# $ErrorActionPreference='Stop' when tauri-cli isn't installed yet — same
# class of bug as G3 Python stub but for any native exe in G1-G10).
#
# `Invoke-NativeProbe` MUST:
#   1. Return $false-shaped Success for an existing exe that exits non-zero,
#      WITHOUT throwing under $ErrorActionPreference='Stop'.
#   2. Return $false-shaped Success for an existing exe that prints to stderr,
#      WITHOUT throwing.
#   3. Return $false for a missing exe path, WITHOUT throwing.
#   4. Return $true-shaped Success for an existing exe that exits 0.
#
# Runs in two modes:
#   1. Pester 5+ — Describe/It/Should -Be (preferred).
#   2. No Pester — pure-PowerShell asserts. Exit 0 on pass, 1 on fail.
#
# Usage (from repo root):
#   pwsh -File scripts/test/install-native-probe.tests.ps1

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Load Invoke-NativeProbe from install.ps1 WITHOUT executing the rest of the
# install script. Same brace-balanced extractor pattern as
# install-py-detect.tests.ps1.
# ---------------------------------------------------------------------------

$installScript = Join-Path $PSScriptRoot '..\install.ps1'
$installScript = (Resolve-Path $installScript).Path
$src = Get-Content -Raw -LiteralPath $installScript

$startIdx = $src.IndexOf('function Invoke-NativeProbe')
if ($startIdx -lt 0) {
    Write-Host "FAIL: function Invoke-NativeProbe not found in $installScript" -ForegroundColor Red
    exit 1
}
$openBrace = $src.IndexOf('{', $startIdx)
$depth = 0
$endIdx = -1
for ($i = $openBrace; $i -lt $src.Length; $i++) {
    $c = $src[$i]
    if ($c -eq '{') { $depth++ }
    elseif ($c -eq '}') {
        $depth--
        if ($depth -eq 0) { $endIdx = $i; break }
    }
}
if ($endIdx -lt 0) {
    Write-Host "FAIL: could not find closing brace for Invoke-NativeProbe" -ForegroundColor Red
    exit 1
}
$funcSrc = $src.Substring($startIdx, $endIdx - $startIdx + 1)
Invoke-Expression $funcSrc

# ---------------------------------------------------------------------------
# Test fixture: build an exe-like script that we can drive deterministically.
# Windows PowerShell can call `cmd.exe /c <batch>` and we get a real native
# command exit code through it. We use cmd.exe directly so each test case
# can pick its own exit code via `cmd /c "exit N"`.
# ---------------------------------------------------------------------------

$cmdExe = (Get-Command cmd.exe -ErrorAction SilentlyContinue).Source
if (-not $cmdExe) {
    Write-Host "FAIL: cmd.exe not found on PATH — cannot run probe fixtures" -ForegroundColor Red
    exit 1
}

# Build a tiny .bat that exits 0 and prints "ok 1.2.3" — emulates a clean
# `--version` probe.
$tmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('mneme-probe-tests-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmpRoot | Out-Null

$okBat = Join-Path $tmpRoot 'ok-version.bat'
@'
@echo off
echo ok 1.2.3
exit /b 0
'@ | Set-Content -LiteralPath $okBat -Encoding ASCII

$failBat = Join-Path $tmpRoot 'fail-version.bat'
@'
@echo off
echo error: no such command: tauri 1>&2
exit /b 101
'@ | Set-Content -LiteralPath $failBat -Encoding ASCII

$stderrBat = Join-Path $tmpRoot 'stderr-version.bat'
@'
@echo off
echo Python was not found; run without arguments to install from the Microsoft Store... 1>&2
exit /b 9009
'@ | Set-Content -LiteralPath $stderrBat -Encoding ASCII

# ---------------------------------------------------------------------------
# Test cases.
# ---------------------------------------------------------------------------

$tests = @()

# Case 1: existing exe + clean exit 0 -> Success = $true.
$tests += @{
    Name     = 'invoke_probe_returns_success_for_existing_exe'
    Block    = {
        $r = Invoke-NativeProbe -ExePath $script:okBat -ProbeArgs @('--version')
        return ($r.Success -eq $true -and $r.ExitCode -eq 0)
    }
    Expected = $true
}

# Case 2: missing exe path -> Success = $false WITHOUT throwing.
# Critical: this MUST hold even with $ErrorActionPreference='Stop' (which is
# already set at the top of this file).
$tests += @{
    Name     = 'invoke_probe_returns_failure_for_missing_exe_without_throw'
    Block    = {
        $threw = $false
        try {
            $r = Invoke-NativeProbe -ExePath 'C:\Definitely\Nope\nonexistent.exe' -ProbeArgs @('--version')
        } catch {
            $threw = $true
        }
        return ((-not $threw) -and ($null -ne $r) -and ($r.Success -eq $false))
    }
    Expected = $true
}

# Case 3: existing exe + non-zero exit -> Success = $false WITHOUT throwing.
# This is THE B-006 follow-on case (cargo tauri --version on a box without
# tauri-cli installed: exits 101 with "no such command").
$tests += @{
    Name     = 'invoke_probe_returns_failure_when_exe_returns_nonzero_without_throw'
    Block    = {
        $threw = $false
        try {
            $r = Invoke-NativeProbe -ExePath $script:failBat -ProbeArgs @('--version')
        } catch {
            $threw = $true
        }
        return ((-not $threw) -and ($null -ne $r) -and ($r.Success -eq $false) -and ($r.ExitCode -eq 101))
    }
    Expected = $true
}

# Case 4: existing exe + stderr output + non-zero exit -> Success = $false
# WITHOUT throwing. The Microsoft-Store python stub case (B-006) and any
# probe that writes to stderr under EAP=Stop normally aborts the script.
$tests += @{
    Name     = 'invoke_probe_returns_failure_when_exe_outputs_to_stderr_without_throw'
    Block    = {
        $threw = $false
        try {
            $r = Invoke-NativeProbe -ExePath $script:stderrBat -ProbeArgs @('--version')
        } catch {
            $threw = $true
        }
        return ((-not $threw) -and ($null -ne $r) -and ($r.Success -eq $false))
    }
    Expected = $true
}

# Stash fixture paths in script scope so the test scriptblocks can see them.
$script:okBat = $okBat
$script:failBat = $failBat
$script:stderrBat = $stderrBat

# ---------------------------------------------------------------------------
# Runner.
# ---------------------------------------------------------------------------

$pesterMod = Get-Module -ListAvailable Pester |
             Where-Object { $_.Version.Major -ge 5 } |
             Sort-Object Version -Descending |
             Select-Object -First 1

if ($pesterMod) {
    Import-Module Pester -MinimumVersion 5.0
    Describe 'Invoke-NativeProbe (B-006 follow-on: generic native-exe probe)' {
        foreach ($t in $tests) {
            $name = $t.Name
            $block = $t.Block
            $expected = $t.Expected
            It $name {
                $actual = & $block
                $actual | Should -Be $expected
            }
        }
    }
    if (Test-Path $tmpRoot) { Remove-Item -Recurse -Force -LiteralPath $tmpRoot -ErrorAction SilentlyContinue }
} else {
    Write-Host '==> Invoke-NativeProbe (B-006 follow-on: generic native-exe probe)' -ForegroundColor Cyan
    $pass = 0
    $fail = 0
    foreach ($t in $tests) {
        try {
            $actual = & $t.Block
            if ($actual -eq $t.Expected) {
                Write-Host ("    [PASS] " + $t.Name) -ForegroundColor Green
                $pass++
            } else {
                Write-Host ("    [FAIL] " + $t.Name + " - expected $($t.Expected), got $actual") -ForegroundColor Red
                $fail++
            }
        } catch {
            Write-Host ("    [FAIL] " + $t.Name + " - threw: $($_.Exception.Message)") -ForegroundColor Red
            $fail++
        }
    }
    Write-Host ""
    Write-Host ("Result: {0} passed, {1} failed" -f $pass, $fail) -ForegroundColor $(if ($fail -eq 0) { 'Green' } else { 'Red' })
    if (Test-Path $tmpRoot) { Remove-Item -Recurse -Force -LiteralPath $tmpRoot -ErrorAction SilentlyContinue }
    if ($fail -gt 0) { exit 1 } else { exit 0 }
}
