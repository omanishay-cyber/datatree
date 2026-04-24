# Mneme — one-line installer for Windows (v0.3.1+)
#
# Usage (PowerShell, as current user — elevation is optional, see below):
#   iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install.ps1 | iex
#
# What it does, in order:
#   1. Ensures Bun is installed (runs the official Bun installer if not
#      already present). Bun is the only runtime dependency — mneme's MCP
#      server is TypeScript that Bun runs. Rust, Node, Python are NOT
#      needed: mneme ships as pre-built binaries.
#   2. Downloads mneme-windows-x64.zip from the latest GitHub release.
#   3. Extracts to %USERPROFILE%\.mneme\ (bin/, mcp/, plugin/).
#   4. Adds Windows Defender exclusions for %USERPROFILE%\.mneme\ and
#      %USERPROFILE%\.claude\ (requires admin; falls back to a printed
#      one-liner if not elevated). Prevents the known SAgent.HAG!MTB
#      ML-heuristic false positive on mneme's memory/log files.
#   5. Adds the bin directory to the user PATH (persistent, user-scope only).
#   6. Starts the mneme daemon in the background.
#   7. Registers the mneme MCP server with Claude Code (MCP entry only —
#      does NOT touch ~/.claude/settings.json or register hooks).
#   8. Prints next steps and verification commands.
#
# Safe to re-run. Every step is idempotent; a step that fails prints a
# clear message and does not abort the remaining steps (except when
# download / extract themselves fail, which is unrecoverable).
#
# Zero-prereq guarantee: running this script on a stock Windows machine
# with PowerShell produces a working mneme install WITHOUT the user
# pre-installing anything else.
#
# Uninstall: `mneme uninstall --platform claude-code` + remove
# %USERPROFILE%\.mneme\ manually. A full `mneme uninstall` command with
# rollback receipts lands in v0.3.2.

$ErrorActionPreference = 'Stop'

$Repo       = 'omanishay-cyber/mneme'
$Asset      = 'mneme-windows-x64.zip'
$MnemeHome  = Join-Path $env:USERPROFILE '.mneme'
$BinDir     = Join-Path $MnemeHome 'bin'
$ClaudeHome = Join-Path $env:USERPROFILE '.claude'

function Write-Step {
    param([string]$Message, [string]$Color = 'Cyan')
    Write-Host ("==> {0}" -f $Message) -ForegroundColor $Color
}
function Write-Info {
    param([string]$Message)
    Write-Host ("    {0}" -f $Message)
}
function Write-Warn {
    param([string]$Message)
    Write-Host ("    warning: {0}" -f $Message) -ForegroundColor Yellow
}
function Write-OK {
    param([string]$Message)
    Write-Host ("    ok: {0}" -f $Message) -ForegroundColor Green
}
function Write-Fail {
    param([string]$Message)
    Write-Host ("    error: {0}" -f $Message) -ForegroundColor Red
}

