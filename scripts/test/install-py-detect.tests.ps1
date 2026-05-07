# scripts/test/install-py-detect.tests.ps1
#
# Tests for B-006: Microsoft-Store Python stub detection in install.ps1.
# Validates `Test-PythonRealOrStub` returns the right answer for stub paths
# (without invoking the executable) and well-formed real-Python output.
#
# Runs in two modes:
#   1. Pester 5+ available -- uses Describe/It/Should -Be (preferred).
#   2. No Pester -- falls back to pure-PowerShell asserts. Either way the
#      script exits 0 on pass, 1 on fail (CI-friendly).
#
# Usage (from repo root):
#   pwsh -File scripts/test/install-py-detect.tests.ps1
#
# Or with Pester 5:
#   Invoke-Pester scripts/test/install-py-detect.tests.ps1

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Load Test-PythonRealOrStub from install.ps1 WITHOUT executing the rest of
# the install script. install.ps1 has top-level side effects (Write-Step,
# process kills, downloads), so dot-sourcing is unsafe. Instead, regex-
# extract the function body and Invoke-Expression it in this scope.
# ---------------------------------------------------------------------------

$installScript = Join-Path $PSScriptRoot '..\install.ps1'
$installScript = (Resolve-Path $installScript).Path
$src = Get-Content -Raw -LiteralPath $installScript

# Brace-balanced extractor for `function Test-PythonRealOrStub { ... }`.
$startIdx = $src.IndexOf('function Test-PythonRealOrStub')
if ($startIdx -lt 0) {
    Write-Host "FAIL: function Test-PythonRealOrStub not found in $installScript" -ForegroundColor Red
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
    Write-Host "FAIL: could not find closing brace for Test-PythonRealOrStub" -ForegroundColor Red
    exit 1
}
$funcSrc = $src.Substring($startIdx, $endIdx - $startIdx + 1)
Invoke-Expression $funcSrc

# ---------------------------------------------------------------------------
# Test cases. Each is (name, scriptblock, expected) -- scriptblock returns
# $true / $false. The test runner just compares.
# ---------------------------------------------------------------------------

$tests = @()

# Case 1: null path -> $false (must NOT throw, must NOT execute anything).
$tests += @{
    Name     = 'null path returns $false'
    Block    = { Test-PythonRealOrStub -ExePath $null }
    Expected = $false
}

# Case 2: empty-string path -> $false.
$tests += @{
    Name     = 'empty path returns $false'
    Block    = { Test-PythonRealOrStub -ExePath '' }
    Expected = $false
}

# Case 3: WindowsApps stub-shaped path -> $false WITHOUT calling --version.
# This is THE case that B-006 is about. We assert by giving a path that
# doesn't exist on the machine. Test-Path will be $false, which short-
# circuits and never invokes the executable. To prove the WindowsApps
# branch is also independently sufficient, we use a fake but real-on-disk
# proxy in case 4.
$tests += @{
    Name     = 'WindowsApps stub path (nonexistent) returns $false'
    Block    = { Test-PythonRealOrStub -ExePath 'C:\Users\TestUser\AppData\Local\Microsoft\WindowsApps\python.exe' }
    Expected = $false
}

# Case 4: WindowsApps stub path that EXISTS on disk -> $false WITHOUT
# invoking the executable. We create a temp file at a path that matches
# `*\WindowsApps\*` and assert detection short-circuits on path inspection
# alone. The temp file is not an executable, so if invocation were
# attempted, it would throw -- proving the path-check fired first.
$tmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('mneme-py-detect-' + [Guid]::NewGuid().ToString('N'))
$fakeStubDir = Join-Path $tmpRoot 'WindowsApps'
New-Item -ItemType Directory -Force -Path $fakeStubDir | Out-Null
$fakeStubPath = Join-Path $fakeStubDir 'python.exe'
'not really an exe' | Set-Content -LiteralPath $fakeStubPath
$tests += @{
    Name     = 'WindowsApps stub path (file exists, would-throw if executed) returns $false'
    Block    = { Test-PythonRealOrStub -ExePath $script:fakeStubPath }
    Expected = $false
}
$script:fakeStubPath = $fakeStubPath

# Case 5: nonexistent non-WindowsApps path -> $false.
$tests += @{
    Name     = 'nonexistent path returns $false'
    Block    = { Test-PythonRealOrStub -ExePath 'C:\Definitely\Nope\python.exe' }
    Expected = $false
}

# Case 6: real Python -- only run if a real python is present on this machine.
# We probe via Get-Command -All and skip the WindowsApps entries. If a real
# one exists, we expect $true.
$realPy = $null
$pyCmds = @(Get-Command python -All -ErrorAction SilentlyContinue) +
          @(Get-Command python3 -All -ErrorAction SilentlyContinue)
foreach ($cmd in $pyCmds) {
    if ($cmd -and $cmd.Source -and $cmd.Source -notlike '*\WindowsApps\*') {
        $realPy = $cmd.Source
        break
    }
}
if ($realPy) {
    $script:realPy = $realPy
    $tests += @{
        Name     = "real python at $realPy returns `$true"
        Block    = { Test-PythonRealOrStub -ExePath $script:realPy }
        Expected = $true
    }
} else {
    Write-Host "    skip: no real Python on this machine -- skipping happy-path test" -ForegroundColor Yellow
}

# ---------------------------------------------------------------------------
# Runner. Pester-aware if Pester 5+ loaded, else plain-PS asserts.
# ---------------------------------------------------------------------------

$pesterMod = Get-Module -ListAvailable Pester |
             Where-Object { $_.Version.Major -ge 5 } |
             Sort-Object Version -Descending |
             Select-Object -First 1

if ($pesterMod) {
    Import-Module Pester -MinimumVersion 5.0
    Describe 'Test-PythonRealOrStub (B-006 stub detection)' {
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
} else {
    Write-Host '==> Test-PythonRealOrStub (B-006 stub detection)' -ForegroundColor Cyan
    $pass = 0
    $fail = 0
    foreach ($t in $tests) {
        try {
            $actual = & $t.Block
            if ($actual -eq $t.Expected) {
                Write-Host ("    [PASS] " + $t.Name) -ForegroundColor Green
                $pass++
            } else {
                Write-Host ("    [FAIL] " + $t.Name + " -- expected $($t.Expected), got $actual") -ForegroundColor Red
                $fail++
            }
        } catch {
            Write-Host ("    [FAIL] " + $t.Name + " -- threw: $($_.Exception.Message)") -ForegroundColor Red
            $fail++
        }
    }
    Write-Host ""
    Write-Host ("Result: {0} passed, {1} failed" -f $pass, $fail) -ForegroundColor $(if ($fail -eq 0) { 'Green' } else { 'Red' })
    # Cleanup temp stub.
    if (Test-Path $tmpRoot) { Remove-Item -Recurse -Force -LiteralPath $tmpRoot -ErrorAction SilentlyContinue }
    if ($fail -gt 0) { exit 1 } else { exit 0 }
}

# Pester branch cleanup.
if (Test-Path $tmpRoot) { Remove-Item -Recurse -Force -LiteralPath $tmpRoot -ErrorAction SilentlyContinue }
