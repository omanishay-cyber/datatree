# Install mneme ML models from a LOCAL source path.
# REFUSES internet downloads. Models must be pre-staged locally.
#
# F18 (2026-05-05 audit): integrity verification.
# If a `<file>.sha256` sidecar exists alongside each staged model,
# this script verifies the SHA-256 sum BEFORE installing. Mismatches
# refuse install. Pass -NoVerify to skip (NOT recommended).
[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)] [string]$From,
    [switch]$WithPhi3,
    [switch]$WithWhisper,
    [switch]$Force,
    [switch]$NoVerify,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
function Write-Log([string]$msg) { if (-not $Quiet) { Write-Host "[mneme-models] $msg" } }

if (-not $From) {
    Write-Host @"
ERROR: -From <local-path> is required.

mneme REFUSES to fetch models from the internet. Models are large and
must be installed from a verified, locally-staged copy. Download the
models bundle separately, then point this script at the unpacked folder:

    install_models.ps1 -From C:\path\to\mneme-models

Required:  bge-small-en-v1.5.onnx
Optional:  phi-3-mini-q4_k_m.gguf, faster-whisper-base\
"@ -ForegroundColor Red
    exit 2
}

if (-not (Test-Path $From -PathType Container)) {
    Write-Host "ERROR: -From path is not a directory: $From" -ForegroundColor Red
    exit 1
}

$MnemeHome = if ($env:MNEME_HOME) { $env:MNEME_HOME } else { Join-Path $env:USERPROFILE '.mneme' }
$ModelDir     = Join-Path $MnemeHome 'models'
if (-not (Test-Path $ModelDir)) { New-Item -ItemType Directory -Force -Path $ModelDir | Out-Null }

function Test-Sha256([string]$src) {
    if ((Get-Item $src).PSIsContainer) {
        return $true
    }
    $sidecar = "$src.sha256"
    if (-not (Test-Path $sidecar)) {
        if ($NoVerify) {
            Write-Log "[warn]    no $sidecar -- skipping verification (-NoVerify)"
            return $true
        }
        Write-Host "ERROR: $src has no $sidecar sidecar." -ForegroundColor Red
        Write-Host "       Either stage the .sha256 alongside the model or pass -NoVerify." -ForegroundColor Red
        return $false
    }
    $expectedLine = (Get-Content -LiteralPath $sidecar -TotalCount 1).Trim()
    if (-not $expectedLine) {
        Write-Host "ERROR: $sidecar is empty." -ForegroundColor Red
        return $false
    }
    $expected = ($expectedLine -split '\s+')[0].ToLowerInvariant()
    $actual   = (Get-FileHash -Algorithm SHA256 -LiteralPath $src).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        Write-Host "ERROR: SHA-256 mismatch for $src" -ForegroundColor Red
        Write-Host "       expected: $expected" -ForegroundColor Red
        Write-Host "       actual:   $actual" -ForegroundColor Red
        Write-Host "       The staged file is not the file the sidecar describes." -ForegroundColor Red
        Write-Host "       Re-download from a trusted mirror or remove the sidecar to skip." -ForegroundColor Red
        return $false
    }
    Write-Log "[verify]  $src ($expected)"
    return $true
}

function Copy-Model($srcRel, $name, $label) {
    $src = Join-Path $From $srcRel
    if (-not (Test-Path $src)) {
        Write-Host "ERROR: $label not found at $src" -ForegroundColor Red
        return $false
    }
    if (-not (Test-Sha256 $src)) {
        return $false
    }
    $dest = Join-Path $ModelDir $name
    if ((Test-Path $dest) -and -not $Force) {
        Write-Log "[skip]    $label already installed at $dest (use -Force to overwrite)"
        return $true
    }
    if (Test-Path $dest) {
        $bak = "$dest.bak"
        if (Test-Path $bak) { Remove-Item -Recurse -Force $bak }
        Move-Item -Path $dest -Destination $bak
        Write-Log "[backup]  $dest -> $bak"
    }
    Write-Log "[install] $label -> $dest"
    if ((Get-Item $src).PSIsContainer) {
        Copy-Item -Recurse -Force -Path $src -Destination $dest
    } else {
        Copy-Item -Force -Path $src -Destination $dest
    }
    return $true
}

[void](Copy-Model 'bge-small-en-v1.5.onnx' 'bge-small-en-v1.5.onnx' 'bge-small-en-v1.5 ONNX (33MB)')

if ($WithPhi3) {
    [void](Copy-Model 'phi-3-mini-q4_k_m.gguf' 'phi-3-mini-q4_k_m.gguf' 'Phi-3-mini Q4_K_M (2.4GB)')
}

if ($WithWhisper) {
    [void](Copy-Model 'faster-whisper-base' 'faster-whisper-base' 'faster-whisper base (140MB)')
}

Write-Log ""
Write-Log "Models installed under: $ModelDir"
Get-ChildItem $ModelDir -Force | Select-Object Name, Length, LastWriteTime | Format-Table | Out-String | Write-Host
exit 0
