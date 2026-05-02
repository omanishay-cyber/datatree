# install-sqlite-fallback.tests.ps1
#
# Bug G regression: install.ps1:720 hardcoded
# `https://www.sqlite.org/2025/sqlite-tools-win-x64-3470100.zip` — postmortem
# §3.G captured a live `(404) Not Found`. sqlite.org rotates filenames every
# release, so any hardcoded URL eventually breaks.
#
# Fix: G7 must try winget primary (`winget install --id SQLite.SQLite`), and
# if winget is unavailable or fails, fall back to a list of candidate
# sqlite.org URLs each gated by a HEAD probe (`Invoke-WebRequest -Method Head`)
# before downloading. The bare hardcoded `Invoke-WebRequest` to a single URL
# without HEAD verification is the failure mode this test guards.
#
# Plan reference: docs/superpowers/plans/2026-04-29-mneme-12-bug-fix.md task G.
#
# WILL RUN ON EC2 — Pester 3.4.0 is also available on the local AWS test instance.

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceRoot  = Resolve-Path (Join-Path $ScriptDir '..\..')
$InstallPs1  = Join-Path $SourceRoot 'scripts\install.ps1'

Describe 'Bug G — install.ps1 G7 SQLite install: winget primary, HEAD-probed fallback' {
    $body = Get-Content $InstallPs1 -Raw

    It 'install.ps1 exists' {
        Test-Path $InstallPs1 | Should Be $true
    }

    It 'G7 attempts winget for SQLite.SQLite as primary path' {
        # The fix should run `winget install --id SQLite.SQLite ...` BEFORE
        # the portable-zip fallback. We accept either `& winget` or `winget`
        # invocations against the `SQLite.SQLite` package id.
        ($body -match 'winget\s+install[^\r\n]*SQLite\.SQLite') | Should Be $true
    }

    It 'G7 uses --silent and --accept-*-agreements flags so winget is non-interactive' {
        # winget install requires explicit accept flags or the call hangs.
        ($body -match 'accept-source-agreements') | Should Be $true
        ($body -match 'accept-package-agreements') | Should Be $true
    }

    It 'G7 HEAD-probes candidate sqlite.org URLs before downloading' {
        # The fallback must not blindly download — it should HEAD-probe each
        # candidate URL and only download from one that returns 200.
        ($body -match '-Method\s+Head') | Should Be $true
    }

    It 'G7 candidate URL list has at least 2 entries (current + 1 future-proof)' {
        # At minimum: the current 2025 URL plus one forward-looking URL so the
        # script survives sqlite.org's annual filename rotation.
        $sqliteUrls = ([regex]::Matches($body, 'https://www\.sqlite\.org/\d{4}/sqlite-tools-[\w\-\.]+\.zip')).Count
        $sqliteUrls -ge 2 | Should Be $true
    }

    It 'G7 does NOT bare-download a hardcoded URL without HEAD probe' {
        # The literal failure mode: Invoke-WebRequest to a single hardcoded
        # $sqliteUrl with no probe. We allow Invoke-WebRequest as long as
        # the URL came from a probed list (asserted by the HEAD-probe test
        # above) — but the OLD assignment pattern
        # `$sqliteUrl = 'https://...sqlite-tools-...'` followed
        # IMMEDIATELY by Invoke-WebRequest to that URL is the smell.
        #
        # We assert the FALLBACK path uses a probed-URL variable, not a
        # bare assignment. Heuristic: the script should pick the URL from
        # an array (either via foreach loop or .Where()).
        ($body -match '\$sqliteUrls\s*=\s*@\(') | Should Be $true
    }

    It 'G7 emits a clear warning if both winget and the portable fallback fail' {
        # On total failure the user must learn `mneme cache du` etc. still work,
        # and SQLite is optional — match any "manual" / "skip" / "optional" hint.
        ($body -match '(?ms)\[G7\][^"]*(install manually|skipping|optional)') | Should Be $true
    }
}
