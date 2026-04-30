param()
$ErrorActionPreference = "Continue"
Get-Process mneme* -ErrorAction SilentlyContinue | Select-Object Name, Id, @{N="WS_MB";E={[math]::Round($_.WorkingSet64/1MB,1)}}, @{N="Threads";E={$_.Threads.Count}}, @{N="Handles";E={$_.HandleCount}} | Format-Table -AutoSize
