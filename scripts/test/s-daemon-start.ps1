param()
$ErrorActionPreference = "Continue"
$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
Write-Host "=== Starting daemon ==="
Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
Start-Sleep 5
Write-Host "=== Status (DB-fallback ok) ==="
& $mneme daemon status 2>&1 | Out-String
Write-Host "=== Process list ==="
Get-Process mneme* -ErrorAction SilentlyContinue | Select-Object Name, Id, WorkingSet64 | Format-Table -AutoSize
