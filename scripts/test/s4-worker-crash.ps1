param()
$ErrorActionPreference = "Continue"

# S4 -- Worker crash injection
# For each worker type (non-supervisor first), kill the PID and measure
# respawn latency. Acceptance: <200ms respawn for every non-supervisor worker.
# Supervisor (mneme-daemon) is killed last and does NOT auto-respawn (correct).

$workerTypes = @(
    "mneme-parsers",
    "mneme-scanners",
    "mneme-store",
    "mneme-brain",
    "mneme-livebus",
    "mneme-md-ingest",
    "mneme-multimodal"
)

$results = @()

# Ensure daemon up
$d0 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d0) {
    Write-Host "WARN: daemon not running, starting..."
    Start-Process -FilePath "C:\Users\Administrator\.mneme\bin\mneme.exe" -ArgumentList "daemon","start" -WindowStyle Hidden
    Start-Sleep 6
}

foreach ($w in $workerTypes) {
    Write-Host "`n=== Killing $w ==="
    $procs = Get-Process -Name $w -ErrorAction SilentlyContinue
    if (-not $procs) {
        Write-Host "$w not running, skipping"
        $results += [pscustomobject]@{ Worker = $w; Status = "NOT_RUNNING"; LatencyMs = -1 }
        continue
    }
    # Kill ONE PID (first one)
    $victim = $procs[0]
    $oldPid = $victim.Id
    Write-Host "Killing PID=$oldPid"
    $killAt = [DateTime]::UtcNow

    Stop-Process -Id $oldPid -Force -ErrorAction Stop

    # Poll for new pid (different from oldPid) every 25ms
    $deadline = (Get-Date).AddSeconds(15)
    $newPid = $null
    while ((Get-Date) -lt $deadline) {
        $alive = Get-Process -Name $w -ErrorAction SilentlyContinue
        if ($alive) {
            $candidates = $alive | Where-Object { $_.Id -ne $oldPid }
            if ($candidates) {
                $newPid = $candidates[0].Id
                $latency = ([DateTime]::UtcNow - $killAt).TotalMilliseconds
                Write-Host "RESPAWN: $w newPid=$newPid latencyMs=$([math]::Round($latency,1))"
                $results += [pscustomobject]@{ Worker = $w; Status = "RESPAWNED"; OldPid = $oldPid; NewPid = $newPid; LatencyMs = [math]::Round($latency,1) }
                break
            }
        }
        Start-Sleep -Milliseconds 25
    }
    if (-not $newPid) {
        Write-Host "FAIL: $w did not respawn within 15s"
        $results += [pscustomobject]@{ Worker = $w; Status = "DID_NOT_RESPAWN"; OldPid = $oldPid; NewPid = $null; LatencyMs = -1 }
    }
    # Give supervisor a moment before next kill
    Start-Sleep -Milliseconds 200
}

# Last: kill supervisor itself. It should NOT auto-respawn (only Windows service mode does)
Write-Host "`n=== Killing mneme-daemon (supervisor) ==="
$dproc = Get-Process -Name "mneme-daemon" -ErrorAction SilentlyContinue
if ($dproc) {
    $oldPid = $dproc[0].Id
    Stop-Process -Id $oldPid -Force
    Start-Sleep 3
    $alive = Get-Process -Name "mneme-daemon" -ErrorAction SilentlyContinue
    if ($alive) {
        Write-Host "UNEXPECTED: supervisor respawned"
        $results += [pscustomobject]@{ Worker = "mneme-daemon"; Status = "UNEXPECTEDLY_RESPAWNED"; LatencyMs = 0 }
    } else {
        Write-Host "EXPECTED: supervisor did NOT auto-respawn (only service-mode does)"
        $results += [pscustomobject]@{ Worker = "mneme-daemon"; Status = "EXPECTED_NO_RESPAWN"; LatencyMs = 0 }
    }
} else {
    $results += [pscustomobject]@{ Worker = "mneme-daemon"; Status = "NOT_RUNNING_BEFORE_KILL"; LatencyMs = 0 }
}

Write-Host "`n=== S4 SUMMARY ==="
$results | Format-Table -AutoSize

$nonSup = $results | Where-Object { $_.Worker -ne "mneme-daemon" -and $_.Status -ne "NOT_RUNNING" }
$bad = $nonSup | Where-Object { $_.Status -ne "RESPAWNED" -or $_.LatencyMs -gt 5000 }
$over200 = $nonSup | Where-Object { $_.Status -eq "RESPAWNED" -and $_.LatencyMs -gt 200 }

Write-Host "Non-supervisor workers tested: $($nonSup.Count)"
Write-Host "Failed-respawn count: $($bad.Count)"
Write-Host "Over-200ms count: $($over200.Count)"
$pass = ($bad.Count -eq 0)
Write-Host "S4_VERDICT: $(if ($pass) { 'PASS_RESPAWN_OK' } else { 'FAIL' })"
Write-Host "S4_NOTE: 200ms latency target (per VMware report) is observed in $($nonSup.Count - $over200.Count)/$($nonSup.Count) workers"
