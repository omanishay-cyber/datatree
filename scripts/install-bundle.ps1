# datatree :: install-bundle.ps1
# Master end-to-end installer for Windows.  Single command does everything:
#   1. Detect OS + arch
#   2. Run check-runtime.ps1; collect missing
#   3. If missing required: install-runtime.ps1 -AutoInstall  (with confirm)
#   4. install-supervisor.ps1
#   5. install_models.ps1 -Required
#   6. start-daemon.ps1
#   7. Print next-steps banner
#
# Flags:
#   -Yes           : assume yes to all prompts
#   -NoStart       : skip step 6
#   -SkipModels    : skip step 5
#   -SkipRuntime   : skip steps 2-3
#   -From <dir>    : pass through to install-runtime.ps1 / install_models.ps1

[CmdletBinding()]
param(
    [switch] $Yes,
    [switch] $NoStart,
    [switch] $SkipModels,
    [switch] $SkipRuntime,
    [string] $From = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ScriptDir    = Split-Path -Parent $MyInvocation.MyCommand.Path
$DataTreeHome = if ($env:DATATREE_HOME) { $env:DATATREE_HOME } else { Join-Path $HOME ".datatree" }
$LogDir       = Join-Path $DataTreeHome "logs"
$LogFile      = Join-Path $LogDir       "install.log"
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
if (-not (Test-Path $LogFile)) { New-Item -ItemType File -Force -Path $LogFile | Out-Null }

function Write-Log {
    param([string]$Message)
    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    "[{0}] [BUNDLE] {1}" -f $ts, $Message | Add-Content -Path $LogFile -Encoding utf8
    Write-Host "[bundle] $Message" -ForegroundColor Magenta
}
function Step {
    param([string]$Message)
    Write-Log "===== STEP: $Message ====="
    Write-Host ""
    Write-Host "========== $Message ==========" -ForegroundColor Magenta
}
function Die {
    param([string]$Message, [int]$Code = 1)
    Write-Log "FATAL: $Message"
    Write-Host ""
    Write-Host "[bundle][FATAL] $Message" -ForegroundColor Red
    Write-Host "See log: $LogFile"
    exit $Code
}

# --------- step 1
Step "1. Detect OS + arch"
$arch = $env:PROCESSOR_ARCHITECTURE
Write-Log "OS=Windows arch=$arch"
if (-not [Environment]::OSVersion.Platform.ToString().StartsWith("Win")) {
    Die "Unsupported OS (use install-bundle.sh on Linux/macOS)" 3
}

# --------- pass-through args
$ptArgs = @()
if ($Yes)            { $ptArgs += "-Yes" }
if (-not [string]::IsNullOrEmpty($From)) { $ptArgs += "-From"; $ptArgs += $From }

# --------- step 2 + 3
if ($SkipRuntime) {
    Write-Log "Skipping runtime steps (-SkipRuntime)"
} else {
    Step "2. Check runtime dependencies"
    $checkScript = Join-Path $ScriptDir "check-runtime.ps1"
    & pwsh -NoProfile -File $checkScript
    $checkRc = $LASTEXITCODE
    Write-Log "check-runtime.ps1 exit=$checkRc"

    if ($checkRc -ne 0) {
        Step "3. Auto-install missing runtime dependencies"
        if (-not $Yes) {
            $resp = Read-Host "Some required runtime deps are missing. Install now? [y/N]"
            if ($resp -notmatch '^(y|yes)$') { Die "User declined runtime install" 1 }
        }
        $installScript = Join-Path $ScriptDir "install-runtime.ps1"
        & pwsh -NoProfile -File $installScript -AutoInstall @ptArgs
        if ($LASTEXITCODE -ne 0) { Die "install-runtime.ps1 failed" 2 }
    } else {
        Write-Log "All runtime deps present; skipping install-runtime.ps1"
    }
}

# --------- step 4
Step "4. Install supervisor (datatree-supervisor)"
$supScript = Join-Path $ScriptDir "install-supervisor.ps1"
if (Test-Path $supScript) {
    & pwsh -NoProfile -File $supScript
    if ($LASTEXITCODE -ne 0) { Die "install-supervisor.ps1 failed" 4 }
} else {
    Write-Log "WARN: install-supervisor.ps1 not found; skipping"
}

# --------- step 5
if ($SkipModels) {
    Write-Log "Skipping models (-SkipModels)"
} else {
    Step "5. Install required models (bge-small)"
    $modelsScript = Join-Path $ScriptDir "install_models.ps1"
    if (Test-Path $modelsScript) {
        & pwsh -NoProfile -File $modelsScript -Required @ptArgs
        if ($LASTEXITCODE -ne 0) { Die "install_models.ps1 -Required failed" 5 }
    } else {
        Write-Log "WARN: install_models.ps1 not found; skipping"
    }
}

# --------- step 6
if ($NoStart) {
    Write-Log "Skipping daemon start (-NoStart)"
} else {
    Step "6. Start datatree daemon"
    $startScript = Join-Path $ScriptDir "start-daemon.ps1"
    if (Test-Path $startScript) {
        & pwsh -NoProfile -File $startScript
        if ($LASTEXITCODE -ne 0) { Die "start-daemon.ps1 failed" 6 }
    } else {
        Write-Log "WARN: start-daemon.ps1 not found; skipping"
    }
}

# --------- step 7
Step "7. Done"
@"

   datatree is installed.

   Next:  open Claude Code in your project and run

          /plugin install datatree

   Useful commands:
     pwsh scripts\check-runtime.ps1           # health-check
     pwsh scripts\start-daemon.ps1            # start
     pwsh scripts\stop-daemon.ps1             # stop
     pwsh scripts\uninstall-runtime.ps1       # remove deps datatree installed

"@ | Write-Host

Write-Log "bundle install complete"
exit 0
