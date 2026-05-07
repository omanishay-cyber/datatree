# install-hooks-default-on.tests.ps1
#
# Bug B regression: install.ps1 step 7/8 must invoke `mneme install` (which
# defaults to writing hooks per the K1 fix in v0.3.2), NOT `mneme register-mcp`
# (which hardcodes `skip_hooks: true` internally -- see
# cli/src/commands/register_mcp.rs:87, the implicit "--skip-hooks" the
# postmortem refers to). Result of the bug: phase6_smoke shows
# settings_hook_count=0 and the persistent-memory pipeline stays empty.
#
# It also asserts the script body does NOT pass any explicit --skip-hooks /
# --no-hooks anywhere outside comments, so a future refactor can't sneak the
# flag back in.
#
# Plan reference: docs/superpowers/plans/2026-04-29-mneme-12-bug-fix.md task B.
#
# WILL RUN ON EC2 -- Pester 3.4.0 is also available on the local AWS test instance.

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceRoot  = Resolve-Path (Join-Path $ScriptDir '..\..')
$InstallPs1  = Join-Path $SourceRoot 'scripts\install.ps1'

# Strip PowerShell line comments (`# ...`) so the assertions don't trip on
# documentation. Keep block-comment / string-literal handling minimal --
# install.ps1 doesn't use here-strings or `<# #>` blocks for these terms.
function Get-NonCommentBody {
    param([string]$Path)
    $lines = Get-Content $Path
    $body = @()
    foreach ($line in $lines) {
        # Skip pure-comment lines (optional leading whitespace then `#`).
        if ($line -match '^\s*#') { continue }
        # Strip trailing inline comments (` # ...` after some code). This is
        # imperfect for `#` inside strings, but install.ps1 doesn't quote
        # any of the flag tokens we look for.
        $code = ($line -split '\s+#', 2)[0]
        $body += $code
    }
    return ($body -join "`n")
}

Describe 'Bug B -- install.ps1 step 7/8 invokes `mneme install` so K1 hooks register' {
    $body = Get-NonCommentBody -Path $InstallPs1

    It 'install.ps1 exists' {
        Test-Path $InstallPs1 | Should Be $true
    }

    It 'install.ps1 invokes `mneme install` (K1 default-on hooks path) at step 7/8' {
        # Match the actual invocation: `& $MnemeBin install ...` -- register-mcp
        # bypasses the hooks pipeline so we expect `install` instead.
        ($body -match '\$MnemeBin\s+install\b') | Should Be $true
    }

    It 'install.ps1 does NOT invoke `mneme register-mcp` (the hooks-bypass path)' {
        # register-mcp hardcodes skip_hooks: true (cli/src/commands/register_mcp.rs:87)
        ($body -match '\$MnemeBin\s+register-mcp\b') | Should Be $false
    }

    It 'install.ps1 does NOT pass --skip-hooks anywhere in code' {
        ($body -match '--skip-hooks\b') | Should Be $false
    }

    It 'install.ps1 does NOT pass --no-hooks anywhere in code' {
        ($body -match '--no-hooks\b') | Should Be $false
    }

    It 'install.ps1 keeps --skip-manifest (the install.ps1 invariant)' {
        # The script still wants to leave CLAUDE.md / AGENTS.md alone; only
        # hooks + mcp entries should be written from this path.
        ($body -match '--skip-manifest\b') | Should Be $true
    }

    It 'install.ps1 post-install banner reflects hooks-on (no scary "hooks NOT registered" warning)' {
        # The K1 fix banner should communicate success, not the legacy warning.
        $banner = Get-Content $InstallPs1 -Raw
        ($banner -match 'Hooks registered|hooks_registered|persistent-memory pipeline live|hooks default-on') | Should Be $true
    }
}
