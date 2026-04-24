# Mneme - one-line installer for Windows (v0.3.1+)
#
# Usage (PowerShell, as current user - elevation is optional, see below):
#   iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install.ps1 | iex
#
# What it does, in order:
#   1. Ensures Bun is installed (runs the official Bun installer if not
#      already present). Bun is the only runtime dependency - mneme's MCP
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
#   7. Registers the mneme MCP server with Claude Code (MCP entry only -
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

Write-Step "mneme - one-line installer"
Write-Info ("target   : {0}" -f $MnemeHome)
Write-Info ("bin      : {0}" -f $BinDir)
Write-Info ("elevated : {0}" -f (Test-IsElevated))
Write-Host ""

# ============================================================================
# Step 0 - Stop any running mneme processes (upgrade safety)
# ============================================================================
#
# If an existing daemon is running, the mneme.exe / mneme-daemon.exe /
# worker binaries are file-locked. Expand-Archive silently skips locked
# files, leaving a mixed-version install where the *.dll metadata says
# v0.3.1 but some of the executable bodies are still v0.3.0. That looks
# identical to "install succeeded" but actually shipped broken binaries.
#
# Unconditional stop is safe: if no daemon is running, this is a no-op.
# The supervisor is restarted later in step 6.

Write-Step "step 0/8 - stop any existing mneme daemon + workers"

