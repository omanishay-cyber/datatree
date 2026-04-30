param()
$ErrorActionPreference = "SilentlyContinue"
$names = @("mneme-daemon","mneme-brain","mneme-parsers","mneme-scanners","mneme-store","mneme-livebus","mneme-md-ingest","mneme-multimodal","mneme")
foreach ($n in $names) { Stop-Process -Name $n -Force -ErrorAction SilentlyContinue }
Start-Sleep 3
$rem = Get-Process mneme* -ErrorAction SilentlyContinue
if ($rem) {
    Write-Host "REMAINING_AFTER_KILL:"
    $rem | Format-Table Name, Id -AutoSize
} else {
    Write-Host "ALL_CLEAN"
}
