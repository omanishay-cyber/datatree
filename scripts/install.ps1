# Mneme — one-line installer for Windows
#
# Usage (PowerShell):
#   iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install.ps1 | iex
#
# What it does:
#   1. Downloads mneme-windows-x64.zip from the latest GitHub release.
#   2. Extracts to %USERPROFILE%\.mneme\ (bin/, mcp/, plugin/).
#   3. Adds the bin directory to the user PATH (persistent).
#   4. Prints next steps.
#
# Safe to re-run.

$ErrorActionPreference = 'Stop'

$Repo       = 'omanishay-cyber/mneme'
$Asset      = 'mneme-windows-x64.zip'
$MnemeHome  = Join-Path $env:USERPROFILE '.mneme'
$BinDir     = Join-Path $MnemeHome 'bin'

Write-Host "mneme: one-line install starting"
Write-Host ("mneme: target directory = {0}" -f $MnemeHome)

# --- fetch latest release metadata --------------------------------------------

$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
Write-Host ("mneme: fetching release metadata from {0}" -f $ApiUrl)

try {
    $Headers = @{ 'User-Agent' = 'mneme-installer' }
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers
} catch {
    Write-Host ("mneme: failed to reach GitHub API: {0}" -f $_.Exception.Message) -ForegroundColor Red
    exit 1
}

$AssetEntry = $Release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1
if ($null -eq $AssetEntry) {
    Write-Host ("mneme: {0} not yet attached to release {1}." -f $Asset, $Release.tag_name) -ForegroundColor Yellow
    Write-Host "       the release workflow may still be building -- retry in ~15 min."
    exit 1
}

# --- download -----------------------------------------------------------------

$Tmp = Join-Path $env:TEMP ("mneme-install-{0}" -f ([System.Guid]::NewGuid().ToString('N').Substring(0, 8)))
New-Item -ItemType Directory -Path $Tmp -Force | Out-Null
$ZipPath = Join-Path $Tmp $Asset

Write-Host ("mneme: downloading {0} ({1:N1} MB)" -f $AssetEntry.name, ($AssetEntry.size / 1MB))
try {
    Invoke-WebRequest -Uri $AssetEntry.browser_download_url -OutFile $ZipPath -UseBasicParsing -Headers $Headers
} catch {
    Write-Host ("mneme: download failed: {0}" -f $_.Exception.Message) -ForegroundColor Red
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
    exit 1
}

# --- extract ------------------------------------------------------------------

if (-not (Test-Path $MnemeHome)) {
    New-Item -ItemType Directory -Path $MnemeHome -Force | Out-Null
}

Write-Host ("mneme: extracting to {0}" -f $MnemeHome)
try {
    Expand-Archive -Path $ZipPath -DestinationPath $MnemeHome -Force
} catch {
    Write-Host ("mneme: extract failed: {0}" -f $_.Exception.Message) -ForegroundColor Red
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
    exit 1
}

Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue

# --- PATH ---------------------------------------------------------------------

$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($null -eq $UserPath) { $UserPath = '' }

if (-not ($UserPath.Split(';') -contains $BinDir)) {
    $NewPath = if ([string]::IsNullOrEmpty($UserPath)) { $BinDir } else { "$UserPath;$BinDir" }
    [Environment]::SetEnvironmentVariable('PATH', $NewPath, 'User')
    $env:PATH = "$env:PATH;$BinDir"
    Write-Host ("mneme: added {0} to user PATH (persistent)" -f $BinDir)
} else {
    Write-Host "mneme: bin already in PATH"
}

# --- done ---------------------------------------------------------------------

Write-Host ""
Write-Host "mneme: installed. Next steps:" -ForegroundColor Green
Write-Host "  1. mneme-daemon start    # start the supervisor"
Write-Host "  2. mneme build .         # index this project"
Write-Host "  3. mneme install         # register with your AI tool"
Write-Host ""
Write-Host ("mneme: open a NEW terminal so the updated PATH takes effect.") -ForegroundColor Yellow
