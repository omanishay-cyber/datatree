param(
    [int]$Files = 5000,
    [string]$Out = "C:\Users\Administrator\synth5k"
)
$ErrorActionPreference = "Continue"

# Phase 1: generate synthetic .rs files
Write-Host "=== S2 Phase 1: generating $Files synthetic .rs files ==="
$genStart = Get-Date
if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Path $Out -Force | Out-Null

# Group into 100-file dirs to avoid one huge directory
for ($i = 0; $i -lt $Files; $i++) {
    $bucket = [int]($i / 100)
    $dir = Join-Path $Out "mod_$bucket"
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $file = Join-Path $dir "f_$i.rs"
    $body = @"
// Synthetic file $i
pub struct Thing$i {
    pub id: u64,
    pub name: String,
}

impl Thing$i {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn rename(&mut self, n: String) { self.name = n; }
}

pub fn process_$i(input: &str) -> String {
    format!("processed_$i({})", input)
}
"@
    Set-Content -Path $file -Value $body -Encoding UTF8
}
$genWall = (Get-Date) - $genStart
Write-Host "Generation: $($genWall.TotalSeconds)s for $Files files"

# Phase 2: pre-build snapshot
Write-Host "=== S2 Phase 2: pre-build daemon snapshot ==="
$d0 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if ($d0) {
    Write-Host "daemon WS=$([math]::Round($d0.WorkingSet64/1MB,1))MB threads=$($d0.Threads.Count) PID=$($d0.Id)"
} else {
    Write-Host "WARN: daemon not running, starting..."
    Start-Process -FilePath "C:\Users\Administrator\.mneme\bin\mneme.exe" -ArgumentList "daemon","start" -WindowStyle Hidden
    Start-Sleep 5
}

# Phase 3: run mneme build --yes
Write-Host "=== S2 Phase 3: running 'mneme build --yes $Out' ==="
$bStart = Get-Date
& "C:\Users\Administrator\.mneme\bin\mneme.exe" build --yes $Out 2>&1
$bExit = $LASTEXITCODE
$bWall = (Get-Date) - $bStart
Write-Host "BUILD_EXIT=$bExit BUILD_WALL_S=$($bWall.TotalSeconds)"

# Phase 4: post-build snapshot + daemon survives check
Write-Host "=== S2 Phase 4: post-build state ==="
$d1 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if ($d1) {
    Write-Host "daemon WS=$([math]::Round($d1.WorkingSet64/1MB,1))MB threads=$($d1.Threads.Count) PID=$($d1.Id) DAEMON_ALIVE=YES"
} else {
    Write-Host "DAEMON_ALIVE=NO -- DAEMON_DIED_DURING_BUILD"
}

# Phase 5: doctor green check
Write-Host "=== S2 Phase 5: mneme doctor ==="
& "C:\Users\Administrator\.mneme\bin\mneme.exe" doctor 2>&1 | Out-String

Write-Host "=== S2 done ==="
Write-Host "ACCEPTANCE: build_wall_s<=120: $($bWall.TotalSeconds -le 120)"
Write-Host "ACCEPTANCE: daemon_alive: $($d1 -ne $null)"
Write-Host "ACCEPTANCE: build_exit_zero: $($bExit -eq 0)"
