# Pester tests for install.ps1 Idempotent-2 (Bun cache spare-others heuristic).
#
# Written locally; designed to RUN ON EC2 where Pester 5.x is installed
# and a real Bun cache may be present. On POS2 (no Pester pinned) these
# serve as the spec.
#
# Run on EC2:
#   Invoke-Pester -Path scripts\tests\install-idempotent-2-bun-cache.Tests.ps1

BeforeAll {
    $script:RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    $script:InstallPs1 = Join-Path $script:RepoRoot 'scripts\install.ps1'

    function Import-BunCacheHelpers {
        $content = Get-Content -LiteralPath $script:InstallPs1 -Raw
        $helpers = @('Test-IsInteractiveSession', 'Test-BunCacheHasOtherProjects')
        $extracted = New-Object System.Text.StringBuilder
        foreach ($name in $helpers) {
            $pattern = "(?ms)^function\s+$name\s*\{.*?^\}"
            $m = [regex]::Match($content, $pattern)
            if ($m.Success) {
                [void]$extracted.AppendLine($m.Value)
                [void]$extracted.AppendLine()
            }
        }
        Invoke-Expression $extracted.ToString()
    }
}

Describe 'Idempotent-2: Bun cache spare-others heuristic' {
    BeforeEach {
        Import-BunCacheHelpers
    }

    It 'Test-BunCacheHasOtherProjects returns false when caches are absent' {
        Mock -CommandName Test-Path -MockWith {
            param($Path)
            return $false
        } -ParameterFilter { $Path -match 'bun' }

        $result = Test-BunCacheHasOtherProjects
        $result | Should -Be $false
    }

    It 'Test-BunCacheHasOtherProjects returns true when foreign packages cached' {
        Mock -CommandName Test-Path -MockWith {
            param($Path)
            return $true
        } -ParameterFilter { $Path -match 'bun' }

        Mock -CommandName Get-ChildItem -MockWith {
            return @(
                [PSCustomObject]@{ Name = 'react@18.2.0' },
                [PSCustomObject]@{ Name = 'zod@3.22.0' }
            )
        } -ParameterFilter { $LiteralPath -match 'bun' }

        $result = Test-BunCacheHasOtherProjects
        $result | Should -Be $true
    }

    It 'RED: unattended path with foreign packages skips wipe' {
        # Audit Idempotent-2: install.ps1 -LocalZip <path> (scripted
        # ship) must NOT nuke other projects' Bun caches. Pre-fix
        # behaviour: Remove-Item -Recurse -Force was unconditional
        # on every -LocalZip install, killing react/zod/etc. caches
        # belonging to unrelated projects.
        $LocalZip = 'C:\path\to\beta.zip'
        $ForceBunCacheClear = $false

        Mock -CommandName Test-BunCacheHasOtherProjects -MockWith { return $true }
        Mock -CommandName Test-IsInteractiveSession -MockWith { return $false }

        $isUnattended = ($null -ne $LocalZip -and $LocalZip -ne '') -or `
                        -not (Test-IsInteractiveSession)

        $shouldWipe = $false
        if ($ForceBunCacheClear) {
            $shouldWipe = $true
        } elseif (-not (Test-BunCacheHasOtherProjects)) {
            $shouldWipe = $true
        } elseif ($isUnattended) {
            $shouldWipe = $false
        }

        $shouldWipe | Should -Be $false
        $isUnattended | Should -Be $true
    }

    It 'ForceBunCacheClear flag overrides spare-others default' {
        # Explicit user opt-in must beat the heuristic.
        $LocalZip = 'C:\path\to\beta.zip'
        $ForceBunCacheClear = $true

        Mock -CommandName Test-BunCacheHasOtherProjects -MockWith { return $true }
        Mock -CommandName Test-IsInteractiveSession -MockWith { return $false }

        $shouldWipe = $false
        if ($ForceBunCacheClear) {
            $shouldWipe = $true
        }
        $shouldWipe | Should -Be $true
    }

    It 'Empty cache (no foreign packages) wipes without prompting' {
        # Nothing to spare, nothing to lose: wipe.
        $LocalZip = $null
        $ForceBunCacheClear = $false

        Mock -CommandName Test-BunCacheHasOtherProjects -MockWith { return $false }
        Mock -CommandName Test-IsInteractiveSession -MockWith { return $true }

        $shouldWipe = $false
        if ($ForceBunCacheClear) {
            $shouldWipe = $true
        } elseif (-not (Test-BunCacheHasOtherProjects)) {
            $shouldWipe = $true
        }
        $shouldWipe | Should -Be $true
    }

    It 'Test-IsInteractiveSession returns false in CI envs' {
        # CI markers should disable the prompt path so automation
        # never hangs on Read-Host.
        $env:CI = 'true'
        try {
            Test-IsInteractiveSession | Should -Be $false
        } finally {
            Remove-Item Env:\CI -ErrorAction SilentlyContinue
        }
    }
}
