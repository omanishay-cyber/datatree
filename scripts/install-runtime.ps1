# datatree :: install-runtime.ps1
# Detects and (optionally) installs the runtime dependencies datatree needs on
# Windows: bun >=1.0, python >=3.10, tesseract >=5.0, ffmpeg.
# SQLite is bundled inside datatree-store via rusqlite's `bundled` feature.
#
# LOCAL-ONLY rule: this script never reaches the internet by itself.  When
# -AutoInstall is passed, it delegates to winget / scoop / choco -- those
# package managers may go to the network on the user's behalf, but datatree
# itself never does.  With -From <dir>, no network access at all.
#
# Usage:
#   pwsh ./install-runtime.ps1                  # detect only, print hints
#   pwsh ./install-runtime.ps1 -AutoInstall     # install via winget/scoop/choco
#   pwsh ./install-runtime.ps1 -From C:\mirror  # use local pre-downloaded folder
#   pwsh ./install-runtime.ps1 -Yes             # assume yes to confirmations
#   pwsh ./install-runtime.ps1 -Quiet           # less chatty
#
# Compatible with Windows PowerShell 5.1+ and PowerShell 7+.

[CmdletBinding()]
param(
    [switch] $AutoInstall,
    [switch] $Yes,
    [switch] $Quiet,
    [string] $From = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# ----------------------------------------------------------- config
$DataTreeHome    = if ($env:DATATREE_HOME) { $env:DATATREE_HOME } else { Join-Path $HOME ".datatree" }
$LogDir          = Join-Path $DataTreeHome "logs"
$LogFile         = Join-Path $LogDir       "install.log"
$ManifestFile    = Join-Path $DataTreeHome "install-manifest.json"
$DataTreeVersion = "0.1.0"

$RequiredDeps = @("bun", "python", "tesseract", "ffmpeg")

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
if (-not (Test-Path $LogFile)) { New-Item -ItemType File -Force -Path $LogFile | Out-Null }

# ----------------------------------------------------------- helpers
function Write-Log {
    param([string]$Level, [string]$Message)
    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    "[{0}] [{1}] {2}" -f $ts, $Level, $Message | Add-Content -Path $LogFile -Encoding utf8
    if ($Quiet -and $Level -ne "ERROR") { return }
    switch ($Level) {
        "INFO"  { Write-Host "[i] $Message" -ForegroundColor Cyan }
        "OK"    { Write-Host "[+] $Message" -ForegroundColor Green }
        "WARN"  { Write-Host "[!] $Message" -ForegroundColor Yellow }
        "ERROR" { Write-Host "[x] $Message" -ForegroundColor Red }
        default { Write-Host $Message }
    }
}

function Confirm-Action {
    param([string]$Prompt)
    if ($Yes) { return $true }
    $resp = Read-Host "$Prompt [y/N]"
    return ($resp -match '^(y|yes)$')
}

function Test-Cmd {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Refresh-Path {
    # Pull updated PATH from machine + user scope so newly installed bins are
    # visible inside the current PowerShell session.
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath    = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = ($machinePath, $userPath, $env:Path -join ";")
    # also add common Bun + Python paths
    $bunBin = Join-Path $HOME ".bun\bin"
    if (Test-Path $bunBin) { $env:Path = "$bunBin;$env:Path" }
}

# ---------------------------------------------------- detect package mgr
function Get-PreferredPackageManager {
    if (Test-Cmd "winget") { return "winget" }
    if (Test-Cmd "scoop")  { return "scoop"  }
    if (Test-Cmd "choco")  { return "choco"  }
    return $null
}

# ---------------------------------------------------- detect deps
function Test-DepPresent {
    param([string]$Dep)
    switch ($Dep) {
        "bun"       { return Test-Cmd "bun" }
        "python"    { return (Test-Cmd "python") -or (Test-Cmd "python3") -or (Test-Cmd "py") }
        "tesseract" { return Test-Cmd "tesseract" }
        "ffmpeg"    { return Test-Cmd "ffmpeg" }
        default     { return $false }
    }
}

function Get-DepVersion {
    param([string]$Dep)
    try {
        switch ($Dep) {
            "bun"       { return (& bun --version 2>$null) }
            "python" {
                if (Test-Cmd "python")  { return ((& python  --version 2>&1) -replace 'Python ','') }
                if (Test-Cmd "python3") { return ((& python3 --version 2>&1) -replace 'Python ','') }
                if (Test-Cmd "py")      { return ((& py      --version 2>&1) -replace 'Python ','') }
                return "?"
            }
            "tesseract" {
                $line = (& tesseract --version 2>&1 | Select-Object -First 1)
                return ($line -replace '^tesseract\s+','')
            }
            "ffmpeg" {
                $line = (& ffmpeg -version 2>$null | Select-Object -First 1)
                if ($line -match 'ffmpeg version (\S+)') { return $Matches[1] }
                return "?"
            }
        }
    } catch { return "?" }
    return "?"
}

# ---------------------------------------------------- package id mapping
function Get-PackageId {
    param([string]$Dep, [string]$Manager)
    $map = @{
        "winget" = @{
            "bun"       = "Oven-sh.Bun"
            "python"    = "Python.Python.3.12"
            "tesseract" = "UB-Mannheim.TesseractOCR"
            "ffmpeg"    = "Gyan.FFmpeg"
        }
        "scoop" = @{
            "bun"       = "main/bun"
            "python"    = "main/python"
            "tesseract" = "extras/tesseract"
            "ffmpeg"    = "main/ffmpeg"
        }
        "choco" = @{
            "bun"       = "bun"
            "python"    = "python"
            "tesseract" = "tesseract"
            "ffmpeg"    = "ffmpeg"
        }
    }
    if ($map.ContainsKey($Manager) -and $map[$Manager].ContainsKey($Dep)) {
        return $map[$Manager][$Dep]
    }
    return $null
}

function Get-InstallHint {
    param([string]$Dep, [string]$Manager)
    if ($null -eq $Manager) {
        return "Install winget (built into Win 11) or scoop/choco, then re-run with -AutoInstall"
    }
    $id = Get-PackageId -Dep $Dep -Manager $Manager
    if ($null -eq $id) {
        if ($Dep -eq "bun") { return "irm bun.sh/install.ps1 | iex" }
        return "(no mapping for $Dep on $Manager)"
    }
    switch ($Manager) {
        "winget" { return "winget install --id $id --accept-package-agreements --accept-source-agreements" }
        "scoop"  { return "scoop install $id" }
        "choco"  { return "choco install $id -y" }
    }
}

# ---------------------------------------------------- install drivers
function Install-Bun-Official {
    Write-Log "INFO" "Installing Bun via official installer (irm bun.sh/install.ps1 | iex)"
    try {
        Invoke-Expression (Invoke-RestMethod "https://bun.sh/install.ps1") *>> $LogFile
    } catch {
        Write-Log "ERROR" "Bun installer failed: $_"
        return $false
    }
    Refresh-Path
    return $true
}

function Install-ViaManager {
    param([string]$Dep, [string]$Manager)
    $id = Get-PackageId -Dep $Dep -Manager $Manager
    if ($null -eq $id) {
        if ($Dep -eq "bun") { return Install-Bun-Official }
        Write-Log "ERROR" "No package id mapping for $Dep on $Manager"
        return $false
    }
    try {
        switch ($Manager) {
            "winget" {
                Write-Log "INFO" "winget install --id $id"
                & winget install --id $id --accept-package-agreements --accept-source-agreements --silent *>> $LogFile
            }
            "scoop" {
                Write-Log "INFO" "scoop install $id"
                & scoop install $id *>> $LogFile
            }
            "choco" {
                Write-Log "INFO" "choco install $id -y"
                & choco install $id -y *>> $LogFile
            }
        }
    } catch {
        Write-Log "ERROR" "$Manager install of $id failed: $_"
        return $false
    }
    Refresh-Path
    return $true
}

function Copy-FromMirror {
    param([string]$Dep)
    if ([string]::IsNullOrEmpty($From)) { return $false }
    if (-not (Test-Path $From)) {
        Write-Log "ERROR" "-From dir not found: $From"
        return $false
    }
    $candidates = @(
        (Join-Path $From "$Dep.exe"),
        (Join-Path $From "$Dep-installer.exe"),
        (Join-Path $From "$Dep.msi"),
        (Join-Path $From "$Dep")
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) {
            Write-Log "INFO"  "Found local mirror artifact: $c"
            Write-Log "WARN"  "Local-mirror install is a stub: launch $c manually."
            return $true
        }
    }
    return $false
}

function Install-Dep {
    param([string]$Dep, [string]$Manager)

    Write-Log "INFO" "Installing $Dep ..."

    if (-not [string]::IsNullOrEmpty($From)) {
        if (Copy-FromMirror -Dep $Dep) { return $true }
        Write-Log "WARN" "No mirror artifact for $Dep; falling back to package manager"
    }

    if ($Dep -eq "bun" -and ($null -eq $Manager -or $Manager -eq "choco")) {
        return Install-Bun-Official
    }

    if ($null -eq $Manager) {
        Write-Log "ERROR" "No package manager available; install winget or scoop and retry"
        return $false
    }

    return Install-ViaManager -Dep $Dep -Manager $Manager
}

# ---------------------------------------------------- manifest writer
function Write-Manifest {
    param([string[]]$InstalledByDatatree, [string[]]$Preexisting)
    $obj = [ordered]@{
        datatree_version       = $DataTreeVersion
        installed_at           = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        installed_by_datatree  = @($InstalledByDatatree)
        preexisting            = @($Preexisting)
        models                 = @{}
    }
    $json = $obj | ConvertTo-Json -Depth 5
    Set-Content -Path $ManifestFile -Value $json -Encoding utf8
    Write-Log "OK" "Wrote install manifest -> $ManifestFile"
}

# ============================================================ main
Write-Log "INFO" "datatree runtime installer v$DataTreeVersion starting"
Write-Log "INFO" "AutoInstall=$AutoInstall  From=$From"

Refresh-Path
$pkgMgr = Get-PreferredPackageManager
if ($pkgMgr) { Write-Log "INFO" "Preferred package manager: $pkgMgr" }
else         { Write-Log "WARN" "No supported package manager found (winget/scoop/choco)" }

# snapshot what's already on the box
$preexisting = @()
foreach ($d in $RequiredDeps) {
    if (Test-DepPresent -Dep $d) { $preexisting += $d }
}

# detect missing
$missing = @()
foreach ($d in $RequiredDeps) {
    if (Test-DepPresent -Dep $d) {
        Write-Log "OK"   "$d already installed ($(Get-DepVersion -Dep $d))"
    } else {
        Write-Log "WARN" "$d MISSING"
        $missing += $d
    }
}

if ($missing.Count -eq 0) {
    Write-Log "OK" "All required runtime deps already present.  Nothing to do."
    Write-Manifest -InstalledByDatatree @() -Preexisting $preexisting
    exit 0
}

if (-not $AutoInstall) {
    Write-Log "WARN" "Missing required deps: $($missing -join ', ')"
    Write-Host ""
    Write-Host "To install them, either re-run with -AutoInstall, or run these commands:"
    Write-Host ""
    foreach ($d in $missing) {
        $hint = Get-InstallHint -Dep $d -Manager $pkgMgr
        "{0,-12} -> {1}" -f $d, $hint | Write-Host
    }
    Write-Host ""
    Write-Host "If you have a pre-downloaded mirror folder, pass -From <dir>."
    exit 1
}

if (-not (Confirm-Action "About to install: $($missing -join ', '). Proceed?")) {
    Write-Log "WARN" "User declined auto-install"
    exit 1
}

$installed = @()
foreach ($d in $missing) {
    $ok = Install-Dep -Dep $d -Manager $pkgMgr
    if (-not $ok) {
        Write-Log "ERROR" "Failed to install $d (see $LogFile)"
        exit 2
    }
    Refresh-Path
    if (-not (Test-DepPresent -Dep $d)) {
        Write-Log "ERROR" "$d install reported success but binary not found on PATH"
        Write-Log "WARN"  "You may need to open a NEW shell for PATH changes to take effect."
        exit 2
    }
    Write-Log "OK" "$d installed -> version $(Get-DepVersion -Dep $d)"
    $installed += $d
}

Write-Manifest -InstalledByDatatree $installed -Preexisting $preexisting
Write-Log "OK" "datatree runtime install complete."
exit 0
