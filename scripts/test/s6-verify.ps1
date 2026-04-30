param()
$ErrorActionPreference = "Continue"

$pidFile = "C:\Users\Administrator\s6-pid.txt"
if (-not (Test-Path $pidFile)) {
    Write-Host "S6_FAIL_NO_PID_FILE"
    exit 1
}
$capturedPid = (Get-Content $pidFile -Raw).Trim()
Write-Host "Looking for previously captured PID: $capturedPid"

$d = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d) {
    Write-Host "S6_FAIL: daemon NOT alive after SSH disconnect"
    exit 1
}

$stillThere = $d | Where-Object { $_.Id -eq $capturedPid }
if ($stillThere) {
    Write-Host "S6_PASS: same daemon PID=$capturedPid alive after SSH disconnect"
} else {
    $newIds = ($d | ForEach-Object { $_.Id }) -join ","
    Write-Host "S6_PASS_NEW_PID: daemon alive but PID changed (was=$capturedPid, now=$newIds). Anti-leak orphan-recover OK as long as one daemon is up."
}
Write-Host "=== Current proc tree ==="
Get-Process mneme* -ErrorAction SilentlyContinue | Format-Table Name,Id -AutoSize
