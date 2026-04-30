param()
$ErrorActionPreference = "Continue"
# Get fresh daemon log via the CLI but redirect to file to avoid huge output
$logOut = "C:\Users\Administrator\daemon-log-snapshot.txt"
& "C:\Users\Administrator\.mneme\bin\mneme.exe" daemon logs --lines 500 2>&1 | Set-Content -Path $logOut -Encoding UTF8

Write-Host "=== Counts of significant events ==="
$content = Get-Content $logOut -Raw
$childRestarts = ([regex]::Matches($content, '"child":"\w+"')).Count
$panics = ([regex]::Matches($content, "panic|FATAL|abort|stack overflow")).Count
$errs = ([regex]::Matches($content, '"level":"error"|"level":"warn"')).Count
$bootMsgs = ([regex]::Matches($content, "booting|started|online")).Count
Write-Host "Child events: $childRestarts"
Write-Host "Panics/aborts: $panics"
Write-Host "Errors/warns: $errs"
Write-Host "Boot/online: $bootMsgs"

Write-Host "=== Searching for non-info events ==="
Get-Content $logOut | Where-Object { $_ -match '"level":"(error|warn)"' } | Select-Object -First 20

Write-Host "=== Searching for restart/exit/dead messages ==="
Get-Content $logOut | Where-Object { $_ -match "restart|exit|dead|killed|panic|exhausted|budget" } | Select-Object -First 10

Write-Host "=== Last 5 lines from each child ==="
$children = "parser-worker-0","parser-worker-1","store-worker","brain-worker","livebus-worker","md-ingest-worker","scanner-worker-0"
foreach ($c in $children) {
    Write-Host "--- $c ---"
    Get-Content $logOut | Where-Object { $_ -match """child"":""$c""" } | Select-Object -Last 3
}
