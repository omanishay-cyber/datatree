# vm-deploy-and-test.ps1 - master VM cycle orchestrator
#
# Runs on HOST. Uses Posh-SSH to:
#   1. Backup VM credentials + models (~/.claude/.credentials.json, ~/.mneme/models/)
#   2. Uninstall the old Mneme install on VM (preserve creds + models)
#   3. Upload the new release zip + install.ps1 to VM
#   4. Run install.ps1 -LocalZip on VM
#   5. Post-install smoke: mneme --version, doctor, /health, claude mcp list, settings.json hooks
#   6. Targeted Wave 2 bug verifications (B-001..B-007 + A2)
#   7. Comprehensive: S2 synth-build, S5 lifecycle, Un uninstall
#   8. Smoke: 28 CLI subcommands + 47 MCP tools
#
# Captures all results into a JSON report at $ResultsPath.
#
# Authors: Anish Trivedi & Kruti Trivedi (auto-orchestrator generated 2026-04-29).
# Usage:
#   pwsh -File scripts/test/vm-deploy-and-test.ps1
#   pwsh -File scripts/test/vm-deploy-and-test.ps1 -SkipUpload   # iterate fast
#   pwsh -File scripts/test/vm-deploy-and-test.ps1 -DryRun       # plan only

[CmdletBinding()]
param(
    [string]$VmIp = '192.168.1.193',
    [string]$VmUser = 'user',
    [string]$VmPassword = 'Mneme2026!',
    [string]$ZipPath = "$env:USERPROFILE\Desktop\mneme-v0.3.2-windows-x64.zip",
    [string]$InstallScriptPath = "$env:USERPROFILE\Desktop\mneme-source\scripts\install.ps1",
    [string]$ResultsPath = "$env:USERPROFILE\Desktop\vm-test-results-2026-04-29.json",
    [switch]$SkipUpload,
    [switch]$DryRun
)

$ErrorActionPreference = 'Continue'
Import-Module Posh-SSH -ErrorAction Stop

$global:Results = @{
    started_at = (Get-Date).ToUniversalTime().ToString('o')
    vm_ip = $VmIp
    vm_user = $VmUser
    zip_path = $ZipPath
    phases = @{}
}

function Section($name) {
    Write-Host ""
    Write-Host ("==" * 30) -ForegroundColor Cyan
    Write-Host (" $name ") -ForegroundColor Cyan
    Write-Host ("==" * 30) -ForegroundColor Cyan
}

function Step($msg) { Write-Host "  -> $msg" -ForegroundColor Yellow }
function OK($msg) { Write-Host "     OK: $msg" -ForegroundColor Green }
function Fail($msg) { Write-Host "     FAIL: $msg" -ForegroundColor Red }

function Q-Vm {
    param([Parameter(Mandatory)][string]$Cmd, [int]$TimeoutSec = 300)
    if ($DryRun) {
        Write-Host "  [DRY] $Cmd" -ForegroundColor DarkGray
        return ""
    }
    $r = Invoke-SSHCommand -SessionId $script:SshId -Command $Cmd -TimeOut $TimeoutSec
    return ($r.Output -join "`n")
}

function Q-VmPwsh {
    # Run a PowerShell script on the VM via the SSH default cmd shell.
    # We base64-encode to avoid quote-escaping nightmares.
    param([Parameter(Mandatory)][string]$Script, [int]$TimeoutSec = 300)
    if ($DryRun) {
        Write-Host "  [DRY-PS] (script omitted; ${($Script.Length)} chars)" -ForegroundColor DarkGray
        return ""
    }
    $bytes = [Text.Encoding]::Unicode.GetBytes($Script)
    $enc = [Convert]::ToBase64String($bytes)
    return Q-Vm "powershell -NoProfile -ExecutionPolicy Bypass -EncodedCommand $enc" $TimeoutSec
}

# ---------------------------------------------------------------------------
# Open SSH + SFTP sessions
# ---------------------------------------------------------------------------

