param()
$ErrorActionPreference = "Continue"
# Phase Un.10 -- reinstall after uninstall
# Acceptance: install.ps1 runs cleanly, mneme.exe back, daemon ready
$zip = "C:\Users\Administrator\mneme.zip"
if (-not (Test-Path $zip)) {
    Write-Host "FAIL: no zip at $zip"
    exit 1
}
$stage = "C:\Users\Administrator\un10-reinstall"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Expand-Archive -Path $zip -DestinationPath $stage -Force
$installPs1 = Get-ChildItem $stage -Filter "install.ps1" -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $installPs1) {
    Write-Host "FAIL: install.ps1 not in zip"
    Get-ChildItem $stage -Recurse -Directory | Select-Object FullName
    exit 1
}
Write-Host "Found install.ps1 at: $($installPs1.FullName)"
$installOut = & powershell -NoProfile -ExecutionPolicy Bypass -File $installPs1.FullName 2>&1 | Out-String
$installCode = $LASTEXITCODE
Write-Host $installOut
$newBin = Test-Path "$env:USERPROFILE\.mneme\bin\mneme.exe"
$mcpInIndex = Test-Path "$env:USERPROFILE\.mneme\mcp\src\index.ts"
$workersFound = (Get-ChildItem "$env:USERPROFILE\.mneme\bin" -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "mneme-*" }).Count
$claudeMcp = (Get-Content "$env:USERPROFILE\.claude.json" -Raw -ErrorAction SilentlyContinue) -match '"mneme"'
Write-Host "=== Un.10-reinstall results ==="
Write-Host "install_exit=$installCode"
Write-Host "mneme.exe exists=$newBin"
Write-Host "mcp/src/index.ts exists=$mcpInIndex"
Write-Host "worker binaries=$workersFound"
Write-Host "claude.json mneme entry=$claudeMcp"
if ($installCode -eq 0 -and $newBin -and $mcpInIndex -and $workersFound -ge 7) {
    Write-Host "Un.10-reinstall: PASS"
} else {
    Write-Host "Un.10-reinstall: FAIL"
}
