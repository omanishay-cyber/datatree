# Start the mneme Windows service (or Task Scheduler task as fallback).
[CmdletBinding()]
param([switch]$Quiet)

$ErrorActionPreference = 'Stop'
function Write-Log([string]$msg) { if (-not $Quiet) { Write-Host "[mneme-start] $msg" } }

$ServiceName = 'MnemeDaemon'

$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    if ($svc.Status -eq 'Running') {
        Write-Log "Service $ServiceName already running"
        exit 0
    }
    & sc.exe start $ServiceName | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[mneme-start] sc.exe start failed (exit $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Log "Service $ServiceName started"
    exit 0
}

# fallback: scheduled task
$task = Get-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
if ($task) {
    Start-ScheduledTask -TaskName $ServiceName
    Write-Log "Scheduled task $ServiceName started"
    exit 0
}

# fallback: spawn directly
$MnemeHome = if ($env:MNEME_HOME) { $env:MNEME_HOME } else { Join-Path $env:USERPROFILE '.mneme' }
$Bin = Join-Path $MnemeHome 'bin\mneme-supervisor.exe'
if (-not (Test-Path $Bin)) {
    Write-Host "[mneme-start] mneme-supervisor.exe not installed at $Bin" -ForegroundColor Red
    exit 1
}
$LogDir = Join-Path $MnemeHome 'logs'
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }

Start-Process -FilePath $Bin -ArgumentList '--daemon' `
    -RedirectStandardOutput (Join-Path $LogDir 'supervisor.out.log') `
    -RedirectStandardError  (Join-Path $LogDir 'supervisor.err.log') `
    -WindowStyle Hidden | Out-Null
Write-Log "mneme daemon started (background process)"
exit 0
