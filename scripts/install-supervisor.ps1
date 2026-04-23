# datatree supervisor installer (Windows PowerShell 5.1+)
# Installs datatree-supervisor.exe to %USERPROFILE%\.datatree\bin\
# Registers a Windows service via sc.exe (DatatreeDaemon).
# Falls back to a Task Scheduler entry at user logon if elevation is denied.
# Idempotent: re-running does not duplicate entries; existing files .bak'd.

[CmdletBinding()]
param(
    [string]$BinaryPath = "",
    [string]$SourceDir  = "",
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
$MarkerVersion = 'v1.0'

function Write-Log([string]$msg) {
    if (-not $Quiet) { Write-Host "[datatree-install] $msg" }
}

function Die([string]$msg) {
    Write-Host "[datatree-install] ERROR: $msg" -ForegroundColor Red
    exit 1
}

# --- detect arch -------------------------------------------------------------
$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { Die "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
}
Write-Log "Detected platform: windows/$arch"

# --- paths -------------------------------------------------------------------
$DatatreeHome = if ($env:DATATREE_HOME) { $env:DATATREE_HOME } else { Join-Path $env:USERPROFILE '.datatree' }
$BinDir = Join-Path $DatatreeHome 'bin'
$LogDir = Join-Path $DatatreeHome 'logs'

foreach ($d in @($DatatreeHome, $BinDir, $LogDir, (Join-Path $DatatreeHome 'projects'), (Join-Path $DatatreeHome 'cache'), (Join-Path $DatatreeHome 'models'))) {
    if (-not (Test-Path $d)) { New-Item -ItemType Directory -Force -Path $d | Out-Null }
}

# --- resolve binary ----------------------------------------------------------
if (-not $BinaryPath) {
    if (-not $SourceDir) {
        $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
        $SourceDir = Join-Path $ScriptDir '..\dist\supervisor'
    }
    $BinaryPath = Join-Path $SourceDir "datatree-supervisor-windows-$arch.exe"
}

if (-not (Test-Path $BinaryPath)) { Die "Binary not found: $BinaryPath" }

$Dest = Join-Path $BinDir 'datatree-supervisor.exe'
if (Test-Path $Dest) {
    Write-Log "Backing up existing binary to $Dest.bak"
    Copy-Item -Path $Dest -Destination "$Dest.bak" -Force
}
Write-Log "Installing binary to $Dest"
Copy-Item -Path $BinaryPath -Destination $Dest -Force

# --- service registration ----------------------------------------------------
$ServiceName = 'DatatreeDaemon'
$DisplayName = 'Datatree Supervisor (datatree-marker ' + $MarkerVersion + ')'

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p  = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Register-Service {
    $existing = & sc.exe query $ServiceName 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Log "Service $ServiceName already exists; reconfiguring (idempotent)"
        & sc.exe stop   $ServiceName | Out-Null
        & sc.exe delete $ServiceName | Out-Null
        Start-Sleep -Milliseconds 500
    }

    $binPath = '"' + $Dest + '" --daemon'
    & sc.exe create $ServiceName binPath= $binPath start= auto DisplayName= $DisplayName | Out-Null
    if ($LASTEXITCODE -ne 0) { return $false }

    & sc.exe description $ServiceName "Datatree per-user knowledge graph daemon" | Out-Null
    & sc.exe failure     $ServiceName reset= 86400 actions= restart/5000/restart/5000/restart/10000 | Out-Null
    Write-Log "Service registered: $ServiceName"
    return $true
}

function Register-ScheduledTask {
    $taskName = 'DatatreeDaemon'
    $existing = & schtasks.exe /Query /TN $taskName 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Log "Scheduled task $taskName exists; deleting for reconfigure"
        & schtasks.exe /Delete /TN $taskName /F | Out-Null
    }

    $tr = '"' + $Dest + '" --daemon'
    & schtasks.exe /Create /TN $taskName /TR $tr /SC ONLOGON /RL LIMITED /F | Out-Null
    if ($LASTEXITCODE -ne 0) { Die "Failed to register scheduled task" }
    Write-Log "Scheduled task registered (runs at user logon): $taskName"
}

if (Test-Admin) {
    if (-not (Register-Service)) {
        Write-Log "Service registration failed; falling back to Task Scheduler"
        Register-ScheduledTask
    }
} else {
    Write-Log "Not running as admin; using Task Scheduler fallback"
    Register-ScheduledTask
}

Write-Log "Install complete. Run scripts/start-daemon.ps1 to launch."
exit 0
