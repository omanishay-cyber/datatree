# datatree :: uninstall-runtime.ps1
# Removes the runtime dependencies datatree installed on Windows.  Reads
# ~/.datatree/install-manifest.json to distinguish deps installed by
# datatree from deps that already existed on the machine.
#
# Flags:
#   -KeepShared    (default ON)  do NOT remove anything in "preexisting"
#   -RemoveShared                explicit opt-in to also remove preexisting
#   -Yes                          skip confirmation prompts
#   -DryRun                       print plan only

[CmdletBinding()]
param(
    [switch] $KeepShared = $true,
    [switch] $RemoveShared,
    [switch] $Yes,
    [switch] $DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$DataTreeHome = if ($env:DATATREE_HOME) { $env:DATATREE_HOME } else { Join-Path $HOME ".datatree" }
$LogDir       = Join-Path $DataTreeHome "logs"
$LogFile      = Join-Path $LogDir       "install.log"
$ManifestFile = Join-Path $DataTreeHome "install-manifest.json"

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
if (-not (Test-Path $LogFile)) { New-Item -ItemType File -Force -Path $LogFile | Out-Null }

if ($RemoveShared) { $KeepShared = $false }

function Write-Log {
    param([string]$Level, [string]$Message)
    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    "[{0}] [UNINSTALL] [{1}] {2}" -f $ts, $Level, $Message | Add-Content -Path $LogFile -Encoding utf8
    switch ($Level) {
        "INFO"  { Write-Host "[uninst] $Message" -ForegroundColor Cyan }
        "OK"    { Write-Host "[uninst] $Message" -ForegroundColor Green }
        "WARN"  { Write-Host "[uninst] $Message" -ForegroundColor Yellow }
        "ERROR" { Write-Host "[uninst] $Message" -ForegroundColor Red }
    }
}
function Confirm-Action {
    param([string]$Prompt)
    if ($Yes) { return $true }
    $r = Read-Host "$Prompt [y/N]"
    return ($r -match '^(y|yes)$')
}

if (-not (Test-Path $ManifestFile)) {
    Write-Log "ERROR" "Install manifest not found: $ManifestFile"
    Write-Log "ERROR" "Either datatree was never installed, or DATATREE_HOME was wiped."
    exit 1
}

try {
    $manifest = Get-Content -Raw -Path $ManifestFile | ConvertFrom-Json
} catch {
    Write-Log "ERROR" "Manifest is not valid JSON: $_"
    exit 1
}

$installedByDatatree = @()
if ($manifest.installed_by_datatree) { $installedByDatatree = @($manifest.installed_by_datatree) }
$preexisting = @()
if ($manifest.preexisting) { $preexisting = @($manifest.preexisting) }

Write-Log "INFO" ("manifest: installed_by_datatree=[{0}]  preexisting=[{1}]" -f ($installedByDatatree -join ','), ($preexisting -join ','))

# build target set
$targets = @($installedByDatatree)
if (-not $KeepShared) { $targets += $preexisting }
$targets = $targets | Where-Object { $_ } | Select-Object -Unique

if ($targets.Count -eq 0) {
    Write-Log "OK" "Nothing to uninstall."
    exit 0
}

Write-Log "INFO" "Will remove: $($targets -join ', ')"
if ($DryRun) { Write-Log "INFO" "Dry run -- exiting without changes."; exit 0 }
if (-not (Confirm-Action "Proceed with removal?")) {
    Write-Log "WARN" "User declined."
    exit 0
}

# detect package manager (in priority order)
function Get-PreferredManager {
    if (Get-Command winget -ErrorAction SilentlyContinue) { return "winget" }
    if (Get-Command scoop  -ErrorAction SilentlyContinue) { return "scoop"  }
    if (Get-Command choco  -ErrorAction SilentlyContinue) { return "choco"  }
    return $null
}
$mgr = Get-PreferredManager
Write-Log "INFO" "Using manager: $mgr"

function Remove-Dep {
    param([string]$Dep)

    # Bun is special: usually under $HOME\.bun
    $bunHome = Join-Path $HOME ".bun"
    if ($Dep -eq "bun" -and (Test-Path $bunHome)) {
        try {
            Remove-Item -Recurse -Force $bunHome
            return $true
        } catch {
            Write-Log "ERROR" "Failed to remove $bunHome : $_"
            return $false
        }
    }

    if ($null -eq $mgr) {
        Write-Log "ERROR" "No package manager available; cannot remove $Dep"
        return $false
    }

    $idMap = @{
        "winget" = @{
            "bun"       = "Oven-sh.Bun"
            "python"    = "Python.Python.3.12"
            "tesseract" = "UB-Mannheim.TesseractOCR"
            "ffmpeg"    = "Gyan.FFmpeg"
        }
        "scoop" = @{
            "bun" = "bun"; "python" = "python"; "tesseract" = "tesseract"; "ffmpeg" = "ffmpeg"
        }
        "choco" = @{
            "bun" = "bun"; "python" = "python"; "tesseract" = "tesseract"; "ffmpeg" = "ffmpeg"
        }
    }
    $key = $Dep.ToLower()
    if (-not $idMap[$mgr].ContainsKey($key)) {
        Write-Log "ERROR" "No removal mapping for $Dep on $mgr"
        return $false
    }
    $id = $idMap[$mgr][$key]
    try {
        switch ($mgr) {
            "winget" {
                Write-Log "INFO" "winget uninstall --id $id"
                & winget uninstall --id $id --silent *>> $LogFile
            }
            "scoop"  {
                Write-Log "INFO" "scoop uninstall $id"
                & scoop uninstall $id *>> $LogFile
            }
            "choco"  {
                Write-Log "INFO" "choco uninstall $id -y"
                & choco uninstall $id -y *>> $LogFile
            }
        }
    } catch {
        Write-Log "ERROR" "$mgr uninstall of $id failed: $_"
        return $false
    }
    return $true
}

$removed = @()
$failed  = @()
foreach ($d in $targets) {
    Write-Log "INFO" "Removing $d ..."
    if (Remove-Dep -Dep $d) {
        Write-Log "OK"   "  removed $d"
        $removed += $d
    } else {
        Write-Log "ERROR" "  failed to remove $d"
        $failed += $d
    }
}

# update manifest
if ($removed.Count -gt 0) {
    $newInstalled = $installedByDatatree | Where-Object { $removed -notcontains $_ }
    $newPreexisting = $preexisting
    if (-not $KeepShared) {
        $newPreexisting = $preexisting | Where-Object { $removed -notcontains $_ }
    }
    $obj = [ordered]@{
        datatree_version       = if ($manifest.datatree_version) { $manifest.datatree_version } else { "0.1.0" }
        installed_at           = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        installed_by_datatree  = @($newInstalled)
        preexisting            = @($newPreexisting)
        models                 = if ($manifest.models) { $manifest.models } else { @{} }
    }
    Set-Content -Path $ManifestFile -Encoding utf8 -Value ($obj | ConvertTo-Json -Depth 5)
    Write-Log "OK" "Manifest updated."
}

if ($failed.Count -gt 0) {
    Write-Log "ERROR" "Some removals failed: $($failed -join ', ')"
    exit 2
}
Write-Log "OK" "Uninstall complete."
exit 0
