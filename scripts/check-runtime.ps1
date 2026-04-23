# datatree :: check-runtime.ps1
# Read-only health check: reports presence + version of every runtime dep
# datatree needs on Windows.  Never installs, never modifies anything.
#
# Exit codes:
#   0 -- all REQUIRED deps present
#   1 -- one or more REQUIRED deps missing
#
# Usage:
#   pwsh ./check-runtime.ps1
#   pwsh ./check-runtime.ps1 -Json
#   pwsh ./check-runtime.ps1 -NoColor

[CmdletBinding()]
param(
    [switch] $Json,
    [switch] $NoColor
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$DataTreeHome = if ($env:DATATREE_HOME) { $env:DATATREE_HOME } else { Join-Path $HOME ".datatree" }
$LogDir       = Join-Path $DataTreeHome "logs"
$LogFile      = Join-Path $LogDir       "install.log"
$ModelDir     = Join-Path $DataTreeHome "llm"

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
if (-not (Test-Path $LogFile)) { New-Item -ItemType File -Force -Path $LogFile | Out-Null }

function Write-Log {
    param([string]$Message)
    $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    "[{0}] [CHECK] {1}" -f $ts, $Message | Add-Content -Path $LogFile -Encoding utf8
}

function Test-Cmd { param([string]$N) [bool](Get-Command $N -ErrorAction SilentlyContinue) }

function Get-Hint {
    param([string]$Dep)
    switch ($Dep) {
        "bun"       { return "winget install Oven-sh.Bun  (or: irm bun.sh/install.ps1 | iex)" }
        "python"    { return "winget install Python.Python.3.12" }
        "tesseract" { return "winget install UB-Mannheim.TesseractOCR" }
        "ffmpeg"    { return "winget install Gyan.FFmpeg" }
        default     { return "(see scripts/install-runtime.ps1)" }
    }
}

function Check-One {
    param([string]$Name, [string]$Bin, [bool]$Required)
    $present = $false
    $version = ""
    if (Test-Cmd $Bin) {
        $present = $true
        try {
            switch ($Bin) {
                "bun"       { $version = (& bun --version 2>$null).Trim() }
                "python"    { $version = ((& python  --version 2>&1) -replace 'Python ','').Trim() }
                "python3"   { $version = ((& python3 --version 2>&1) -replace 'Python ','').Trim() }
                "py"        { $version = ((& py      --version 2>&1) -replace 'Python ','').Trim() }
                "tesseract" {
                    $line = (& tesseract --version 2>&1 | Select-Object -First 1)
                    if ($line -match 'tesseract\s+(\S+)') { $version = $Matches[1] } else { $version = "present" }
                }
                "ffmpeg" {
                    $line = (& ffmpeg -version 2>$null | Select-Object -First 1)
                    if ($line -match 'ffmpeg version (\S+)') { $version = $Matches[1] } else { $version = "present" }
                }
                default { $version = "present" }
            }
        } catch { $version = "?" }
    }
    return [pscustomobject]@{
        Name     = $Name
        Present  = $present
        Version  = $version
        Hint     = if ($present) { "" } else { Get-Hint -Dep $Name.ToLower() }
        Required = $Required
    }
}

# ---------------------------------------- collect rows
$rows = New-Object System.Collections.Generic.List[object]

# Python: try python -> python3 -> py
$pyBin = $null
if     (Test-Cmd "python")  { $pyBin = "python" }
elseif (Test-Cmd "python3") { $pyBin = "python3" }
elseif (Test-Cmd "py")      { $pyBin = "py" }
if ($pyBin) {
    $rows.Add( (Check-One -Name "Python" -Bin $pyBin -Required $true) ) | Out-Null
} else {
    $rows.Add( [pscustomobject]@{ Name="Python"; Present=$false; Version=""; Hint=(Get-Hint "python"); Required=$true } )
}

$rows.Add( (Check-One -Name "Bun"       -Bin "bun"       -Required $true) ) | Out-Null
$rows.Add( (Check-One -Name "Tesseract" -Bin "tesseract" -Required $true) ) | Out-Null
$rows.Add( (Check-One -Name "ffmpeg"    -Bin "ffmpeg"    -Required $true) ) | Out-Null

# SQLite -- bundled
$rows.Add( [pscustomobject]@{ Name="SQLite"; Present=$true; Version="(bundled in datatree-store)"; Hint=""; Required=$true } )

# Models
$bgePath     = Join-Path $ModelDir "bge-small\model.onnx"
$phi3Path    = Join-Path $ModelDir "phi3-mini\model.onnx"
$whisperPath = Join-Path $ModelDir "faster-whisper-base\model.bin"

# Precompute version strings (PS 5.1 has no if-expression)
$bgePresent = Test-Path $bgePath
$bgeVer = ""
if ($bgePresent) { $bgeVer = "$bgePath (~33MB)" }

$phi3Present = Test-Path $phi3Path
$phi3Ver = "optional; ~2.4GB"
if ($phi3Present) { $phi3Ver = "$phi3Path (~2.4GB)" }

$whisperPresent = Test-Path $whisperPath
$whisperVer = "optional; ~140MB"
if ($whisperPresent) { $whisperVer = "$whisperPath (~140MB)" }

$rows.Add( [pscustomobject]@{
    Name="bge-small"; Present=$bgePresent; Version=$bgeVer;
    Hint="datatree models install --required --from <dir>"; Required=$true
} )
$rows.Add( [pscustomobject]@{
    Name="Phi-3"; Present=$phi3Present; Version=$phi3Ver;
    Hint="datatree models install --with-phi3 --from <dir>"; Required=$false
} )
$rows.Add( [pscustomobject]@{
    Name="faster-whisper"; Present=$whisperPresent; Version=$whisperVer;
    Hint="datatree models install --with-whisper --from <dir>"; Required=$false
} )

# ---------------------------------------- output
if ($Json) {
    $out = @{ deps = @($rows | ForEach-Object {
        @{ name=$_.Name; present=[bool]$_.Present; version=$_.Version; hint=$_.Hint; required=[bool]$_.Required }
    }) }
    $out | ConvertTo-Json -Depth 5
} else {
    $useColor = -not $NoColor -and $Host.UI.SupportsVirtualTerminal
    function W { param($t,$c) if ($useColor) { Write-Host $t -ForegroundColor $c -NoNewline } else { Write-Host $t -NoNewline } }
    Write-Host ""
    if ($useColor) { Write-Host "datatree :: runtime check" -ForegroundColor Cyan } else { Write-Host "datatree :: runtime check" }
    Write-Host ("-" * 60)
    foreach ($r in $rows) {
        if ($r.Present) {
            W "[+] " "Green"
            $v = if ($r.Version) { $r.Version } else { "present" }
            Write-Host ("{0,-15} {1}" -f $r.Name, $v)
        } else {
            if ($r.Required) {
                W "[x] " "Red"
                Write-Host ("{0,-15} NOT FOUND  (install: {1})" -f $r.Name, $r.Hint)
            } else {
                W "[!] " "Yellow"
                Write-Host ("{0,-15} NOT FOUND  (optional; {1})" -f $r.Name, $r.Hint)
            }
        }
    }
    Write-Host ""
}

$missingRequired = @($rows | Where-Object { -not $_.Present -and $_.Required }).Count
Write-Log "result: missing_required=$missingRequired"
if ($missingRequired -gt 0) { exit 1 } else { exit 0 }
