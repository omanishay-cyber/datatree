# install-localzip-canonical.tests.ps1
#
# Bug A regression: INSTALL.md, VERSION.txt, and START-HERE.md must all reference
# the SAME canonical zip filename (mneme-v0.3.2-windows-x64.zip). If INSTALL.md
# drifts to v0.3.0, following the doc verbatim makes install.ps1 fetch v0.3.0
# from GitHub Releases and overwrite the freshly-extracted v0.3.2.
#
# Plan reference: docs/superpowers/plans/2026-04-29-mneme-12-bug-fix.md task A.
#
# WILL RUN ON EC2 -- Pester 3.4.0 is also available on the local AWS test instance, so this also runs
# locally. The test reads files from BOTH the source tree and (when present)
# the parent bundle dir; on a fresh install layout both are present.

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceRoot  = Resolve-Path (Join-Path $ScriptDir '..\..')
$BundleRoot  = Resolve-Path (Join-Path $SourceRoot '..') -ErrorAction SilentlyContinue

$CanonicalZip = 'mneme-v0.3.2-windows-x64.zip'
$WrongZipV030 = 'mneme-v0.3.0-windows-x64.zip'

Describe 'Bug A -- install docs reference canonical v0.3.2 zip filename' {

    Context 'INSTALL.md (in source tree, committable)' {
        $installMd = Join-Path $SourceRoot 'INSTALL.md'

        It 'INSTALL.md exists' {
            Test-Path $installMd | Should Be $true
        }

        It 'INSTALL.md references canonical v0.3.2 zip filename' {
            $content = Get-Content $installMd -Raw
            $content.Contains($CanonicalZip) | Should Be $true
        }

        It 'INSTALL.md does NOT reference stale v0.3.0 zip filename' {
            $content = Get-Content $installMd -Raw
            $content.Contains($WrongZipV030) | Should Be $false
        }

        It 'INSTALL.md documents the -LocalZip canonical install path' {
            $content = Get-Content $installMd -Raw
            $content.Contains('-LocalZip') | Should Be $true
        }
    }

    Context 'VERSION.txt (in bundle root, parent of source)' {
        $versionTxt = if ($BundleRoot) { Join-Path $BundleRoot 'VERSION.txt' } else { $null }
        $hasVersion = $versionTxt -and (Test-Path $versionTxt)

        It 'VERSION.txt references canonical v0.3.2 zip filename' -Skip:(-not $hasVersion) {
            $content = Get-Content $versionTxt -Raw
            $content.Contains($CanonicalZip) | Should Be $true
        }

        It 'VERSION.txt does NOT reference stale v0.3.0 zip filename' -Skip:(-not $hasVersion) {
            $content = Get-Content $versionTxt -Raw
            $content.Contains($WrongZipV030) | Should Be $false
        }

        It 'VERSION.txt documents the -LocalZip canonical install path' -Skip:(-not $hasVersion) {
            $content = Get-Content $versionTxt -Raw
            $content.Contains('-LocalZip') | Should Be $true
        }
    }

    Context 'START-HERE.md (in bundle root, parent of source)' {
        $startHere  = if ($BundleRoot) { Join-Path $BundleRoot 'START-HERE.md' } else { $null }
        $hasStart   = $startHere -and (Test-Path $startHere)

        It 'START-HERE.md references canonical v0.3.2 zip filename' -Skip:(-not $hasStart) {
            $content = Get-Content $startHere -Raw
            $content.Contains($CanonicalZip) | Should Be $true
        }

        It 'START-HERE.md does NOT reference stale v0.3.0 zip filename' -Skip:(-not $hasStart) {
            $content = Get-Content $startHere -Raw
            $content.Contains($WrongZipV030) | Should Be $false
        }
    }
}