$tries = 0
do {
    $running = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^mneme' }
    if ($running) {
        Write-Info ("stopping {0} mneme process(es): {1}" -f $running.Count, (($running.ProcessName | Sort-Object -Unique) -join ', '))
        $running | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
    $tries++
} while ($running -and $tries -lt 5)

$leftover = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^mneme' }
if ($leftover) {
    Write-Warn ("could not stop all mneme processes ({0} still running)" -f $leftover.Count)
    Write-Warn "Expand-Archive may skip locked binaries - close any mneme window and rerun"
} else {
    Write-OK "no mneme processes running - safe to extract"
}

# ============================================================================
# Step 1 - Check + install runtime prerequisites
# ============================================================================
#
# Three tools matter for a full mneme + Claude-Code experience:
#
#   Bun       - mneme's MCP server (`mneme mcp stdio`) launches TypeScript
#               via `bun`. Required for `/mn-*` commands in Claude Code.
#   Node.js   - only needed if the user wants the Claude Code CLI
#               (`npm install -g @anthropic-ai/claude-code`). Not strictly
#               required to run mneme itself, but the whole point of mneme
#               is to serve Claude Code, so we install it by default.
#   git       - only needed for `mneme build` on git repos (so mneme can
#               pin the indexed commit SHA per project). Mneme works
#               without it; just no git metadata in the graph.
#
# Rust is deliberately NOT installed - mneme ships pre-built binaries.
# Python is not needed for v0.3.1 (multimodal sidecar is feature-gated).
#
# Every check below follows the same pattern: detect on PATH, detect at
# standard user-scope install path, install if missing. All installs are
# user-scope where possible (no admin required); fall back to system
# installer where the official path does. Idempotent on every re-run.

function Test-Tool {
    param([string]$Name, [string]$FallbackPath)
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    if ($FallbackPath -and (Test-Path $FallbackPath)) { return $FallbackPath }
    return $null
}

# --- 1a. Bun (required for MCP server) --------------------------------------
Write-Step "step 1/8 - Bun runtime (required for mneme MCP)"

$BunFallback = Join-Path $env:USERPROFILE '.bun\bin\bun.exe'
$BunExe = Test-Tool -Name 'bun' -FallbackPath $BunFallback

if ($BunExe) {
    try {
        $BunVer = (& $BunExe --version 2>$null).Trim()
        Write-OK ("bun $BunVer present at $BunExe")
    } catch {
        Write-OK ("bun present at $BunExe (version check failed, continuing)")
    }
} else {
    Write-Info "bun not found - installing via direct GitHub release download"
    try {
        # The bun.sh/install.ps1 script uses `curl.exe -#` which errors in
        # non-interactive sessions (see v0.3.0 install-report). We pull the
        # release ZIP ourselves - works in any shell context.
        $bunBin = Join-Path $env:USERPROFILE '.bun\bin'
        New-Item -ItemType Directory -Force -Path $bunBin | Out-Null
        $bunZip = Join-Path $env:TEMP 'bun-windows-x64.zip'
        Invoke-WebRequest -Uri 'https://github.com/oven-sh/bun/releases/latest/download/bun-windows-x64.zip' -OutFile $bunZip -UseBasicParsing
        Expand-Archive -Path $bunZip -DestinationPath $env:TEMP -Force
        Copy-Item (Join-Path $env:TEMP 'bun-windows-x64\bun.exe') $bunBin -Force
        # Persist PATH (user-scope)
        $userPath = [Environment]::GetEnvironmentVariable('PATH','User')
        if ($userPath -notmatch [regex]::Escape($bunBin)) {
            [Environment]::SetEnvironmentVariable('PATH', "$userPath;$bunBin", 'User')
        }
        $env:PATH = "$env:PATH;$bunBin"
        $BunExe = Join-Path $bunBin 'bun.exe'
        Write-OK ("bun $((& $BunExe --version 2>$null).Trim()) installed at $BunExe")
    } catch {
        Write-Warn ("Bun install failed: {0}" -f $_.Exception.Message)
        Write-Warn "mneme CLI will still work, but MCP tools in Claude Code will not"
        Write-Warn "Manual install later: https://bun.sh/install"
    }
}

# --- 1b. Node.js + npm (for Claude Code CLI) --------------------------------
Write-Step "step 1b/8 - Node.js + npm (for Claude Code CLI)"

$NodeExe = Test-Tool -Name 'node' -FallbackPath 'C:\Program Files\nodejs\node.exe'

if ($NodeExe) {
    $NodeVer = (& $NodeExe --version 2>$null).Trim()
    Write-OK ("node $NodeVer present at $NodeExe")
} else {
    Write-Info "node not found - installing Node.js LTS via direct MSI"
    try {
        $nodeUrl = 'https://nodejs.org/dist/v22.13.1/node-v22.13.1-x64.msi'
        $nodeMsi = Join-Path $env:TEMP 'node-lts.msi'
        Invoke-WebRequest -Uri $nodeUrl -OutFile $nodeMsi -UseBasicParsing
        $p = Start-Process msiexec.exe -ArgumentList '/i', "`"$nodeMsi`"", '/qn', '/norestart' -Wait -PassThru
        if ($p.ExitCode -eq 0) {
            # Refresh session PATH so subsequent steps find npm
            $env:PATH = [Environment]::GetEnvironmentVariable('PATH','Machine') + ';' + [Environment]::GetEnvironmentVariable('PATH','User')
            $NodeExe = Test-Tool -Name 'node' -FallbackPath 'C:\Program Files\nodejs\node.exe'
            if ($NodeExe) {
                Write-OK ("node $((& $NodeExe --version 2>$null).Trim()) installed")
            } else {
                Write-Warn "node installer exited 0 but node not on PATH - re-open shell"
            }
        } else {
            Write-Warn ("node MSI exited with code {0}" -f $p.ExitCode)
        }
    } catch {
        Write-Warn ("Node.js install failed: {0}" -f $_.Exception.Message)
        Write-Warn "Claude Code CLI will not be installable until Node is present"
        Write-Warn "Manual install: https://nodejs.org/"
    }
}

# --- 1c. git (optional, for `mneme build` on git repos) ---------------------
Write-Step "step 1c/8 - git (optional, for richer project metadata)"

$GitExe = Test-Tool -Name 'git' -FallbackPath 'C:\Program Files\Git\cmd\git.exe'

if ($GitExe) {
    $GitVer = (& $GitExe --version 2>$null).Trim()
    Write-OK ("git $GitVer present at $GitExe")
} else {
    Write-Info "git not found - installing Git for Windows (silent)"
    try {
        $gitUrl = 'https://github.com/git-for-windows/git/releases/download/v2.48.1.windows.1/Git-2.48.1-64-bit.exe'
        $gitExe = Join-Path $env:TEMP 'git-setup.exe'
        Invoke-WebRequest -Uri $gitUrl -OutFile $gitExe -UseBasicParsing
        $p = Start-Process $gitExe -ArgumentList '/VERYSILENT','/NORESTART','/NOCANCEL','/SP-','/SUPPRESSMSGBOXES' -Wait -PassThru
        if ($p.ExitCode -eq 0) {
            $env:PATH = [Environment]::GetEnvironmentVariable('PATH','Machine') + ';' + [Environment]::GetEnvironmentVariable('PATH','User')
            Write-OK "git installed"
        } else {
            Write-Warn ("git installer exited with code {0}" -f $p.ExitCode)
        }
    } catch {
        Write-Warn ("git install failed: {0}" -f $_.Exception.Message)
        Write-Warn "mneme will still work without git; just no commit-SHA metadata"
    }
}

# ============================================================================
# Step 2 - Fetch latest release metadata
# ============================================================================

Write-Step "step 2/8 - fetching latest release metadata"

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
    Write-Warn "       the release workflow may still be building - retry in ~15 min."
    exit 1
}
Write-OK ("release {0} - asset {1} ({2:N1} MB)" -f $Release.tag_name, $Asset, ($AssetEntry.size / 1MB))

# ============================================================================
# Step 3 - Download + extract
# ============================================================================

Write-Step "step 3/8 - downloading + extracting"

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
# Step 4 - Windows Defender exclusions
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

Write-Step "step 4/8 - Windows Defender exclusions"

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
    Write-Warn "not running elevated - skipping Defender exclusion"
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
# Step 5 - Add bin dir to user PATH
# ============================================================================

Write-Step "step 5/8 - updating user PATH"

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
# Step 6 - Start the mneme daemon
# ============================================================================

Write-Step "step 6/8 - starting mneme daemon"

$MnemeBin = Join-Path $BinDir 'mneme.exe'
if (-not (Test-Path $MnemeBin)) {
    Write-Warn ("mneme.exe not found at {0} - did extraction succeed?" -f $MnemeBin)
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
# Step 7 - Register MCP with Claude Code (NO hook injection, NO manifest)
# ============================================================================
#
# v0.3.1 hard rule: the installer only writes a single mcpServers.mneme
# entry into ~/.claude.json. It does NOT touch ~/.claude/settings.json.
# It does NOT write a CLAUDE.md manifest by default. Those changes were
# what poisoned Claude Code on v0.3.0 - see F-011/F-012 in the install
# report. Power users who want the manifest can run without
# --skip-manifest later.

Write-Step "step 7/8 - registering MCP with Claude Code"

if (-not (Test-Path $MnemeBin)) {
    Write-Warn "mneme.exe not present, skipping MCP registration"
} else {
    try {
        & $MnemeBin register-mcp --platform claude-code 2>&1 | ForEach-Object { Write-Info $_ }
        if ($LASTEXITCODE -eq 0) {
            Write-OK "Claude Code MCP registration complete"
        } else {
            Write-Warn ("mneme register-mcp exited {0} - MCP may not be registered" -f $LASTEXITCODE)
            Write-Warn "run manually later: mneme register-mcp --platform claude-code"
        }
    } catch {
        Write-Warn ("MCP registration error: {0}" -f $_.Exception.Message)
        Write-Warn "run manually later: mneme register-mcp --platform claude-code"
    }
}

# ============================================================================
# Step 8 - Done
# ============================================================================

Write-Step "step 8/8 - complete"
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  mneme installed - v$($Release.tag_name)" -ForegroundColor Green
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