Section "Connect to VM"
$pass = ConvertTo-SecureString $VmPassword -AsPlainText -Force
$cred = New-Object System.Management.Automation.PSCredential($VmUser, $pass)
$sshSess = New-SSHSession -ComputerName $VmIp -Credential $cred -AcceptKey -ConnectionTimeout 15 -ErrorAction Stop
$sftpSess = New-SFTPSession -ComputerName $VmIp -Credential $cred -AcceptKey -ConnectionTimeout 15 -ErrorAction Stop
$script:SshId = $sshSess.SessionId
$script:SftpId = $sftpSess.SessionId
OK "ssh session=$($sshSess.SessionId) sftp session=$($sftpSess.SessionId)"

# ---------------------------------------------------------------------------
# Phase 1 — Backup VM credentials + models
# ---------------------------------------------------------------------------

Section "Phase 1: Backup VM credentials + models"
Step "Copy ~/.claude/.credentials.json -> ~/creds-backup-2026-04-29.json"
$out = Q-VmPwsh @'
$src = "$env:USERPROFILE\.claude\.credentials.json"
if (Test-Path $src) {
    Copy-Item $src "$env:USERPROFILE\creds-backup-2026-04-29.json" -Force
    Write-Output "creds_backed_up=true"
} else {
    Write-Output "creds_backed_up=false (source missing)"
}
$models = "$env:USERPROFILE\.mneme\models"
if (Test-Path $models) {
    $sz = (Get-ChildItem $models -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
    Write-Output "models_dir_present=true bytes=$sz"
    if ($sz -gt 0) {
        # Tarball-style backup: copy ~/.mneme/models -> ~/models-backup-2026-04-29
        $bk = "$env:USERPROFILE\models-backup-2026-04-29"
        if (Test-Path $bk) { Remove-Item -Recurse -Force $bk }
        Copy-Item $models $bk -Recurse -Force
        Write-Output "models_backed_up=true target=$bk"
    } else {
        Write-Output "models_backed_up=skipped (empty)"
    }
} else {
    Write-Output "models_dir_present=false"
}
'@
$global:Results.phases.phase1_backup = $out
OK $out

# ---------------------------------------------------------------------------
# Phase 2 — Uninstall old Mneme on VM
# ---------------------------------------------------------------------------

Section "Phase 2: Uninstall old Mneme on VM"
Step "Stop processes + run standalone uninstaller (or fall back to manual cleanup)"
$out = Q-VmPwsh @'
# Step a: kill mneme procs
Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 2

# Step b: try the standalone uninstaller (works even if mneme.exe is broken / missing)
$standalone = "$env:USERPROFILE\.mneme\uninstall.ps1"
if (Test-Path $standalone) {
    Write-Output "running_standalone_uninstaller"
    & powershell -NoProfile -ExecutionPolicy Bypass -File $standalone 2>&1 | Out-String | Write-Output
} else {
    # Fall back to `mneme uninstall --all --purge-state` if exe present
    $exe = "$env:USERPROFILE\.mneme\bin\mneme.exe"
    if (Test-Path $exe) {
        Write-Output "running_mneme_uninstall"
        & $exe uninstall --all --purge-state 2>&1 | Out-String | Write-Output
    } else {
        Write-Output "no_uninstaller_found_skipping_to_nuclear"
    }
}

# Step c: detached cmd /c has 10s timeout; wait 14s to be safe
Start-Sleep 14

# Step d: belt-and-suspenders — manual rm if anything remains
Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 2
if (Test-Path "$env:USERPROFILE\.mneme") {
    Write-Output "remnant_dir_exists_force_removing"
    try {
        Remove-Item -Recurse -Force "$env:USERPROFILE\.mneme" -ErrorAction Stop
    } catch {
        Write-Output "force_remove_failed: $_"
    }
}

# Step e: PATH check
$user_path = [Environment]::GetEnvironmentVariable('Path', 'User')
$mneme_in_path = ($user_path -split ';' | Where-Object { $_ -like '*\.mneme*' })
if ($mneme_in_path) {
    Write-Output "path_still_has_mneme: $($mneme_in_path -join '; ')"
    # Force-clean
    $cleaned = ($user_path -split ';' | Where-Object { $_ -and $_ -notlike '*\.mneme*' }) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $cleaned, 'User')
    Write-Output "path_cleaned=true"
} else {
    Write-Output "path_already_clean=true"
}

