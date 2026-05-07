param()
$ErrorActionPreference = "Continue"

# S5 -- daemon lifecycle 5 cycles
# Each cycle: start, status, stop, verify clean. No proc leak between cycles.

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
$results = @()

# Pre-cleanup
Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 3
$pre = Get-Process mneme* -ErrorAction SilentlyContinue
if ($pre) {
    Write-Host "WARN: pre-cycle procs not clean: $($pre.Count)"
}

for ($i = 1; $i -le 5; $i++) {
    Write-Host "`n=== Cycle $i ==="
    $cycle = [pscustomobject]@{
        Cycle = $i
        StartOk = $false
        StatusOk = $false
        StopOk = $false
        ProcsAfterStop = -1
        Notes = ""
    }

    # START
    Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
    Start-Sleep 5
    $started = Get-Process mneme-daemon -ErrorAction SilentlyContinue
    $cycle.StartOk = ($started -ne $null)

    # STATUS
    $st = & $mneme daemon status 2>&1
    $stExit = $LASTEXITCODE
    $cycle.StatusOk = ($stExit -eq 0)

    # STOP
    $stopOut = & $mneme daemon stop 2>&1
    $stopExit = $LASTEXITCODE
    Start-Sleep 5

    # Verify all mneme* gone
    $remaining = Get-Process mneme* -ErrorAction SilentlyContinue
    $remCount = if ($remaining) { $remaining.Count } else { 0 }
    $cycle.ProcsAfterStop = $remCount
    $cycle.StopOk = ($stopExit -eq 0 -and $remCount -eq 0)
    if ($remCount -gt 0) {
        $cycle.Notes = ($remaining | ForEach-Object { "$($_.Name)#$($_.Id)" }) -join ","
        # If anything leaked, kill it before next cycle
        $remaining | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep 2
    }
    $results += $cycle
}

Write-Host "`n=== S5 SUMMARY ==="
$results | Format-Table -AutoSize

$cleanCycles = ($results | Where-Object { $_.StartOk -and $_.StatusOk -and $_.StopOk }).Count
Write-Host "Clean cycles: $cleanCycles / 5"
$pass = ($cleanCycles -eq 5)
Write-Host "S5_VERDICT: $(if ($pass) { 'PASS' } else { 'FAIL' })"
