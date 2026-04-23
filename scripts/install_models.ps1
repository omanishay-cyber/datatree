# Install datatree ML models from a LOCAL source path.
# REFUSES internet downloads. Models must be pre-staged locally.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)] [string]$From,
    [switch]$WithPhi3,
    [switch]$WithWhisper,
    [switch]$Force,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
function Write-Log([string]$msg) { if (-not $Quiet) { Write-Host "[datatree-models] $msg" } }

if (-not $From) {
    Write-Host @"
ERROR: -From <local-path> is required.

datatree REFUSES to fetch models from the internet. Models are large and
must be installed from a verified, locally-staged copy. Download the
models bundle separately, then point this script at the unpacked folder:

    install_models.ps1 -From C:\path\to\datatree-models

Required:  bge-small-en-v1.5.onnx
Optional:  phi-3-mini-q4_k_m.gguf, faster-whisper-base\
"@ -ForegroundColor Red
    exit 2
}

if (-not (Test-Path $From -PathType Container)) {
    Write-Host "ERROR: -From path is not a directory: $From" -ForegroundColor Red
    exit 1
}

$DatatreeHome = if ($env:DATATREE_HOME) { $env:DATATREE_HOME } else { Join-Path $env:USERPROFILE '.datatree' }
$ModelDir     = Join-Path $DatatreeHome 'models'
if (-not (Test-Path $ModelDir)) { New-Item -ItemType Directory -Force -Path $ModelDir | Out-Null }

function Copy-Model($srcRel, $name, $label) {
    $src = Join-Path $From $srcRel
    if (-not (Test-Path $src)) {
        Write-Host "ERROR: $label not found at $src" -ForegroundColor Red
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
