# Start the datatree Windows service (or Task Scheduler task as fallback).
[CmdletBinding()]
param([switch]$Quiet)

$ErrorActionPreference = 'Stop'
function Write-Log([string]$msg) { if (-not $Quiet) { Write-Host "[datatree-start] $msg" } }

$ServiceName = 'DatatreeDaemon'

$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    if ($svc.Status -eq 'Running') {
        Write-Log "Service $ServiceName already running"
        exit 0
    }
    & sc.exe start $ServiceName | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[datatree-start] sc.exe start failed (exit $LASTEXITCODE)" -ForegroundColor Red
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
$DatatreeHome = if ($env:MNEME_HOME) { $env:MNEME_HOME } else { Join-Path $env:USERPROFILE '.datatree' }
$Bin = Join-Path $DatatreeHome 'bin\datatree-supervisor.exe'
if (-not (Test-Path $Bin)) {
    Write-Host "[datatree-start] datatree-supervisor.exe not installed at $Bin" -ForegroundColor Red
    exit 1
}
$LogDir = Join-Path $DatatreeHome 'logs'
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }

Start-Process -FilePath $Bin -ArgumentList '--daemon' `
    -RedirectStandardOutput (Join-Path $LogDir 'supervisor.out.log') `
    -RedirectStandardError  (Join-Path $LogDir 'supervisor.err.log') `
    -WindowStyle Hidden | Out-Null
Write-Log "datatree daemon started (background process)"
exit 0
