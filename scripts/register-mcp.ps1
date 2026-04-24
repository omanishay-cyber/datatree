# Register the Mneme MCP server with Claude Code (and other MCP clients).
# Run once per user; Claude Code will auto-discover Mneme on its next start.
#
# Idempotent — re-running is safe, it only writes what's missing.

$ErrorActionPreference = 'Stop'

$ClaudeSettings = Join-Path $env:USERPROFILE '.claude\settings.json'
$McpEntry = @{
  command = 'mneme'
  args    = @('mcp', 'stdio')
}

Write-Host "Mneme MCP registration"
Write-Host "======================"

# 1. Ensure the daemon is running.
$daemon = Get-Process -Name 'mneme-daemon' -ErrorAction SilentlyContinue
if (-not $daemon) {
  $daemonPath = Join-Path $env:USERPROFILE '.mneme\bin\mneme-daemon.exe'
  if (-not (Test-Path $daemonPath)) {
    Write-Host "daemon not installed — run the one-liner first:" -ForegroundColor Yellow
    Write-Host "  iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install.ps1 | iex"
    exit 1
  }
  Write-Host "starting mneme-daemon in the background..."
  Start-Process -FilePath $daemonPath -ArgumentList 'start' -WindowStyle Hidden
  Start-Sleep -Seconds 3
} else {
  Write-Host "mneme-daemon already running (PID $($daemon.Id))"
}

# 2. Health probe.
try {
  $health = Invoke-RestMethod -Uri 'http://127.0.0.1:7777/health' -TimeoutSec 5
  Write-Host "health: $($health.status) — supervisor uptime $($health.supervisor_uptime_s)s"
} catch {
  Write-Host "health probe failed: $($_.Exception.Message)" -ForegroundColor Yellow
}

# 3. Register the MCP server in ~/.claude/settings.json.
if (-not (Test-Path $ClaudeSettings)) {
  New-Item -ItemType File -Path $ClaudeSettings -Force | Out-Null
  Set-Content -Path $ClaudeSettings -Value '{}' -NoNewline
}

$json = Get-Content $ClaudeSettings -Raw | ConvertFrom-Json -Depth 10
if ($null -eq $json) { $json = [PSCustomObject]@{} }

if (-not $json.PSObject.Properties.Match('mcpServers').Count) {
  $json | Add-Member -MemberType NoteProperty -Name 'mcpServers' -Value ([PSCustomObject]@{}) -Force
}

$already = $json.mcpServers.PSObject.Properties.Match('mneme').Count -gt 0
if ($already) {
  Write-Host "mneme MCP already registered in $ClaudeSettings"
} else {
  $json.mcpServers | Add-Member -MemberType NoteProperty -Name 'mneme' -Value ([PSCustomObject]$McpEntry)
  $json | ConvertTo-Json -Depth 10 | Set-Content -Path $ClaudeSettings -NoNewline
  Write-Host "registered mneme in $ClaudeSettings" -ForegroundColor Green
}

# 4. Verify.
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Fully quit Claude Code (not just close the window — Task Manager or File > Exit)."
Write-Host "  2. Reopen Claude Code in any project."
Write-Host "  3. The 46 mneme_* MCP tools will appear in the tool list automatically."
Write-Host "  4. Ask Claude: 'use mneme_context to summarise this repo' — verify it works."
Write-Host ""
Write-Host "If the tools don't appear, check:"
Write-Host "  - curl http://127.0.0.1:7777/health    (daemon must be up)"
Write-Host "  - Get-Content $ClaudeSettings | Select-String mneme    (registration must exist)"
Write-Host "  - mneme doctor                          (shards must be writable)"
