param()
$ErrorActionPreference = "Continue"

# S6 -- SSH disconnect survival
# Logic: capture daemon PID before this script's plink session ends.
# A subsequent plink session checks if mneme-daemon is alive.
#
# This script's job is JUST: clean state -> start daemon -> capture PID -> exit.
# A second plink call after this exits checks survival.

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"

Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 3

Write-Host "=== Starting daemon ==="
Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
Start-Sleep 5

$d = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d) {
    Write-Host "S6_FAIL_AT_START: daemon not up after start"
    exit 1
}
$capturedPid = $d[0].Id
Write-Host "DAEMON_PID=$capturedPid"
$capturedPid | Out-File -FilePath "C:\Users\Administrator\s6-pid.txt" -Encoding ASCII

# Process tree snapshot
Write-Host "=== Pre-disconnect proc tree ==="
Get-Process mneme* -ErrorAction SilentlyContinue | Format-Table Name,Id -AutoSize

Write-Host "S6_PHASE1_OK: daemon started, this plink session about to exit"
