# mneme — standalone PowerShell uninstaller (K19 / Phase A recovery)
#
# Self-contained recovery script. Does NOT depend on mneme.exe being
# functional — usable when the binary is broken, missing flags, or
# bricked by an interrupted install.
#
# Run via:
#   powershell -ExecutionPolicy Bypass -File "$HOME\.mneme\uninstall.ps1"
#
# What it does:
#   1. Taskkill the daemon + worker processes
#   2. Drop ~/.mneme/bin from the User PATH (registry-backed)
#   3. Remove Defender exclusions for ~/.mneme and ~/.claude (best-effort)
#   4. Strip mneme-marked entries from ~/.claude/settings.json hooks
#   5. Remove ~/.mneme/ entirely (or all-but-projects/ with -KeepShards)
#
# What it does NOT do:
#   - Touch the user's Claude Code creds (~/.claude/.credentials.json)
#   - Remove unrelated Claude Code config (only mneme-marked hook entries)
#
# Mneme Personal-Use License v1.0. (c) 2026 Anish Trivedi & Kruti Trivedi.

[CmdletBinding()]
param(
    [switch]$KeepShards,   # leave per-project shards in ~/.mneme/projects/ in place
    [switch]$DryRun,       # print what would happen, don't execute
    [switch]$Quiet         # suppress status output
)

$ErrorActionPreference = 'Continue'
$home_root = $env:USERPROFILE
$mneme_dir = Join-Path $home_root '.mneme'
$claude_dir = Join-Path $home_root '.claude'

function Step($msg) { if (-not $Quiet) { Write-Host "==> $msg" -ForegroundColor Cyan } }
function Ok($msg)   { if (-not $Quiet) { Write-Host "    $msg" -ForegroundColor Green } }
function Warn($msg) { if (-not $Quiet) { Write-Host "    WARN: $msg" -ForegroundColor Yellow } }

if ($DryRun -and -not $Quiet) {
    Write-Host "[DRY RUN] No changes will be made." -ForegroundColor Magenta
}

# 1. Stop daemon + workers (best effort; ok if not running)
Step "stopping mneme processes"
$proc_names = @(
    'mneme-daemon',
    'mneme-store',
    'mneme-parsers',
    'mneme-scanners',
    'mneme-livebus',
    'mneme-md-ingest',
    'mneme-brain',
    'mneme-multimodal',
    'mneme'
)
foreach ($n in $proc_names) {
    $running = Get-Process -Name $n -ErrorAction SilentlyContinue
    if ($running) {
        if ($DryRun) {
            Warn "would taskkill $n ($($running.Count) instance(s))"
        } else {
            Stop-Process -Name $n -Force -ErrorAction SilentlyContinue
            Ok "killed $n"
        }
    }
}

# 2. Remove ~/.mneme/bin from User PATH
Step "cleaning User PATH"
$mneme_bin = (Join-Path $mneme_dir 'bin').ToLower()
$user_path = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($user_path) {
    $entries = $user_path -split ';' | Where-Object {
        $_ -and
        $_.ToLower().TrimEnd('\') -ne $mneme_bin.TrimEnd('\') -and
        $_.ToLower() -notlike '*\.mneme\bin*'
    }
    $new_path = ($entries -join ';')
    if ($new_path -ne $user_path) {
        if ($DryRun) {
            Warn "would set User PATH = (original minus ~/.mneme/bin entries)"
        } else {
            [Environment]::SetEnvironmentVariable('Path', $new_path, 'User')
            Ok "removed ~/.mneme/bin from User PATH"
        }
    } else {
        Ok "User PATH already clean"
    }
}

# 3. Defender exclusions
Step "removing Defender exclusions"
foreach ($p in @($mneme_dir, $claude_dir)) {
    if ($DryRun) {
        Warn "would Remove-MpPreference -ExclusionPath '$p'"
    } else {
        try {
            Remove-MpPreference -ExclusionPath $p -ErrorAction SilentlyContinue
            Ok "removed exclusion for $p"
        } catch {
            Warn "Defender removal failed for $p (run as admin if needed)"
        }
    }
}

# 4. Strip mneme-marked hook entries from ~/.claude/settings.json
Step "stripping mneme hook entries from ~/.claude/settings.json"
$settings = Join-Path $claude_dir 'settings.json'
if (Test-Path $settings) {
    if ($DryRun) {
        Warn "would strip _mneme.managed=true entries from settings.json hooks"
    } else {
        try {
            $raw = Get-Content $settings -Raw -Encoding UTF8
            # UTF-8 BOM tolerance
            if ($raw.Length -gt 0 -and $raw[0] -eq [char]0xFEFF) { $raw = $raw.Substring(1) }
            $obj = $raw | ConvertFrom-Json
            if ($obj.hooks) {
                $touched = $false
                foreach ($evt in @($obj.hooks.PSObject.Properties.Name)) {
                    $arr = $obj.hooks.$evt
                    if ($arr -is [array]) {
                        $kept = @($arr | Where-Object {
                            -not ($_._mneme -and $_._mneme.managed -eq $true)
                        })
                        if ($kept.Count -ne $arr.Count) {
                            $touched = $true
                            if ($kept.Count -eq 0) {
                                $obj.hooks.PSObject.Properties.Remove($evt)
                            } else {
                                $obj.hooks.$evt = $kept
                            }
                        }
                    }
                }
                if ($touched) {
                    # Backup then write
                    $bak = "$settings.mneme-uninstall-$(Get-Date -Format 'yyyyMMdd-HHmmss').bak"
                    Copy-Item $settings $bak -Force
                    $obj | ConvertTo-Json -Depth 32 | Set-Content $settings -Encoding UTF8
                    Ok "stripped mneme hooks (backup: $bak)"
                } else {
                    Ok "no mneme hooks present"
                }
            } else {
                Ok "no hooks key in settings.json"
            }
        } catch {
            Warn "could not parse settings.json: $($_.Exception.Message)"
        }
    }
} else {
    Ok "no settings.json present"
}

# 5. Remove ~/.mneme/
Step "removing ~/.mneme/"
if (Test-Path $mneme_dir) {
    if ($KeepShards) {
        Get-ChildItem -Path $mneme_dir -Force | Where-Object { $_.Name -ne 'projects' } | ForEach-Object {
            if ($DryRun) {
                Warn "would delete $($_.FullName)"
            } else {
                Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        Ok "removed ~/.mneme/* (kept projects/)"
    } else {
        if ($DryRun) {
            Warn "would delete entire $mneme_dir"
        } else {
            Remove-Item -LiteralPath $mneme_dir -Recurse -Force -ErrorAction SilentlyContinue
            if (Test-Path $mneme_dir) {
                Warn "could not fully delete (locked file?). Retry after closing all mneme processes."
            } else {
                Ok "deleted $mneme_dir"
            }
        }
    }
} else {
    Ok "~/.mneme/ already absent"
}

if (-not $Quiet) {
    Write-Host ""
    Write-Host "mneme uninstall complete." -ForegroundColor Green
    Write-Host "Claude Code creds at ~/.claude/.credentials.json were NOT touched." -ForegroundColor Green
}
exit 0