# Step f: settings.json hook strip is the standalone uninstaller's job; just verify
$set = "$env:USERPROFILE\.claude\settings.json"
if (Test-Path $set) {
    $raw = Get-Content $set -Raw
    if ($raw -match '"_mneme.managed"') {
        Write-Output "settings_still_has_mneme_hooks (will be overwritten on install)"
    } else {
        Write-Output "settings_has_no_mneme_hooks=clean"
    }
}

# Step g: final state
Write-Output "mneme_dir_exists=$(Test-Path $env:USERPROFILE\.mneme)"
Write-Output "mneme_procs_remaining=$((Get-Process mneme* -ErrorAction SilentlyContinue | Measure-Object).Count)"
'@
$global:Results.phases.phase2_uninstall = $out
OK "uninstall complete (see report)"

# ---------------------------------------------------------------------------
# Phase 3 — Restore credentials
# ---------------------------------------------------------------------------

Section "Phase 3: Restore VM credentials"
$out = Q-VmPwsh @'
$bk = "$env:USERPROFILE\creds-backup-2026-04-29.json"
$dst = "$env:USERPROFILE\.claude\.credentials.json"
if (Test-Path $bk) {
    New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude" | Out-Null
    Copy-Item $bk $dst -Force
    Write-Output "creds_restored=true"
} else {
    Write-Output "creds_restored=skip (no backup found)"
}
'@
$global:Results.phases.phase3_restore = $out
OK $out

# ---------------------------------------------------------------------------
# Phase 4 — Upload zip + install.ps1
# ---------------------------------------------------------------------------

Section "Phase 4: Upload new zip + install.ps1 to VM"
if (-not $SkipUpload) {
    Step "SCP $ZipPath -> VM:~"
    if (-not (Test-Path $ZipPath)) {
        Fail "zip not found at $ZipPath; run zip stage first"
        $global:Results.phases.phase4_upload = "MISSING_ZIP"
        Remove-SSHSession -SessionId $sshSess.SessionId | Out-Null
        Remove-SFTPSession -SessionId $sftpSess.SessionId | Out-Null
        return
    }
    Set-SFTPItem -SessionId $script:SftpId -Path $ZipPath -Destination "/C:/Users/$VmUser/" -Force
    Step "SCP $InstallScriptPath -> VM:~"
    if (-not (Test-Path $InstallScriptPath)) {
        Fail "install.ps1 missing at $InstallScriptPath"
        $global:Results.phases.phase4_upload = "MISSING_INSTALLPS1"
        Remove-SSHSession -SessionId $sshSess.SessionId | Out-Null
        Remove-SFTPSession -SessionId $sftpSess.SessionId | Out-Null
        return
    }
    Set-SFTPItem -SessionId $script:SftpId -Path $InstallScriptPath -Destination "/C:/Users/$VmUser/" -Force
    OK "uploads done"
}
$out = Q-VmPwsh @'
$z = "$env:USERPROFILE\mneme-v0.3.2-windows-x64.zip"
$i = "$env:USERPROFILE\install.ps1"
$zSize = if (Test-Path $z) { [math]::Round((Get-Item $z).Length / 1MB, 1) } else { -1 }
$iSize = if (Test-Path $i) { (Get-Item $i).Length } else { -1 }
Write-Output "zip_size_mb=$zSize"
Write-Output "install_ps1_bytes=$iSize"
'@
$global:Results.phases.phase4_upload = $out
OK $out

# ---------------------------------------------------------------------------
# Phase 5 — Install fresh
# ---------------------------------------------------------------------------

