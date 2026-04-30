param(
    [int]$IdleSeconds = 60,
    [int]$LoadCalls = 1000
)
$ErrorActionPreference = "Continue"

# S7 — Memory + thread idle-return (THE leak gate)
# Phase A: capture baseline at idle
# Phase B: hammer status under load
# Phase C: capture under-load
# Phase D: idle for $IdleSeconds seconds
# Phase E: capture final
# Acceptance: |E - A| WS <= 1MB, threads return to baseline (or +1 max)
#
# Per feedback_leak_is_the_leak.md: ANY plateau without return is a leak.

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"

function Get-DaemonMetrics {
    $p = Get-Process mneme-daemon -ErrorAction SilentlyContinue
    if (-not $p) { return $null }
    # Use the supervisor (highest PID handle? — nope, just first). All PIDs returned will be same proc family
    $sup = $p[0]
    return [pscustomobject]@{
        Pid = $sup.Id
        WorkingSetMB = [math]::Round($sup.WorkingSet64 / 1MB, 3)
        Threads = $sup.Threads.Count
        Handles = $sup.HandleCount
    }
}

# Ensure daemon running
$d0 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d0) {
    Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
    Start-Sleep 6
}

# Phase A: baseline
Write-Host "=== S7 Phase A: idle baseline (waiting 30s for steady state) ==="
Start-Sleep 30
$A = Get-DaemonMetrics
Write-Host ("A: PID={0} WS={1}MB threads={2} handles={3}" -f $A.Pid, $A.WorkingSetMB, $A.Threads, $A.Handles)

# Phase B: hammer status
Write-Host "=== S7 Phase B: hammer 'mneme daemon status' x$LoadCalls ==="
$loadStart = Get-Date
for ($i = 0; $i -lt $LoadCalls; $i++) {
    & $mneme daemon status > $null 2>&1
}
$loadWall = (Get-Date) - $loadStart
Write-Host ("Load wall: {0}s, ~{1} calls/sec" -f [math]::Round($loadWall.TotalSeconds,2), [math]::Round($LoadCalls/$loadWall.TotalSeconds,1))

# Phase C: under-load capture (right after load completes)
$C = Get-DaemonMetrics
Write-Host ("C: PID={0} WS={1}MB threads={2} handles={3}" -f $C.Pid, $C.WorkingSetMB, $C.Threads, $C.Handles)

# Phase D: idle for $IdleSeconds
Write-Host "=== S7 Phase D: idle for $IdleSeconds seconds ==="
Start-Sleep $IdleSeconds

# Phase E: capture after idle return
$E = Get-DaemonMetrics
Write-Host ("E: PID={0} WS={1}MB threads={2} handles={3}" -f $E.Pid, $E.WorkingSetMB, $E.Threads, $E.Handles)

# Verdict
$wsDelta = $E.WorkingSetMB - $A.WorkingSetMB
$threadDelta = $E.Threads - $A.Threads
$handleDelta = $E.Handles - $A.Handles
$wsPass = [Math]::Abs($wsDelta) -le 1.0
$threadPass = $threadDelta -le 1
$pidStable = $E.Pid -eq $A.Pid

Write-Host "`n=== S7 SUMMARY ==="
Write-Host ("WS delta E-A: {0:F3} MB (acceptance: |delta|<=1.0 MB) -> {1}" -f $wsDelta, $(if ($wsPass) { 'OK' } else { 'FAIL' }))
Write-Host ("Threads delta: {0} (acceptance: <=+1) -> {1}" -f $threadDelta, $(if ($threadPass) { 'OK' } else { 'FAIL' }))
Write-Host ("Handles delta: {0} (informational only)" -f $handleDelta)
Write-Host ("PID stable: $pidStable (must be true)")
Write-Host ("WS load peak: C={0:F3} MB" -f $C.WorkingSetMB)

$pass = $wsPass -and $threadPass -and $pidStable
Write-Host "S7_VERDICT: $(if ($pass) { 'PASS_LEAK_GATE' } else { 'FAIL_LEAK_DETECTED' })"
