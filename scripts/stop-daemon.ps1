# Stop the datatree Windows service or Task Scheduler task.
[CmdletBinding()]
param([switch]$Quiet)

$ErrorActionPreference = 'Continue'
function Write-Log([string]$msg) { if (-not $Quiet) { Write-Host "[datatree-stop] $msg" } }

$ServiceName = 'DatatreeDaemon'

$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    if ($svc.Status -eq 'Stopped') {
        Write-Log "Service $ServiceName already stopped"
    } else {
        & sc.exe stop $ServiceName | Out-Null
        Write-Log "Service $ServiceName stopped"
    }
}

$task = Get-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
if ($task) {
    try { Stop-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue } catch {}
    Write-Log "Scheduled task $ServiceName stopped"
}

# also kill any stray supervisor process
Get-Process -Name 'datatree-supervisor' -ErrorAction SilentlyContinue | ForEach-Object {
    try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch {}
    Write-Log ("Killed datatree-supervisor pid {0}" -f $_.Id)
}

exit 0
