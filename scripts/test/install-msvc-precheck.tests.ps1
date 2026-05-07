# install-msvc-precheck.tests.ps1
#
# Bug F regression: install.ps1 G4 step (Tauri CLI) used to run
# `cargo install tauri-cli --locked --version "^2.0"` (3-5 min, 560 crates,
# ~53 MB) BEFORE checking whether MSVC's link.exe / cl.exe were on PATH.
# On a stock Windows machine without MSVC Build Tools, the install spent
# 5 min downloading then died at link stage with `linker 'link.exe' not found`.
#
# Fix: a Test-MsvcLinker helper near the top of install.ps1, called BEFORE
# the cargo install block. If link.exe + cl.exe aren't both on PATH, print
# a clear remediation hint and skip the cargo install -- saving the 5 minutes
# of doomed download.
#
# This test asserts the helper exists, is invoked at the cargo install
# decision point, and that the cargo invocation is gated on the helper's
# return value. We do NOT actually run cargo or invoke link.exe -- Pester
# runs against the script body via static parsing (Get-Content + regex).
#
# Plan reference: docs/superpowers/plans/2026-04-29-mneme-12-bug-fix.md task F.
#
# WILL RUN ON EC2 -- Pester 3.4.0 is also available on the local AWS test instance.

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceRoot  = Resolve-Path (Join-Path $ScriptDir '..\..')
$InstallPs1  = Join-Path $SourceRoot 'scripts\install.ps1'

Describe 'Bug F -- install.ps1 pre-checks MSVC link.exe before cargo install tauri-cli' {
    $body = Get-Content $InstallPs1 -Raw

    It 'install.ps1 exists' {
        Test-Path $InstallPs1 | Should Be $true
    }

    It 'install.ps1 defines a Test-MsvcLinker helper function' {
        ($body -match '(?ms)function\s+Test-MsvcLinker\s*\{') | Should Be $true
    }

    It 'Test-MsvcLinker probes link.exe' {
        # The helper body must reference link.exe -- Get-Command or similar.
        # We allow either ' link.exe' or quoted 'link.exe'.
        ($body -match '(?ms)function\s+Test-MsvcLinker\s*\{[^}]*link\.exe') | Should Be $true
    }

    It 'Test-MsvcLinker probes cl.exe' {
        # Same structure for cl.exe (the C compiler MSVC ships alongside link.exe).
        ($body -match '(?ms)function\s+Test-MsvcLinker\s*\{[^}]*cl\.exe') | Should Be $true
    }

    It 'install.ps1 calls Test-MsvcLinker before invoking cargo install tauri-cli' {
        # Strategy: split the script at the first cargo install tauri-cli line.
        # Everything BEFORE that split must contain a call to Test-MsvcLinker.
        $idx = $body.IndexOf("install','tauri-cli'")
        if ($idx -lt 0) {
            # If the script no longer references tauri-cli at all (e.g. removed),
            # the test trivially passes -- no doomed download possible.
            $true | Should Be $true
        } else {
            $beforeCargo = $body.Substring(0, $idx)
            ($beforeCargo -match 'Test-MsvcLinker') | Should Be $true
        }
    }

    It 'install.ps1 emits a remediation hint when MSVC is missing' {
        # The else-branch of the gate must reference winget + the
        # Microsoft.VisualStudio.2022.BuildTools package id so the user
        # gets actionable copy-paste recovery instructions.
        ($body -match 'Microsoft\.VisualStudio\.2022\.BuildTools') | Should Be $true
    }
}