function Test-IsElevated {
    try {
        $id  = [Security.Principal.WindowsIdentity]::GetCurrent()
        $p   = New-Object Security.Principal.WindowsPrincipal($id)
        return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch {
        return $false
    }
}

Write-Step "mneme — one-line installer"
Write-Info ("target   : {0}" -f $MnemeHome)
Write-Info ("bin      : {0}" -f $BinDir)
Write-Info ("elevated : {0}" -f (Test-IsElevated))
Write-Host ""

# ============================================================================
# Step 1 — Ensure Bun is installed (the only runtime dep)
# ============================================================================
#
# Bun is required because mneme's MCP server (`~/.mneme/mcp/`) is
# TypeScript that `mneme mcp stdio` launches via `bun`. We detect an
# existing install first to respect the user's setup, then fall back to
# the official installer — which is maintained by Oven (Bun's company),
# user-scope, no admin required, and handles PATH correctly.
#
# Rust is NOT needed: mneme is shipped pre-built.
# Node is NOT needed: Bun is the runtime.
# Python is NOT needed for v0.3.1: the multimodal sidecar is feature-gated.

Write-Step "step 1/8 — checking Bun runtime"

$BunExe = $null
# Respect existing bun on PATH (might be user-installed elsewhere).
$CmdBun = Get-Command bun -ErrorAction SilentlyContinue
if ($CmdBun) {
    $BunExe = $CmdBun.Source
} else {
    # Fall back to the standard user-scope install path.
    $candidate = Join-Path $env:USERPROFILE '.bun\bin\bun.exe'
    if (Test-Path $candidate) {
        $BunExe = $candidate
    }
}

if ($BunExe) {
    try {
        $BunVer = (& $BunExe --version 2>$null).Trim()
        Write-OK ("bun $BunVer present at $BunExe")
    } catch {
        Write-OK ("bun present at $BunExe (version check failed, continuing)")
    }
} else {
    Write-Info "bun not found — running official Bun installer (user-scope, no admin)"
    try {
        iwr -useb https://bun.sh/install.ps1 | iex
        # The Bun installer drops the binary here regardless of PATH state.
        $candidate = Join-Path $env:USERPROFILE '.bun\bin\bun.exe'
        if (Test-Path $candidate) {
            $BunExe = $candidate
            # Add Bun to current session PATH so this script's later steps
            # can find it. The Bun installer has already added it to the
            # persistent user PATH.
            $env:PATH = "$env:PATH;$(Split-Path $candidate -Parent)"
            $BunVer = (& $BunExe --version 2>$null).Trim()
            Write-OK ("bun $BunVer installed at $BunExe")
        } else {
            Write-Warn "Bun installer ran but bun.exe not found at expected path"
            Write-Warn "mneme's MCP server (Claude Code /mn- commands) will not work"
            Write-Warn "Manual install: iwr -useb https://bun.sh/install.ps1 | iex"
        }
    } catch {
        Write-Warn ("Bun install failed: {0}" -f $_.Exception.Message)
        Write-Warn "mneme CLI will still work, but MCP tools in Claude Code will not"
        Write-Warn "Manual install later: iwr -useb https://bun.sh/install.ps1 | iex"
    }
}

# ============================================================================
# Step 2 — Fetch latest release metadata
# ============================================================================

Write-Step "step 2/8 — fetching latest release metadata"

$ApiUrl  = "https://api.github.com/repos/$Repo/releases/latest"
$Headers = @{ 'User-Agent' = 'mneme-installer' }

try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers
} catch {
    Write-Fail ("GitHub API unreachable: {0}" -f $_.Exception.Message)
    exit 1
}

$AssetEntry = $Release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1
if ($null -eq $AssetEntry) {
    Write-Warn ("{0} not yet attached to release {1}" -f $Asset, $Release.tag_name)
    Write-Warn "       the release workflow may still be building — retry in ~15 min."
    exit 1
}
Write-OK ("release {0} — asset {1} ({2:N1} MB)" -f $Release.tag_name, $Asset, ($AssetEntry.size / 1MB))

# ============================================================================
# Step 3 — Download + extract
# ============================================================================

Write-Step "step 3/8 — downloading + extracting"

$Tmp     = Join-Path $env:TEMP ("mneme-install-{0}" -f ([System.Guid]::NewGuid().ToString('N').Substring(0, 8)))
$ZipPath = Join-Path $Tmp $Asset

New-Item -ItemType Directory -Path $Tmp -Force | Out-Null

try {
    Invoke-WebRequest -Uri $AssetEntry.browser_download_url -OutFile $ZipPath -UseBasicParsing -Headers $Headers
} catch {
    Write-Fail ("download failed: {0}" -f $_.Exception.Message)
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
    exit 1
}

if (-not (Test-Path $MnemeHome)) {
    New-Item -ItemType Directory -Path $MnemeHome -Force | Out-Null
}

try {
    Expand-Archive -Path $ZipPath -DestinationPath $MnemeHome -Force
} catch {
    Write-Fail ("extract failed: {0}" -f $_.Exception.Message)
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
    exit 1
}

Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
Write-OK ("extracted to {0}" -f $MnemeHome)

# ============================================================================
# Step 4 — Windows Defender exclusions
# ============================================================================
#
# Defender's ML-based SAgent.HAG!MTB classifier false-positives on mneme's
# memory files because they contain dense agent-automation language
# ("hook", "pre-tool", "blocked", "inject", "subprocess", "exec"). Without
# an exclusion, random mneme data files will be silently quarantined,
# which looks like mysterious data loss to the user.
#
# This step attempts to add exclusions via Add-MpPreference. Requires
# admin. If not elevated, we print the exact one-liner the user can run
# from an elevated shell later.

Write-Step "step 4/8 — Windows Defender exclusions"

$ExcludeDirs = @($MnemeHome, $ClaudeHome)
$DefenderFailed = $false

if (Test-IsElevated) {
    foreach ($dir in $ExcludeDirs) {
        try {
            Add-MpPreference -ExclusionPath $dir -ErrorAction Stop
            Write-OK ("excluded {0}" -f $dir)
        } catch {
            Write-Warn ("could not exclude {0}: {1}" -f $dir, $_.Exception.Message)
            $DefenderFailed = $true
        }
    }
} else {
    Write-Warn "not running elevated — skipping Defender exclusion"
    $DefenderFailed = $true
}

