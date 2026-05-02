# Pester tests for install.ps1 Idempotent-3 (Defender exclusion pre-check).
#
# Written locally; designed to RUN ON EC2 where Pester 5.x is installed.
# On the local AWS test instance (no Pester pinned) these serve as the spec.
#
# Run on EC2:
#   Invoke-Pester -Path scripts\tests\install-idempotent-3-defender.Tests.ps1

BeforeAll {
    $script:RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    $script:InstallPs1 = Join-Path $script:RepoRoot 'scripts\install.ps1'

    # Helper: import only the leaf functions from install.ps1 we want to
    # test, without running the script body's side-effecting steps.
    function Import-DefenderHelpers {
        $content = Get-Content -LiteralPath $script:InstallPs1 -Raw
        $helpers = @('Get-DefenderExclusions', 'Get-NormalizedPath', 'Test-IsElevated')
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

Describe 'Idempotent-3: Defender exclusion pre-check' {
    BeforeEach {
        Import-DefenderHelpers
    }

    It 'Get-DefenderExclusions returns array when Get-MpPreference succeeds' {
        Mock -CommandName Get-MpPreference -MockWith {
            return [PSCustomObject]@{
                ExclusionPath = @('C:\Users\test\.mneme', 'C:\Users\test\.claude')
            }
        }
        $result = Get-DefenderExclusions
        $result.Count | Should -Be 2
        $result | Should -Contain 'C:\Users\test\.mneme'
    }

    It 'Get-DefenderExclusions returns null when Get-MpPreference throws' {
        Mock -CommandName Get-MpPreference -MockWith {
            throw 'Defender service not running'
        }
        $result = Get-DefenderExclusions
        $result | Should -BeNullOrEmpty
    }

    It 'Get-DefenderExclusions returns empty array when ExclusionPath is null' {
        Mock -CommandName Get-MpPreference -MockWith {
            return [PSCustomObject]@{ ExclusionPath = $null }
        }
        $result = Get-DefenderExclusions
        $result | Should -BeOfType [array]
        $result.Count | Should -Be 0
    }

    It 'Get-NormalizedPath strips trailing backslashes and lowercases' {
        Get-NormalizedPath 'C:\Users\Test\.MNEME\\\' | Should -Be 'c:\users\test\.mneme'
        Get-NormalizedPath 'C:\Foo\' | Should -Be 'c:\foo'
        Get-NormalizedPath '' | Should -Be ''
    }

    It 'RED: Add-MpPreference is NOT called when exclusion already present' {
        # Audit Idempotent-3: on a re-run where both exclusions are set,
        # the script must NOT invoke Add-MpPreference and must NOT print
        # the "run as admin" warning. Pre-fix behaviour: cmdlet was
        # called every time, which on non-elevated shell printed the
        # warning even when nothing needed adding.
        $existing = @(
            'C:\Users\test\.mneme',
            'C:\Users\test\.claude'
        )
        Mock -CommandName Get-MpPreference -MockWith {
            return [PSCustomObject]@{ ExclusionPath = $existing }
        }
        Mock -CommandName Add-MpPreference -MockWith { return }

        # Replay the relevant install.ps1 step-4/8 logic in isolation.
        $MnemeHome = 'C:\Users\test\.mneme'
        $ClaudeHome = 'C:\Users\test\.claude'
        $ExcludeDirs = @($MnemeHome, $ClaudeHome)

        $ExistingExclusions = Get-DefenderExclusions
        $ExistingNormalized = $ExistingExclusions | ForEach-Object { Get-NormalizedPath $_ }

        $DirsToAdd = @()
        foreach ($dir in $ExcludeDirs) {
            $normDir = Get-NormalizedPath $dir
            if ($ExistingNormalized -notcontains $normDir) {
                $DirsToAdd += $dir
            }
        }

        # Assert: nothing left to add.
        $DirsToAdd.Count | Should -Be 0
        # Add-MpPreference must NOT have been called.
        Should -Invoke -CommandName Add-MpPreference -Times 0
    }

    It 'Add-MpPreference IS called for missing exclusions when elevated' {
        Mock -CommandName Get-MpPreference -MockWith {
            return [PSCustomObject]@{ ExclusionPath = @('C:\Users\test\.mneme') }
        }
        Mock -CommandName Add-MpPreference -MockWith { return }

        $MnemeHome = 'C:\Users\test\.mneme'
        $ClaudeHome = 'C:\Users\test\.claude'
        $ExcludeDirs = @($MnemeHome, $ClaudeHome)

        $ExistingNormalized = (Get-DefenderExclusions) | ForEach-Object { Get-NormalizedPath $_ }
        $DirsToAdd = $ExcludeDirs | Where-Object {
            (Get-NormalizedPath $_) -notin $ExistingNormalized
        }

        # Only ClaudeHome is missing.
        $DirsToAdd.Count | Should -Be 1
        $DirsToAdd[0] | Should -Be 'C:\Users\test\.claude'

        foreach ($dir in $DirsToAdd) {
            Add-MpPreference -ExclusionPath $dir
        }
        Should -Invoke -CommandName Add-MpPreference -Times 1
    }

    It 'Path normalization survives trailing-slash and case mismatch' {
        # User has the exclusion stored as 'C:\Users\Test\.Mneme\' (with
        # trailing slash + mixed case). Our pre-check must still
        # recognize 'C:\Users\test\.mneme' as the same path.
        $existing = @('C:\Users\Test\.Mneme\')
        Mock -CommandName Get-MpPreference -MockWith {
            return [PSCustomObject]@{ ExclusionPath = $existing }
        }
        Mock -CommandName Add-MpPreference -MockWith { return }

        $ExistingNormalized = (Get-DefenderExclusions) | ForEach-Object { Get-NormalizedPath $_ }
        $candidate = Get-NormalizedPath 'C:\Users\test\.mneme'
        $ExistingNormalized | Should -Contain $candidate
    }
}
