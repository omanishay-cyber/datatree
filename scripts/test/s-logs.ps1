param()
$ErrorActionPreference = "Continue"
$logDirs = @(
    "$env:USERPROFILE\.mneme\logs",
    "$env:USERPROFILE\.mneme\daemon",
    "$env:USERPROFILE\.mneme"
)
foreach ($d in $logDirs) {
    Write-Host "=== Searching: $d ==="
    if (Test-Path $d) {
        Get-ChildItem $d -File -Recurse -ErrorAction SilentlyContinue | Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize
    }
}
$logCandidates = @(
    "$env:USERPROFILE\.mneme\logs\daemon.log",
    "$env:USERPROFILE\.mneme\logs\supervisor.log",
    "$env:USERPROFILE\.mneme\daemon.log"
)
foreach ($f in $logCandidates) {
    if (Test-Path $f) {
        Write-Host "`n=== Tail $f (last 100 lines) ==="
        Get-Content $f -Tail 100
    }
}
Write-Host "=== mneme daemon logs (CLI) ==="
& "C:\Users\Administrator\.mneme\bin\mneme.exe" daemon logs --lines 80 2>&1 | Out-String
