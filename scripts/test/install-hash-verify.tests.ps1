# scripts/test/install-hash-verify.tests.ps1
#
# HIGH-13 (2026-05-06 deep audit): tests for the SHA-256 verification
# block in install.ps1. The block:
#   1. Fetches release-checksums.json sidecar from the GH Release.
#   2. Parses it for the asset basename -> sha256-hex mapping.
#   3. Computes Get-FileHash SHA256 on the downloaded zip.
#   4. Lowercases both, compares.
#   5. On mismatch -> Write-Fail + exit 1.
#   6. On match    -> Write-OK MATCH + continue to extract.
#
# We DO NOT run install.ps1 end-to-end (heavy side effects: kills
# processes, registers MCP, starts daemon). Instead we lift the
# verification logic into an isolated function and exercise it on
# fake zips + fake manifests built inside a temp directory.
#
# Cases:
#   1. parse_smoke                          install.ps1 still parses with
#                                           -SkipHashVerify in the param block.
#   2. param_block_includes_skiphashverify  the param block declares the new
#                                           switch.
#   3. envvar_read_present                  install.ps1 reads MNEME_SKIP_HASH_VERIFY
#                                           and MNEME_SKIP_HASH_CHECK.
#   4. verdict_print_present                Write-Info expected/actual lines exist.
#   5. match_passes                         identical hashes -> verdict MATCH.
#   6. tampered_one_byte_rejected           flipping one byte must throw.
#   7. override_skips_tampered              MNEME_SKIP_HASH_VERIFY=1 bypasses.
#   8. no_manifest_warns_continues          missing manifest -> non-fatal warn path.
#
# Usage:
#   pwsh -File scripts/test/install-hash-verify.tests.ps1
# Or with Pester 5:
#   Invoke-Pester scripts/test/install-hash-verify.tests.ps1

$ErrorActionPreference = 'Stop'

$installScript = Join-Path $PSScriptRoot '..\install.ps1'
$installScript = (Resolve-Path $installScript).Path

# ---------------------------------------------------------------------------
# Smoke check 1 - install.ps1 parses cleanly. Catches any regression
# introduced by the SkipHashVerify branch.
# ---------------------------------------------------------------------------
$parseTokens = $null
$parseErrors = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile(
    $installScript, [ref]$parseTokens, [ref]$parseErrors)
if ($parseErrors -and $parseErrors.Count -gt 0) {
    Write-Host 'FAIL: install.ps1 has parse errors:' -ForegroundColor Red
    $parseErrors | ForEach-Object { Write-Host ('    ' + $_.ToString()) -ForegroundColor Red }
    exit 1
}

$src = Get-Content -Raw -LiteralPath $installScript

# ---------------------------------------------------------------------------
# Helpers - fake an isolated verifier that mirrors install.ps1's logic.
# Returns one of:
#   match / mismatch / skipped-override / skipped-no-manifest
# Throws on mismatch when the override is not set (mirrors install.ps1's exit 1).
# ---------------------------------------------------------------------------

function Invoke-FakeHashVerify {
    param(
        [string]$ZipPath,
        [string]$Asset,
        [string]$ExpectedHashLowerHex,
        [bool]$Override
    )
    if ($Override) {
        return 'skipped-override'
    }
    if (-not $ExpectedHashLowerHex) {
        return 'skipped-no-manifest'
    }
    $actual = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -eq $ExpectedHashLowerHex.ToLowerInvariant()) {
        return 'match'
    }
    throw ('ARCHIVE INTEGRITY CHECK FAILED expected={0} actual={1}' -f $ExpectedHashLowerHex, $actual)
}

function New-FakeZipAndManifest {
    param([string]$Root, [string]$Name = 'mneme-windows-x64.zip')
    $stage = Join-Path $Root 'zip-stage'
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'bin') | Out-Null
    foreach ($exe in @('mneme.exe', 'mneme-daemon.exe')) {
        ('payload-' + $exe) | Set-Content -LiteralPath (Join-Path $stage "bin\$exe")
    }
    $zip = Join-Path $Root $Name
    if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
    $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    return @{ Zip = $zip; Hash = $hash; Asset = $Name }
}

# Tamper one byte: replicates a TLS MITM body swap or a flipped bit.
function Invoke-TamperOneByte {
    param([string]$ZipPath)
    $bytes = [System.IO.File]::ReadAllBytes($ZipPath)
    if ($bytes.Length -lt 100) { throw ('zip too small to tamper sensibly: ' + $bytes.Length + ' bytes') }
    $idx = [int]([math]::Floor($bytes.Length / 2))
    $bytes[$idx] = $bytes[$idx] -bxor 0xFF
    [System.IO.File]::WriteAllBytes($ZipPath, $bytes)
}

# ---------------------------------------------------------------------------
# Cases.
# ---------------------------------------------------------------------------

$tests = @()

$tests += @{
    Name   = 'param_block_includes_skiphashverify'
    Block  = { return @{ ok = ($src -match '\[switch\]\$SkipHashVerify') } }
    Verify = { param($r) $r.ok }
}