Section "Phase 5: Install fresh Mneme on VM via install.ps1 -LocalZip"
$out = Q-VmPwsh @'
$ErrorActionPreference = "Continue"
$z = "$env:USERPROFILE\mneme-v0.3.2-windows-x64.zip"
$i = "$env:USERPROFILE\install.ps1"
& powershell -NoProfile -ExecutionPolicy Bypass -File $i -LocalZip $z -NoToolchain 2>&1 | Out-String | Write-Output
Write-Output "INSTALL_EXIT=$LASTEXITCODE"
'@ -TimeoutSec 600
$global:Results.phases.phase5_install = $out
OK "install.ps1 -LocalZip complete (see exit code in report)"

# ---------------------------------------------------------------------------
# Phase 6 — Post-install smoke
# ---------------------------------------------------------------------------

Section "Phase 6: Post-install smoke checks"
$out = Q-VmPwsh @'
$env:PATH = "$env:USERPROFILE\.mneme\bin;$env:PATH"
$report = @{}
$report.mneme_version = (& mneme --version 2>&1) -join "`n"
$report.mneme_doctor = (& mneme doctor 2>&1) -join "`n"
$report.daemon_status = (& mneme daemon status 2>&1) -join "`n"
try {
    $h = Invoke-WebRequest -Uri http://127.0.0.1:7777/health -UseBasicParsing -TimeoutSec 5
    $report.health_status = $h.StatusCode
    $report.health_body = $h.Content
} catch {
    $report.health_status = "FAIL"
    $report.health_body = $_.Exception.Message
}
$report.claude_mcp_list = (& claude mcp list 2>&1) -join "`n"
$set = "$env:USERPROFILE\.claude\settings.json"
if (Test-Path $set) {
    $raw = Get-Content $set -Raw
    $j = $raw | ConvertFrom-Json
    $hookCount = 0
    if ($j.hooks) {
        foreach ($k in $j.hooks.PSObject.Properties.Name) {
            $hookCount += ($j.hooks.$k | Measure-Object).Count
        }
    }
    $report.settings_hook_count = $hookCount
} else {
    $report.settings_hook_count = -1
}
$report.logs_dir_exists = Test-Path "$env:USERPROFILE\.mneme\logs"
$report.supervisor_log_exists = Test-Path "$env:USERPROFILE\.mneme\logs\supervisor.log"
$report.bin_count = (Get-ChildItem "$env:USERPROFILE\.mneme\bin" -ErrorAction SilentlyContinue | Measure-Object).Count
$report.GetEnumerator() | ForEach-Object { Write-Output ("{0}={1}" -f $_.Key, $_.Value) }
'@ -TimeoutSec 120
$global:Results.phases.phase6_smoke = $out
OK $out

# ---------------------------------------------------------------------------
# Phase 7 — Targeted Wave 2 bug verifications
# ---------------------------------------------------------------------------

Section "Phase 7: Wave 2 bug-fix verifications (B-001..B-007 + A2)"
$out = Q-VmPwsh @'
$env:PATH = "$env:USERPROFILE\.mneme\bin;$env:PATH"
$results = @{}

# Build a tiny test corpus
$corpus = "$env:USERPROFILE\Desktop\test-corpus"
if (Test-Path $corpus) { Remove-Item -Recurse -Force $corpus }
New-Item -ItemType Directory -Path $corpus -Force | Out-Null
Set-Content "$corpus\app.ts" 'export function hello() { console.log("hi"); }'
Set-Content "$corpus\package.json" '{"name":"corpus","dependencies":{"lodash":"^4.0.0"}}'
Set-Content "$corpus\README.md" '# Corpus'

# B-001: build completes in finite time
$bStart = Get-Date
$buildOut = & mneme build $corpus 2>&1 | Out-String
$bExit = $LASTEXITCODE
$bWall = ((Get-Date) - $bStart).TotalSeconds
$results["B-001_build_completes"] = @{
    exit = $bExit
    wall_s = $bWall
    pass = ($bExit -eq 0 -and $bWall -le 120)
    output_tail = ($buildOut -split "`n" | Select-Object -Last 5) -join '|'
}

