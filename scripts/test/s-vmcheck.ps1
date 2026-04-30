param()
$ErrorActionPreference = "Continue"
Write-Host "=== mneme --version ==="
& "C:\Users\Administrator\.mneme\bin\mneme.exe" --version
Write-Host "=== bin/ files ==="
Get-ChildItem "C:\Users\Administrator\.mneme\bin" -Filter *.exe | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize
Write-Host "=== mcp/src/index.ts size ==="
$idx = "C:\Users\Administrator\.mneme\mcp\src\index.ts"
if (Test-Path $idx) {
    Get-Item $idx | Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize
} else {
    Write-Host "MISSING: $idx"
}
Write-Host "=== node modules present ==="
$nm = "C:\Users\Administrator\.mneme\mcp\node_modules"
if (Test-Path $nm) { Write-Host "OK: node_modules exists" } else { Write-Host "MISSING: node_modules" }
Write-Host "=== bun + node ==="
$bunPath = "$env:USERPROFILE\.bun\bin\bun.exe"
if (Test-Path $bunPath) { & $bunPath --version } else { Write-Host "bun not found" }
$nodePath = (Get-Command node -ErrorAction SilentlyContinue).Source
if ($nodePath) { & $nodePath --version } else { Write-Host "node not found" }
