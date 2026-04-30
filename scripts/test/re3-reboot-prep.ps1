param()
$ErrorActionPreference = "Continue"
# Capture pre-reboot state, write to file, then issue reboot
$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
$pre = @{
    timestamp_utc = (Get-Date).ToUniversalTime().ToString("o")
    daemon_pid = (Get-Process mneme-daemon -ErrorAction SilentlyContinue).Id
    workers = @(Get-Process mneme-* -ErrorAction SilentlyContinue | Select-Object Name, Id)
    mneme_version = (& $mneme --version) -join " "
    shards_count = if (Test-Path "$env:USERPROFILE\.mneme\projects") { (Get-ChildItem "$env:USERPROFILE\.mneme\projects" -Directory -ErrorAction SilentlyContinue).Count } else { 0 }
    shards_total_size = if (Test-Path "$env:USERPROFILE\.mneme\projects") { ((Get-ChildItem "$env:USERPROFILE\.mneme\projects" -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Sum Length).Sum) } else { 0 }
}
$pre | ConvertTo-Json -Depth 4 | Set-Content "C:\Users\Administrator\re3-prereboot.json" -Encoding UTF8
Write-Host "RE3-PRE captured:"
Get-Content "C:\Users\Administrator\re3-prereboot.json" | Write-Host
Write-Host "Issuing shutdown /r /t 5 ..."
& shutdown /r /t 5 /f
Write-Host "REBOOT_SCHEDULED"