if ($DefenderFailed) {
    Write-Host ""
    Write-Host "    To add the exclusions yourself (prevents Defender false positives):" -ForegroundColor Yellow
    Write-Host "    Run this ONCE in an Administrator PowerShell:" -ForegroundColor Yellow
    Write-Host ""
    foreach ($dir in $ExcludeDirs) {
        Write-Host ("      Add-MpPreference -ExclusionPath `"$dir`"") -ForegroundColor White
    }
    Write-Host ""
    Write-Host "    Without this, Defender may randomly quarantine mneme data files" -ForegroundColor Yellow
    Write-Host "    (SAgent.HAG!MTB false positive on agent-automation text)." -ForegroundColor Yellow
    Write-Host ""
}

# ============================================================================
# Step 5 — Add bin dir to user PATH
# ============================================================================

Write-Step "step 5/8 — updating user PATH"

$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($null -eq $UserPath) { $UserPath = '' }

if (-not ($UserPath.Split(';') -contains $BinDir)) {
    $NewPath = if ([string]::IsNullOrEmpty($UserPath)) { $BinDir } else { "$UserPath;$BinDir" }
    [Environment]::SetEnvironmentVariable('PATH', $NewPath, 'User')
    $env:PATH = "$env:PATH;$BinDir"
    Write-OK ("added {0} to user PATH" -f $BinDir)
} else {
    Write-OK "bin already in PATH"
}

# ============================================================================
# Step 6 — Start the mneme daemon
# ============================================================================

Write-Step "step 6/8 — starting mneme daemon"

$MnemeBin = Join-Path $BinDir 'mneme.exe'
if (-not (Test-Path $MnemeBin)) {
    Write-Warn ("mneme.exe not found at {0} — did extraction succeed?" -f $MnemeBin)
    Write-Warn "skipping daemon start. run manually later: mneme daemon start"
} else {
    try {
        & $MnemeBin daemon start | Out-Null
        Start-Sleep -Milliseconds 500
        Write-OK "daemon started"
    } catch {
        Write-Warn ("daemon start failed: {0}" -f $_.Exception.Message)
        Write-Warn "run manually later: mneme daemon start"
    }
}

# ============================================================================
# Step 7 — Register MCP with Claude Code (NO hook injection, NO manifest)
# ============================================================================
#
# v0.3.1 hard rule: the installer only writes a single mcpServers.mneme
# entry into ~/.claude.json. It does NOT touch ~/.claude/settings.json.
# It does NOT write a CLAUDE.md manifest by default. Those changes were
# what poisoned Claude Code on v0.3.0 — see F-011/F-012 in the install
# report. Power users who want the manifest can run without
# --skip-manifest later.

Write-Step "step 7/8 — registering MCP with Claude Code"

if (-not (Test-Path $MnemeBin)) {
    Write-Warn "mneme.exe not present, skipping MCP registration"
} else {
    try {
        & $MnemeBin install --platform claude-code --skip-manifest --skip-hooks 2>&1 | ForEach-Object { Write-Info $_ }
        if ($LASTEXITCODE -eq 0) {
            Write-OK "Claude Code MCP registration complete"
        } else {
            Write-Warn ("mneme install exited {0} — MCP may not be registered" -f $LASTEXITCODE)
            Write-Warn "run manually later: mneme install --platform claude-code --skip-manifest --skip-hooks"
        }
    } catch {
        Write-Warn ("MCP registration error: {0}" -f $_.Exception.Message)
        Write-Warn "run manually later: mneme install --platform claude-code --skip-manifest --skip-hooks"
    }
}

# ============================================================================
# Step 8 — Done
# ============================================================================

Write-Step "step 8/8 — complete"
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  mneme installed — v$($Release.tag_name)" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Next steps:" -ForegroundColor White
Write-Host "    1. Restart Claude Code so it picks up the new MCP server"
Write-Host "    2. Open a project directory and run: mneme build ."
Write-Host "    3. Inside Claude Code, try:  /mn-recall `"what does auth do`""
Write-Host ""
Write-Host "  Verify:" -ForegroundColor White
Write-Host "    mneme daemon status"
Write-Host "    mneme --version"
Write-Host ""
Write-Host "  Uninstall:" -ForegroundColor White
Write-Host "    mneme uninstall --platform claude-code"
Write-Host "    Remove-Item -Recurse -Force $MnemeHome"
Write-Host ""
Write-Host "  Open a NEW terminal if the PATH change was just applied." -ForegroundColor Yellow
Write-Host ""