$tests += @{
    Name  = 'envvar_read_present'
    Block = {
        $ok1 = $src -match 'MNEME_SKIP_HASH_VERIFY'
        $ok2 = $src -match 'MNEME_SKIP_HASH_CHECK'
        $ok3 = $src -match 'skipHashVerifyEnv'
        return @{ ok = ($ok1 -and $ok2 -and $ok3) }
    }
    Verify = { param($r) $r.ok }
}

$tests += @{
    Name  = 'verdict_print_present'
    Block = {
        $ok1 = $src -match 'Write-Info \("  asset    : \{0\}"'
        $ok2 = $src -match 'Write-Info \("  expected : \{0\}"'
        $ok3 = $src -match 'Write-Info \("  actual   : \{0\}"'
        $ok4 = $src -match 'verdict: MATCH'
        return @{ ok = ($ok1 -and $ok2 -and $ok3 -and $ok4) }
    }
    Verify = { param($r) $r.ok }
}

$tests += @{
    Name  = 'match_passes'
    Block = {
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('mneme-hashtest-' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        try {
            $f = New-FakeZipAndManifest -Root $tmp
            $verdict = Invoke-FakeHashVerify -ZipPath $f.Zip -Asset $f.Asset -ExpectedHashLowerHex $f.Hash -Override:$false
            return @{ ok = ($verdict -eq 'match'); verdict = $verdict }
        } finally {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        }
    }
    Verify = { param($r) $r.ok }
}

$tests += @{
    Name  = 'tampered_one_byte_rejected'
    Block = {
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('mneme-hashtest-' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        try {
            $f = New-FakeZipAndManifest -Root $tmp
            $expected = $f.Hash
            Invoke-TamperOneByte -ZipPath $f.Zip
            $threw = $false
            $msg = $null
            try {
                $null = Invoke-FakeHashVerify -ZipPath $f.Zip -Asset $f.Asset -ExpectedHashLowerHex $expected -Override:$false
            } catch {
                $threw = $true
                $msg = $_.Exception.Message
            }
            return @{ ok = $threw; threwMessage = $msg }
        } finally {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        }
    }
    Verify = { param($r) $r.ok -and ($r.threwMessage -match 'ARCHIVE INTEGRITY CHECK FAILED') }
}

$tests += @{
    Name  = 'override_skips_tampered'
    Block = {
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('mneme-hashtest-' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        try {
            $f = New-FakeZipAndManifest -Root $tmp
            $expected = $f.Hash
            Invoke-TamperOneByte -ZipPath $f.Zip
            $verdict = Invoke-FakeHashVerify -ZipPath $f.Zip -Asset $f.Asset -ExpectedHashLowerHex $expected -Override:$true
            return @{ ok = ($verdict -eq 'skipped-override'); verdict = $verdict }
        } finally {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        }
    }
    Verify = { param($r) $r.ok }
}

$tests += @{
    Name  = 'no_manifest_warns_continues'
    Block = {
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('mneme-hashtest-' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        try {
            $f = New-FakeZipAndManifest -Root $tmp
            $verdict = Invoke-FakeHashVerify -ZipPath $f.Zip -Asset $f.Asset -ExpectedHashLowerHex $null -Override:$false
            return @{ ok = ($verdict -eq 'skipped-no-manifest'); verdict = $verdict }
        } finally {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        }
    }
    Verify = { param($r) $r.ok }
}

# ---------------------------------------------------------------------------
# Runner: dual-mode (Pester 5 if installed, plain runner otherwise).
# ---------------------------------------------------------------------------

$pesterMod = Get-Module -ListAvailable Pester |
             Where-Object { $_.Version.Major -ge 5 } |
             Sort-Object Version -Descending |
             Select-Object -First 1

if ($pesterMod) {
    Import-Module Pester -MinimumVersion 5.0
    Describe 'install.ps1 SHA-256 verify (HIGH-13)' {
        foreach ($t in $tests) {
            $name   = $t.Name
            $block  = $t.Block
            $verify = $t.Verify
            It $name {
                $result = & $block
                (& $verify $result) | Should -BeTrue
            }
        }
    }
} else {
    Write-Host '==> install.ps1 SHA-256 verify (HIGH-13)' -ForegroundColor Cyan
    $pass = 0
    $fail = 0
    foreach ($t in $tests) {
        try {
            $result = & $t.Block
            $okFlag = & $t.Verify $result
            if ($okFlag) {
                Write-Host ('    [PASS] ' + $t.Name) -ForegroundColor Green
                $pass++
            } else {
                $asJson = if ($result) { $result | ConvertTo-Json -Compress -Depth 3 } else { '<null>' }
                Write-Host ('    [FAIL] ' + $t.Name + ' - result: ' + $asJson) -ForegroundColor Red
                $fail++
            }
        } catch {
            Write-Host ('    [FAIL] ' + $t.Name + ' - threw: ' + $_.Exception.Message) -ForegroundColor Red
            $fail++
        }
    }
    Write-Host ''
    Write-Host ('Result: {0} passed, {1} failed' -f $pass, $fail) -ForegroundColor $(if ($fail -eq 0) { 'Green' } else { 'Red' })
    if ($fail -gt 0) { exit 1 } else { exit 0 }
}
