param()
$ErrorActionPreference = "Continue"
# Run after VM comes back. Verify: daemon not auto-started, but starts cleanly, shards intact.
$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
$results = @()
function Add-Result {
    param($Name, $Status, $Detail)
    $script:results += [pscustomobject]@{ name = $Name; status = $Status; detail = $Detail }
}

$pre = Get-Content "C:\Users\Administrator\re3-prereboot.json" -Raw | ConvertFrom-Json

$bootTime = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime
Add-Result "Re3-vm-rebooted" "INFO" "boot_time=$bootTime"

# Verify daemon NOT auto-started
$d0 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if ($d0) {
    Add-Result "Re3-no-auto-start" "FAIL" "daemon auto-started -- v0.3 contract says it should NOT"
} else {
    Add-Result "Re3-no-auto-start" "PASS" "daemon did not auto-start -- correct per v0.3 contract"
}

# Verify shards intact
$shardsCountAfter = if (Test-Path "$env:USERPROFILE\.mneme\projects") { (Get-ChildItem "$env:USERPROFILE\.mneme\projects" -Directory -ErrorAction SilentlyContinue).Count } else { 0 }
$shardsSizeAfter = if (Test-Path "$env:USERPROFILE\.mneme\projects") { ((Get-ChildItem "$env:USERPROFILE\.mneme\projects" -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Sum Length).Sum) } else { 0 }
$intact = ($shardsCountAfter -ge $pre.shards_count) -and ($shardsSizeAfter -gt 0)
Add-Result "Re3-shards-intact" $(if ($intact) { "PASS" } else { "FAIL" }) "before count=$($pre.shards_count) after=$shardsCountAfter; before size=$($pre.shards_total_size) after=$shardsSizeAfter"

# Bring daemon back
Start-Process -FilePath $mneme -ArgumentList "daemon", "start" -WindowStyle Hidden
Start-Sleep 8
$d1 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if ($d1) {
    Add-Result "Re3-daemon-restartable" "PASS" "daemon up post-reboot pid=$($d1.Id)"
} else {
    Add-Result "Re3-daemon-restartable" "FAIL" "could not restart daemon post-reboot"
}

# Verify status responds
$st = & $mneme daemon status 2>&1 | Out-String
$stOk = ($st -match '"running"' -or $st -match '"name":')
Add-Result "Re3-status-ok" $(if ($stOk) { "PASS" } else { "FAIL" }) "status_excerpt=$($st.Substring(0, [Math]::Min(200, $st.Length)))"

Write-Host "=== Re3-RESULTS-JSON ==="
$results | ConvertTo-Json -Depth 3
Write-Host "=== Re3-END ==="
$pass = ($results | Where-Object status -eq "PASS").Count
$fail = ($results | Where-Object status -eq "FAIL").Count
Write-Host "Re3_VERDICT: pass=$pass fail=$fail"
