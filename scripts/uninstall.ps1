# Uninstall mneme on Windows.
# KEEPS user data unless --purge is passed.
[CmdletBinding()]
param([switch]$Purge, [switch]$Quiet)

$ErrorActionPreference = 'Continue'
function Write-Log([string]$msg) { if (-not $Quiet) { Write-Host "[mneme-uninstall] $msg" } }

$ServiceName  = 'MnemeDaemon'
$MnemeHome = if ($env:MNEME_HOME) { $env:MNEME_HOME } else { Join-Path $env:USERPROFILE '.mneme' }
$BinDir       = Join-Path $MnemeHome 'bin'

# stop first
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$StopScript = Join-Path $ScriptDir 'stop-daemon.ps1'
if (Test-Path $StopScript) {
    try { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $StopScript -Quiet } catch {}
}

# remove service
$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    & sc.exe delete $ServiceName | Out-Null
    Write-Log "Removed service $ServiceName"
}

# remove scheduled task
$task = Get-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
if ($task) {
    try { Unregister-ScheduledTask -TaskName $ServiceName -Confirm:$false } catch {}
    Write-Log "Removed scheduled task $ServiceName"
}

# remove binaries
if (Test-Path $BinDir) {
    Remove-Item -Recurse -Force $BinDir
    Write-Log "Removed $BinDir"
}

$LogsDir = Join-Path $MnemeHome 'logs'
if (Test-Path $LogsDir) { Remove-Item -Recurse -Force $LogsDir }
$PidFile = Join-Path $MnemeHome 'supervisor.pid'
if (Test-Path $PidFile) { Remove-Item -Force $PidFile }

if ($Purge) {
    Write-Log "WARNING: --Purge will delete projects/, cache/, models/"
    foreach ($sub in @('projects','cache','models')) {
        $p = Join-Path $MnemeHome $sub
        if (Test-Path $p) { Remove-Item -Recurse -Force $p }
    }
    if ((Test-Path $MnemeHome) -and -not (Get-ChildItem $MnemeHome -Force)) {
        Remove-Item -Force $MnemeHome
    }
    Write-Log "User data purged"
} else {
    Write-Log "User data preserved at $MnemeHome (projects/, cache/, models/)"
    Write-Log "Run uninstall.ps1 -Purge to delete it."
}
exit 0
