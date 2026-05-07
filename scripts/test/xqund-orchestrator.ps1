param()
$ErrorActionPreference = "Continue"

# Phase X+Q+Un+Re+D orchestrator
# Runs phases in safe order:
#   1. X (cross-test) - non-destructive
#   2. Q (edge cases) - non-destructive
#   3. D (privacy) - non-destructive, BEFORE uninstall (needs daemon running)
#   4. Re (resilience) - kills daemon mid-way; restart needed
#   5. Un (uninstall) - destructive, must be LAST
#
# Each phase prints its own result block. Final summary at the end.

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"

function Ensure-Daemon {
    $d = Get-Process mneme-daemon -ErrorAction SilentlyContinue
    if (-not $d) {
        Write-Host "[orchestrator] starting daemon..."
        Start-Process -FilePath $mneme -ArgumentList "daemon", "start" -WindowStyle Hidden
        Start-Sleep 6
        $d = Get-Process mneme-daemon -ErrorAction SilentlyContinue
    }
    return ($d -ne $null)
}

function Step-Header {
    param($name)
    Write-Host ""
    Write-Host "================================================================================"
    Write-Host "==  $name"
    Write-Host "================================================================================"
}

# ---------- prepare X2 corpus (small synthetic) ----------
$x2Corpus = "C:\x2-corpus"
if (-not (Test-Path $x2Corpus)) {
    New-Item -ItemType Directory -Path $x2Corpus -Force | Out-Null
    1..50 | ForEach-Object {
        Set-Content -Path (Join-Path $x2Corpus "f$_.rs") -Value "pub fn f$_() -> u32 { $_ }`n" -Encoding UTF8
    }
    Write-Host "[orchestrator] x2 corpus seeded (50 files)"
}

Ensure-Daemon | Out-Null

# ---------- Phase X ----------
Step-Header "Phase X -- Cross-test"
$xOut = & node "C:\Users\Administrator\x-cross-test.mjs" 2>&1 | Out-String
Write-Host $xOut

# ---------- Phase Q ----------
Step-Header "Phase Q -- Edge cases"
Ensure-Daemon | Out-Null
$qOut = & node "C:\Users\Administrator\q-edge-cases.mjs" 2>&1 | Out-String
Write-Host $qOut

# ---------- Phase D ----------
Step-Header "Phase D -- Privacy"
Ensure-Daemon | Out-Null
$dOut = & powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\Administrator\d-privacy.ps1" 2>&1 | Out-String
Write-Host $dOut

# ---------- Phase Re ----------
Step-Header "Phase Re -- Resilience"
Ensure-Daemon | Out-Null
$reOut = & powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\Administrator\re-resilience.ps1" 2>&1 | Out-String
Write-Host $reOut

# ---------- Phase Un (DESTRUCTIVE -- runs last) ----------
Step-Header "Phase Un -- Uninstall (DESTRUCTIVE)"
Ensure-Daemon | Out-Null
$unOut = & powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\Administrator\un-uninstall.ps1" -SkipReinstall 2>&1 | Out-String
Write-Host $unOut

# ---------- Final summary ----------
Step-Header "ORCHESTRATOR FINAL"
Write-Host "All five phases complete. Per-phase JSON blocks above."