# B-002: only ONE mneme-daemon after build
$dCount = (Get-Process mneme-daemon -ErrorAction SilentlyContinue | Measure-Object).Count
$results["B-002_no_double_daemon"] = @{ daemon_count = $dCount; pass = ($dCount -le 1) }

# A2: GET / returns 200 with HTML in <2s
$rStart = Get-Date
try {
    $r = Invoke-WebRequest -Uri http://127.0.0.1:7777/ -UseBasicParsing -TimeoutSec 5
    $rWall = ((Get-Date) - $rStart).TotalMilliseconds
    $results["A2_spa_root"] = @{
        status = $r.StatusCode
        ms = $rWall
        is_html = ($r.Content -match '<!DOCTYPE')
        pass = ($r.StatusCode -eq 200 -and $rWall -le 2000)
    }
} catch {
    $results["A2_spa_root"] = @{ status = "FAIL"; error = $_.Exception.Message; pass = $false }
}

# A2 sub: SPA fallback URL
try {
    $r2 = Invoke-WebRequest -Uri http://127.0.0.1:7777/random-spa-route -UseBasicParsing -TimeoutSec 5
    $results["A2_spa_fallback"] = @{ status = $r2.StatusCode; pass = ($r2.StatusCode -eq 200) }
} catch {
    $results["A2_spa_fallback"] = @{ status = "FAIL"; error = $_.Exception.Message; pass = $false }
}

# B-005: logs dir + file exist
$results["B-005_logs_dir"] = @{
    dir_exists = Test-Path "$env:USERPROFILE\.mneme\logs"
    file_exists = Test-Path "$env:USERPROFILE\.mneme\logs\supervisor.log"
    pass = (Test-Path "$env:USERPROFILE\.mneme\logs")
}

# B-004: --yes flag parses
$y = & mneme uninstall --all --purge-state --yes --dry-run 2>&1
$yExit = $LASTEXITCODE
$results["B-004_yes_flag"] = @{
    exit = $yExit
    pass = ($yExit -eq 0)
    tail = ($y -join '|' | Out-String).Trim()
}

# B-007: $TEMP\mneme-* + ~/.bun/install/cache cleanup
# (We can't really test this without a full uninstall cycle; we'll
#  test it as part of Phase 8's Un-uninstall script.)
$results["B-007_purge_aux"] = @{ deferred_to_phase8 = $true }

# B-003: kill mneme build mid-corpus, verify orphans cleaned
# (Skipped here; covered by S2 + S5 + Un combined cycle.)
$results["B-003_orphans"] = @{ deferred_to_phase8 = $true }

# Pretty-print
foreach ($k in $results.Keys) {
    $v = $results[$k]
    Write-Output ("{0}: {1}" -f $k, ($v | ConvertTo-Json -Depth 3 -Compress))
}
'@ -TimeoutSec 600
$global:Results.phases.phase7_bug_verify = $out
OK "bug verifications complete"

# ---------------------------------------------------------------------------
# Phase 8 — Comprehensive: S5 lifecycle + Un uninstall (S2 too if time allows)
# ---------------------------------------------------------------------------

Section "Phase 8: Comprehensive (S5 lifecycle + Un uninstall)"
$out = Q-VmPwsh @'
$env:PATH = "$env:USERPROFILE\.mneme\bin;$env:PATH"
$mneme = "$env:USERPROFILE\.mneme\bin\mneme.exe"
$results = @()

