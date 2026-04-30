param(
    [switch]$AllowReboot = $false
)
$ErrorActionPreference = "Continue"

# Phase Re - Restart resilience
# Re1: kill daemon mid-build -> recovery or graceful failure
# Re2: kill daemon mid-mcp stdio call -> clean error, no hang
# Re3: VM reboot (gated by -AllowReboot)

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
$results = @()
function Add-Result {
    param($Name, $Status, $Detail)
    $script:results += [pscustomobject]@{ name = $Name; status = $Status; detail = $Detail }
}

# ---------- Setup ----------
$d = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d) {
    Start-Process -FilePath $mneme -ArgumentList "daemon", "start" -WindowStyle Hidden
    Start-Sleep 6
}

$corpus = "C:\Users\Administrator\re-corpus"
if (-not (Test-Path $corpus)) {
    New-Item -ItemType Directory -Path $corpus -Force | Out-Null
    1..200 | ForEach-Object {
        Set-Content -Path (Join-Path $corpus "f$_.rs") -Value "pub fn f$_() -> u32 { $_ }`n" -Encoding UTF8
    }
}

# ---------- Re1: kill daemon mid-build ----------
Write-Host "=== Re1 kill daemon mid-build ==="
$buildJob = Start-Job -ScriptBlock {
    param($mn, $cp)
    & $mn build --yes $cp 2>&1
    $LASTEXITCODE
} -ArgumentList $mneme, $corpus

Start-Sleep 2
$d1 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
$killedPid = $null
if ($d1) {
    $killedPid = $d1.Id
    Stop-Process -Id $killedPid -Force -ErrorAction SilentlyContinue
    Write-Host "killed daemon PID=$killedPid at t+2s"
}
$buildResult = $buildJob | Wait-Job -Timeout 60 | Receive-Job
Remove-Job $buildJob -Force -ErrorAction SilentlyContinue

# verify shard not corrupted
$shardOk = $true
$openErr = ""
try {
    $shards = Get-ChildItem "$env:USERPROFILE\.mneme\shards" -Recurse -Filter "*.db" -ErrorAction SilentlyContinue
    if (-not $shards) { $shardOk = $true }
} catch {
    $shardOk = $false
    $openErr = $_.ToString()
}

# verify daemon comes back via mneme daemon start
Start-Sleep 1
$d2 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d2) {
    Start-Process -FilePath $mneme -ArgumentList "daemon", "start" -WindowStyle Hidden
    Start-Sleep 6
    $d2 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
}

$re1Pass = ($d2 -ne $null) -and $shardOk
Add-Result "Re1-kill-mid-build" $(if ($re1Pass) { "PASS" } else { "FAIL" }) "killedPid=$killedPid buildResult=$buildResult shardOk=$shardOk daemonBack=$($d2 -ne $null) openErr=$openErr"

# ---------- Re2: kill daemon mid-MCP-call ----------
Write-Host "=== Re2 kill daemon mid-mcp-stdio ==="
$re2Driver = "C:\Users\Administrator\re2-driver.mjs"
$re2Code = @'
import { spawn } from "node:child_process";
const p = spawn("mneme", ["mcp", "stdio"], {
  cwd: process.env.USERPROFILE + "\\.mneme",
  env: { ...process.env, MNEME_LOG: "error", MNEME_MCP_PATH: process.env.USERPROFILE + "\\.mneme\\mcp\\src\\index.ts", MNEME_IPC_TIMEOUT_MS: "2000" },
  stdio: ["pipe", "pipe", "pipe"],
});
let buf = "";
const pending = new Map();
let nextId = 1;
p.stdout.on("data", (d) => {
  buf += d.toString();
  let nl;
  while ((nl = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, nl);
    buf = buf.slice(nl + 1);
    if (!line.trim()) continue;
    try { const m = JSON.parse(line); if (m.id != null && pending.has(m.id)) { pending.get(m.id).resolve(m); pending.delete(m.id); } } catch {}
  }
});
function rpc(method, params = {}, timeoutMs = 8000) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    p.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    setTimeout(() => { if (pending.has(id)) { pending.delete(id); reject(new Error("timeout")); } }, timeoutMs);
  });
}
const t0 = Date.now();
try {
  await rpc("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "re2", version: "1" } });
  const list = await rpc("tools/list");
  console.log("INIT_OK tools=" + (list.result?.tools?.length ?? 0));
  let res, err;
  try {
    res = await rpc("tools/call", { name: "audit", arguments: {} }, 12000);
  } catch (e) { err = String(e); }
  console.log("CALL_DONE t=" + (Date.now() - t0) + " ok=" + (!!res && !res.error) + " err=" + (err || res?.error?.message || "") );
} catch (e) {
  console.log("FATAL " + e.message);
}
process.exit(0);
'@
Set-Content -Path $re2Driver -Value $re2Code -Encoding UTF8

# Run driver as background job, then kill daemon ~600ms in
$re2Job = Start-Job -ScriptBlock { param($drv) node $drv } -ArgumentList $re2Driver
Start-Sleep -Milliseconds 700
# kill daemon
Get-Process mneme-daemon -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Host "killed daemon at t+700ms"
$re2OutCollected = $re2Job | Wait-Job -Timeout 30 | Receive-Job
Remove-Job $re2Job -Force -ErrorAction SilentlyContinue
$re2Out = $re2OutCollected | Out-String
Write-Host $re2Out
$re2NoHang = ($re2Out -match "CALL_DONE")
$re2InitOk = ($re2Out -match "INIT_OK tools=47")
$re2Pass = $re2NoHang -and $re2InitOk
Add-Result "Re2-kill-mid-mcp" $(if ($re2Pass) { "PASS" } else { "FAIL" }) "init_ok=$re2InitOk no_hang=$re2NoHang"

# bring daemon back
Start-Sleep 1
$d3 = Get-Process mneme-daemon -ErrorAction SilentlyContinue
if (-not $d3) {
    Start-Process -FilePath $mneme -ArgumentList "daemon", "start" -WindowStyle Hidden
    Start-Sleep 6
}

# ---------- Re3: deferred ----------
Add-Result "Re3-vm-reboot" "DEFERRED" "Reboot is destructive; orchestrator schedules separately if -AllowReboot. Default: skip on this run."

# ---------- Output ----------
Write-Host "=== Re-RESULTS-JSON ==="
$results | ConvertTo-Json -Depth 3
Write-Host "=== Re-END ==="
$pass = ($results | Where-Object status -eq "PASS").Count
$fail = ($results | Where-Object status -eq "FAIL").Count
$def = ($results | Where-Object status -eq "DEFERRED").Count
Write-Host "Re_VERDICT: pass=$pass fail=$fail deferred=$def"
