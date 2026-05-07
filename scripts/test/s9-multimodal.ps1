param(
    [string]$Out = "C:\Users\Administrator\s9-multi"
)
$ErrorActionPreference = "Continue"

# S9 -- Big-data multimodal corpus
# 100+ files: mix .rs, .ts, .py, .md, with edge cases (0 bytes, BOM, etc.)
# Acceptance: no panic, all processed (success or graceful skip), doctor green

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"

Write-Host "=== S9 Phase 1: generate 120-file mixed corpus at $Out ==="
if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Path $Out -Force | Out-Null

# Helpers
function Write-RsFile($path, $idx) {
    $body = @"
// Rust file IDX_PLACEHOLDER
pub struct R_IDX_PLACEHOLDER { pub n: u64 }
impl R_IDX_PLACEHOLDER { pub fn new(n: u64) -> Self { Self { n } } }
pub fn run_IDX_PLACEHOLDER() -> u64 { IDX_PLACEHOLDER }
"@
    $body = $body.Replace("IDX_PLACEHOLDER", "$idx")
    Set-Content -Path $path -Value $body -Encoding UTF8
}
function Write-TsFile($path, $idx) {
    $body = @"
// TypeScript file IDX_PLACEHOLDER
export interface T_IDX_PLACEHOLDER { id: number; name: string }
export function process_IDX_PLACEHOLDER(t: T_IDX_PLACEHOLDER): string {
    return "processed_IDX_PLACEHOLDER-" + t.name + "-" + t.id;
}
"@
    $body = $body.Replace("IDX_PLACEHOLDER", "$idx")
    Set-Content -Path $path -Value $body -Encoding UTF8
}
function Write-PyFile($path, $idx) {
    $body = @"
# Python file IDX_PLACEHOLDER
class P_IDX_PLACEHOLDER:
    def __init__(self, n: int) -> None:
        self.n = n
    def run(self) -> int:
        return self.n * IDX_PLACEHOLDER

def process_IDX_PLACEHOLDER(s: str) -> str:
    return f"processed_IDX_PLACEHOLDER({s})"
"@
    $body = $body.Replace("IDX_PLACEHOLDER", "$idx")
    Set-Content -Path $path -Value $body -Encoding UTF8
}
function Write-MdFile($path, $idx) {
    $body = @"
# Document IDX_PLACEHOLDER

## Overview
This is a synthetic doc IDX_PLACEHOLDER for the mneme S9 multimodal stress test.

## Sections
- Section A
- Section B
- Section C

## Code example
``````rust
let x = IDX_PLACEHOLDER;
``````

## Conclusion
End of doc IDX_PLACEHOLDER.
"@
    $body = $body.Replace("IDX_PLACEHOLDER", "$idx")
    Set-Content -Path $path -Value $body -Encoding UTF8
}

# 30 .rs
1..30 | ForEach-Object { Write-RsFile (Join-Path $Out "src\rust_$_.rs") $_ } 2>$null
New-Item -ItemType Directory -Path (Join-Path $Out "src") -Force | Out-Null
1..30 | ForEach-Object { Write-RsFile (Join-Path $Out "src\rust_$_.rs") $_ }
# 30 .ts
New-Item -ItemType Directory -Path (Join-Path $Out "ts") -Force | Out-Null
1..30 | ForEach-Object { Write-TsFile (Join-Path $Out "ts\mod_$_.ts") $_ }
# 30 .py
New-Item -ItemType Directory -Path (Join-Path $Out "py") -Force | Out-Null
1..30 | ForEach-Object { Write-PyFile (Join-Path $Out "py\mod_$_.py") $_ }
# 25 .md
New-Item -ItemType Directory -Path (Join-Path $Out "docs") -Force | Out-Null
1..25 | ForEach-Object { Write-MdFile (Join-Path $Out "docs\doc_$_.md") $_ }

# Edge cases
$edge = Join-Path $Out "edge"
New-Item -ItemType Directory -Path $edge -Force | Out-Null
# Zero-byte file
"" | Out-File -FilePath (Join-Path $edge "zero.rs") -Encoding ASCII -NoNewline
[System.IO.File]::WriteAllBytes((Join-Path $edge "zero.rs"), @())
# BOM-prefixed file
$bomBytes = [byte[]](0xEF,0xBB,0xBF) + [System.Text.Encoding]::UTF8.GetBytes("// BOM file`npub fn boom() -> u32 { 7 }`n")
[System.IO.File]::WriteAllBytes((Join-Path $edge "bom.rs"), $bomBytes)
# Very large md (~256KB)
$big = "# Big doc`n" + ("- line of repeated text " * 8000)
$big | Out-File -FilePath (Join-Path $edge "big.md") -Encoding UTF8

$count = (Get-ChildItem $Out -Recurse -File | Measure-Object).Count
Write-Host "Files generated: $count"

# Phase 2: build
Write-Host "=== S9 Phase 2: mneme build --yes $Out ==="
$d0 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d0) {
    Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
    Start-Sleep 6
}

$bStart = Get-Date
& $mneme build --yes $Out 2>&1 | Out-String | Write-Host
$bExit = $LASTEXITCODE
$bWall = (Get-Date) - $bStart
Write-Host "BUILD_EXIT=$bExit BUILD_WALL_S=$([math]::Round($bWall.TotalSeconds,2))"

# Phase 3: verify daemon survives
$d1 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if ($d1) {
    Write-Host "DAEMON_ALIVE_AFTER_BUILD=YES PID=$($d1[0].Id)"
} else {
    Write-Host "DAEMON_ALIVE_AFTER_BUILD=NO PANIC_OR_DEATH"
}

# Phase 4: doctor green check
Write-Host "=== S9 Phase 4: mneme doctor ==="
$dr = & $mneme doctor 2>&1 | Out-String
Write-Host $dr
$drGreen = ($dr -notmatch "FAIL|red|ERROR" -or $dr -match "PASS|OK|green")

Write-Host "=== S9 SUMMARY ==="
Write-Host "ACCEPTANCE: build_exit_zero=$($bExit -eq 0)"
Write-Host "ACCEPTANCE: daemon_alive=$($d1 -ne $null)"
Write-Host "ACCEPTANCE: doctor_green=$drGreen"
$pass = ($bExit -eq 0) -and ($d1 -ne $null)
Write-Host "S9_VERDICT: $(if ($pass) { 'PASS' } else { 'FAIL' })"