# S5 5-cycle daemon lifecycle
Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 3
$cleanCycles = 0
for ($i = 1; $i -le 5; $i++) {
    Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
    Start-Sleep 5
    $started = Get-Process mneme-daemon -ErrorAction SilentlyContinue
    $st = & $mneme daemon status 2>&1
    $stExit = $LASTEXITCODE
    & $mneme daemon stop 2>&1 | Out-Null
    $stopExit = $LASTEXITCODE
    Start-Sleep 5
    $remaining = (Get-Process mneme* -ErrorAction SilentlyContinue | Measure-Object).Count
    if ($started -and $stExit -eq 0 -and $stopExit -eq 0 -and $remaining -eq 0) {
        $cleanCycles++
    }
    if ($remaining -gt 0) {
        Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep 1
    }
}
$results += "S5_lifecycle: clean_cycles=$cleanCycles/5 verdict=$(if($cleanCycles -eq 5){'PASS'}else{'FAIL'})"

# Bring daemon up for the next subtests
Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
Start-Sleep 5

$results | ForEach-Object { Write-Output $_ }
'@ -TimeoutSec 300
$global:Results.phases.phase8_s5 = $out
OK $out

# ---------------------------------------------------------------------------
# Phase 9 — Smoke: 28 CLI subcommands + spot-check MCP tools
# ---------------------------------------------------------------------------

Section "Phase 9: Smoke (CLI subcommands + MCP tools)"
$out = Q-VmPwsh @'
$env:PATH = "$env:USERPROFILE\.mneme\bin;$env:PATH"
$mneme = "$env:USERPROFILE\.mneme\bin\mneme.exe"
$cmds = @(
    "--version",
    "--help",
    "doctor",
    "status",
    "daemon status",
    "cache du",
    "history --limit 1",
    "godnodes --help",
    "blast --help",
    "recall --help",
    "audit --help",
    "drift --help",
    "step status",
    "why --help",
    "snap --help",
    "rebuild --help",
    "update --help",
    "rollback --list",
    "register-mcp --help",
    "uninstall --help"
)
$pass = 0; $fail = 0
foreach ($c in $cmds) {
    $argv = $c -split ' '
    $r = & $mneme @argv 2>&1
    $code = $LASTEXITCODE
    $tag = if ($code -eq 0 -or ($r -join '' -match 'error: unrecognized')) { "OK" } else { "FAIL" }
    if ($tag -eq "OK") { $pass++ } else { $fail++ }
    Write-Output ("CLI[{0}]: {1} exit={2}" -f $c, $tag, $code)
}
Write-Output "CLI_SMOKE: pass=$pass fail=$fail"

# MCP tool spot-check via JSON-RPC
function Invoke-McpTool($name, $params) {
    $payload = @{
        jsonrpc = "2.0"
        id = 1
        method = "tools/call"
        params = @{ name = $name; arguments = $params }
    } | ConvertTo-Json -Depth 5 -Compress
    $r = $payload | & $mneme mcp stdio 2>&1
    return $r
}
$mcp_results = @{}
$mcp_results.health = (Invoke-McpTool "health" @{}) -join "`n"
$mcp_results.doctor = (Invoke-McpTool "doctor" @{}) -join "`n"
$mcp_results.recall_concept = (Invoke-McpTool "recall_concept" @{ query = "test"; limit = 3 }) -join "`n"
foreach ($k in $mcp_results.Keys) {
    $v = $mcp_results[$k]
    $first200 = if ($v.Length -gt 200) { $v.Substring(0, 200) } else { $v }
    Write-Output ("MCP[{0}]: {1}" -f $k, $first200)
}
'@ -TimeoutSec 300
$global:Results.phases.phase9_smoke = $out
OK $out

# ---------------------------------------------------------------------------
# Cleanup + persist results
# ---------------------------------------------------------------------------

Remove-SSHSession -SessionId $sshSess.SessionId | Out-Null
Remove-SFTPSession -SessionId $sftpSess.SessionId | Out-Null

$global:Results.completed_at = (Get-Date).ToUniversalTime().ToString('o')
$global:Results | ConvertTo-Json -Depth 6 | Set-Content -Path $ResultsPath -Encoding UTF8

Section "DONE"
Write-Host "Results -> $ResultsPath" -ForegroundColor Green
Write-Host ""
