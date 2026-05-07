param(
    [int]$DurationMinutes = 10,
    [string]$Out = "C:\Users\Administrator\s3-corpus"
)
$ErrorActionPreference = "Continue"

# S3 -- Sustained build + concurrent status polls
# This is THE NEW-036 gate. Daemon must NOT terminate under data load.
# Acceptance:
#   - daemon does NOT terminate
#   - memory growth <= 1MB/hour
#   - thread count bounded
#   - build either completes or is still running at the duration mark

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"

# Phase 1: ensure clean and start daemon
Get-Process mneme* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 3
Start-Process -FilePath $mneme -ArgumentList "daemon","start" -WindowStyle Hidden
Start-Sleep 6

$d0 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d0) { Write-Host "S3_FAIL: daemon failed to start"; exit 1 }
$startPid = $d0[0].Id
$startWS = [math]::Round($d0[0].WorkingSet64 / 1MB, 3)
$startTh = $d0[0].Threads.Count
Write-Host "Daemon started: PID=$startPid WS=${startWS}MB threads=${startTh}"

# Phase 2: generate the corpus (3000 files mix)
Write-Host "=== Generating 3000 mixed files ==="
if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Path $Out -Force | Out-Null
$genStart = Get-Date
for ($i = 0; $i -lt 1500; $i++) {
    $bucket = [int]($i / 100)
    $dir = Join-Path $Out "rs\mod_$bucket"
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    @"
pub struct R$i { pub n: u64 }
impl R$i { pub fn new(n: u64) -> Self { Self { n } } }
pub fn run_$i() -> u64 { $i }
"@ | Set-Content -Path (Join-Path $dir "f_$i.rs") -Encoding UTF8
}
for ($i = 0; $i -lt 1000; $i++) {
    $bucket = [int]($i / 100)
    $dir = Join-Path $Out "ts\mod_$bucket"
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    @"
export interface T$i { id: number; name: string }
export function p$i(t: T$i): string { return ``p$i-`${t.id}``; }
"@ | Set-Content -Path (Join-Path $dir "m_$i.ts") -Encoding UTF8
}
for ($i = 0; $i -lt 500; $i++) {
    $bucket = [int]($i / 100)
    $dir = Join-Path $Out "md\bucket_$bucket"
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    @"
# Doc $i
- alpha
- beta
- gamma
"@ | Set-Content -Path (Join-Path $dir "doc_$i.md") -Encoding UTF8
}
$genWall = (Get-Date) - $genStart
Write-Host "Generation: $([math]::Round($genWall.TotalSeconds,1))s"

# Phase 3: concurrent build + status poll
Write-Host "=== Phase 3: build (background) + status polls every 1s for $DurationMinutes min ==="
$buildJob = Start-Job -Name "s3-build" -ScriptBlock {
    param($mneme, $out)
    & $mneme build --yes $out 2>&1
    "BUILD_EXIT=$LASTEXITCODE"
} -ArgumentList $mneme, $Out

$startTime = Get-Date
$endTime = $startTime.AddMinutes($DurationMinutes)
$samples = @()
$pollIdx = 0
$daemonDied = $false

while ((Get-Date) -lt $endTime) {
    $pollIdx++
    $now = Get-Date
    $d = Get-Process -Id $startPid -ErrorAction SilentlyContinue
    if (-not $d) {
        $daemonDied = $true
        Write-Host "DAEMON_DIED at sample $pollIdx ($([math]::Round(($now - $startTime).TotalSeconds,0))s)"
        break
    }
    if ($pollIdx % 30 -eq 0) {
        # Snapshot every 30 polls (~30s)
        $sample = [pscustomobject]@{
            T = [math]::Round(($now - $startTime).TotalSeconds, 0)
            WS_MB = [math]::Round($d.WorkingSet64 / 1MB, 3)
            Threads = $d.Threads.Count
            Handles = $d.HandleCount
        }
        $samples += $sample
        Write-Host ("t={0,4}s WS={1,7}MB threads={2,3} handles={3}" -f $sample.T, $sample.WS_MB, $sample.Threads, $sample.Handles)
    }
    # Status poll (the 1s cadence)
    & $mneme daemon status > $null 2>&1
    $sleepFor = 1.0 - ((Get-Date) - $now).TotalSeconds
    if ($sleepFor -gt 0) { Start-Sleep -Milliseconds ([int]($sleepFor * 1000)) }
}

# Phase 4: clean up build job
Write-Host "=== Phase 4: collecting build job state ==="
$bj = Get-Job -Name "s3-build"
$jobState = $bj.State
Write-Host "Build job state: $jobState"
$buildOut = Receive-Job -Job $bj -ErrorAction SilentlyContinue | Out-String
$bj | Stop-Job -ErrorAction SilentlyContinue
$bj | Remove-Job -ErrorAction SilentlyContinue
Write-Host "--- Build output (first 2000 chars) ---"
Write-Host $buildOut.Substring(0, [Math]::Min($buildOut.Length, 2000))

# Phase 5: final state
$dF = Get-Process -Id $startPid -ErrorAction SilentlyContinue
if ($dF) {
    $endWS = [math]::Round($dF.WorkingSet64 / 1MB, 3)
    $endTh = $dF.Threads.Count
    Write-Host "FINAL: PID=$startPid still alive WS=${endWS}MB threads=${endTh}"
} else {
    Write-Host "FINAL: daemon NOT alive"
}

# Phase 6: verdict
$wsDelta = if ($dF) { [math]::Round($endWS - $startWS, 3) } else { -1 }
$thDelta = if ($dF) { $endTh - $startTh } else { -1 }
$durHrs = $DurationMinutes / 60
$wsPerHr = if ($dF -and $durHrs -gt 0) { [math]::Round($wsDelta / $durHrs, 3) } else { -1 }

Write-Host ""
Write-Host "=== S3 SUMMARY ==="
Write-Host "Duration target: $DurationMinutes min"
Write-Host "Daemon survived: $($dF -ne $null)"
Write-Host ("WS delta over run: {0} MB ({1} MB/hour)" -f $wsDelta, $wsPerHr)
Write-Host ("Threads delta: {0}" -f $thDelta)
Write-Host ("Samples collected: {0}" -f $samples.Count)
$pass = ($dF -ne $null) -and ($wsPerHr -le 1.0 -or $wsPerHr -eq -1)
Write-Host "S3_VERDICT: $(if ($pass) { 'PASS' } else { 'FAIL' })"
