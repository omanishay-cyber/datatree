param()
$ErrorActionPreference = "Continue"

# Phase D -- Privacy / data-leak audit
# Acceptance: 100% local invariant holds -- no outbound TCP/UDP to non-localhost addresses
# during normal operation (build/recall/mcp/daemon up).

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
$results = @()
function Add-Result {
    param($Name, $Status, $Detail)
    $script:results += [pscustomobject]@{ name = $Name; status = $Status; detail = $Detail }
}

# ---------- D1: outbound TCP capture via netstat ----------
Write-Host "=== D1 outbound TCP audit ==="
# Take a snapshot of mneme TCP connections during normal operation
# Iterate: recall, build, mcp stdio
$mnemePids = (Get-Process mneme-* -ErrorAction SilentlyContinue).Id
$snapshots = @()

function Snap-Connections {
    param($mark)
    $netstatRaw = netstat -ano | Out-String
    $lines = $netstatRaw -split "`r?`n"
    $tcpRows = $lines | Where-Object { $_ -match '^\s*(TCP|UDP)\s+(\S+)\s+(\S+)\s+(\S+)?\s*(\d+)?$' }
    $tcpParsed = $tcpRows | ForEach-Object {
        if ($_ -match '^\s*(TCP|UDP)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\d+)$') {
            [pscustomobject]@{ proto = $matches[1]; local = $matches[2]; remote = $matches[3]; state = $matches[4]; pid = [int]$matches[5] }
        } elseif ($_ -match '^\s*(TCP|UDP)\s+(\S+)\s+(\S+)\s+(\d+)$') {
            [pscustomobject]@{ proto = $matches[1]; local = $matches[2]; remote = $matches[3]; state = ""; pid = [int]$matches[4] }
        }
    } | Where-Object { $_ -ne $null }
    $mnemeOnly = $tcpParsed | Where-Object { $script:mnemePids -contains $_.pid }
    [pscustomobject]@{ mark = $mark; total = $tcpParsed.Count; mneme = $mnemeOnly }
}

# Snap idle baseline
$snapBase = Snap-Connections "idle-baseline"
$snapshots += $snapBase

# Trigger recall
$recOut = & $mneme recall test 2>&1 | Out-String
$snapAfterRecall = Snap-Connections "after-recall"
$snapshots += $snapAfterRecall

# Trigger build small
$buildCorpus = "C:\Users\Administrator\d-build-corpus"
if (-not (Test-Path $buildCorpus)) {
    New-Item -ItemType Directory -Path $buildCorpus -Force | Out-Null
    1..20 | ForEach-Object {
        Set-Content -Path (Join-Path $buildCorpus "f$_.rs") -Value "pub fn f$_() -> u32 { $_ }`n" -Encoding UTF8
    }
}
$buildJob = Start-Job -ScriptBlock {
    param($mn, $cp)
    & $mn build --yes $cp 2>&1
} -ArgumentList $mneme, $buildCorpus
Start-Sleep 1
$snapDuringBuild = Snap-Connections "during-build"
$snapshots += $snapDuringBuild
$buildJob | Wait-Job -Timeout 60 | Out-Null
Remove-Job $buildJob -Force -ErrorAction SilentlyContinue

# Trigger mcp stdio
$mcpTester = @'
import { spawn } from "node:child_process";
const p = spawn("mneme", ["mcp", "stdio"], { cwd: process.env.USERPROFILE + "\\.mneme", env: { ...process.env, MNEME_LOG: "error", MNEME_MCP_PATH: process.env.USERPROFILE + "\\.mneme\\mcp\\src\\index.ts", MNEME_IPC_TIMEOUT_MS: "2000" }, stdio: ["pipe", "pipe", "pipe"] });
let buf = ""; const pending = new Map(); let nextId = 1;
p.stdout.on("data", (d) => { buf += d.toString(); let nl; while ((nl = buf.indexOf("\n")) >= 0) { const line = buf.slice(0, nl); buf = buf.slice(nl + 1); if (!line.trim()) continue; try { const m = JSON.parse(line); if (m.id != null && pending.has(m.id)) { pending.get(m.id).resolve(m); pending.delete(m.id); } } catch {} } });
function rpc(method, params={}, t=8000) { const id = nextId++; return new Promise((res, rej) => { pending.set(id, {resolve:res,reject:rej}); p.stdin.write(JSON.stringify({jsonrpc:"2.0",id,method,params})+"\n"); setTimeout(() => { if (pending.has(id)) { pending.delete(id); rej(new Error("timeout")); } }, t); }); }
await rpc("initialize", {protocolVersion:"2024-11-05",capabilities:{},clientInfo:{name:"d",version:"1"}});
await rpc("tools/list");
await rpc("tools/call", {name:"recall_decision", arguments:{query:"test"}});
console.log("MCP_DONE");
process.exit(0);
'@
$mcpDriverPath = "C:\Users\Administrator\d-mcp-driver.mjs"
Set-Content -Path $mcpDriverPath -Value $mcpTester -Encoding UTF8
$mcpJob = Start-Job -ScriptBlock { param($drv) node $drv } -ArgumentList $mcpDriverPath
Start-Sleep 1
$snapDuringMcp = Snap-Connections "during-mcp"
$snapshots += $snapDuringMcp
$mcpJob | Wait-Job -Timeout 30 | Out-Null
Remove-Job $mcpJob -Force -ErrorAction SilentlyContinue

# Now analyze: any non-localhost remote address from any mneme PID?
$nonLocalPattern = '^(127\.0\.0\.1|0\.0\.0\.0|::1|\[::1\]|\[::\])'
$violations = @()
foreach ($snap in $snapshots) {
    foreach ($conn in $snap.mneme) {
        $remote = $conn.remote
        # extract just the IP/host
        $remoteIp = if ($remote -match '^\[([^\]]+)\]:\d+$') { $matches[1] } elseif ($remote -match '^([^:]+):\d+$') { $matches[1] } else { $remote }
        $isLocal = ($remoteIp -match '^(127\.0\.0\.1|0\.0\.0\.0|::1|::|0\.0\.0\.0:0)$') -or ($remote -eq "*:*")
        # also accept loopback variations
        if (-not $isLocal -and $remote -ne "*:*" -and $remote -notmatch "^0\.0\.0\.0") {
            $violations += [pscustomobject]@{ snap = $snap.mark; pid = $conn.pid; proto = $conn.proto; remote = $remote; state = $conn.state }
        }
    }
}

if ($violations.Count -eq 0) {
    Add-Result "D1-no-outbound-tcp" "PASS" "0 non-localhost connections from any mneme PID across $(($snapshots).Count) snapshots"
} else {
    Add-Result "D1-no-outbound-tcp" "FAIL" "$($violations.Count) violations: $($violations | ConvertTo-Json -Compress -Depth 3)"
}

# ---------- D2: PII / paths in logs ----------
Write-Host "=== D2 daemon log scrub ==="
$logsOut = & $mneme daemon logs 2>&1 | Out-String
$logSize = $logsOut.Length
# Scan for absolute paths that include sensitive markers
$pathHits = @()
$pathPatterns = @(
    'C:\\Users\\[A-Za-z0-9._-]+\\Documents',
    'C:\\Users\\[A-Za-z0-9._-]+\\Desktop',
    'C:\\Users\\[A-Za-z0-9._-]+\\AppData',
    'C:\\Users\\[A-Za-z0-9._-]+\\Pictures',
    'C:\\Users\\[A-Za-z0-9._-]+\\.ssh',
    '[A-Z]+_TOKEN\s*=',
    'PASSWORD\s*=',
    'API_KEY\s*=',
    'secret_key',
    'AKIA[0-9A-Z]{16}'
)
foreach ($pat in $pathPatterns) {
    $m = [regex]::Matches($logsOut, $pat)
    if ($m.Count -gt 0) {
        $pathHits += [pscustomobject]@{ pattern = $pat; count = $m.Count; sample = $m[0].Value }
    }
}
if ($pathHits.Count -eq 0) {
    Add-Result "D2-logs-pii-clean" "PASS" "no token/secret/personal-path patterns in $logSize chars of logs"
} else {
    Add-Result "D2-logs-pii-clean" "FAIL" "found patterns: $($pathHits | ConvertTo-Json -Compress -Depth 3)"
}

# ---------- D3: outbound DNS ----------
Write-Host "=== D3 DNS audit ==="
# Use netstat udp 53 + Resolve-DnsName? Simpler: check for any UDP connection to port 53 from mneme PIDs
$udpConns = @()
foreach ($snap in $snapshots) {
    foreach ($c in $snap.mneme) {
        if ($c.proto -eq "UDP" -and $c.remote -match ":53$") {
            $udpConns += $c
        }
    }
}
if ($udpConns.Count -eq 0) {
    Add-Result "D3-no-outbound-dns" "PASS" "0 DNS queries from mneme PIDs in any snapshot"
} else {
    Add-Result "D3-no-outbound-dns" "FAIL" "$($udpConns.Count) UDP/53 connections: $($udpConns | ConvertTo-Json -Compress)"
}

# ---------- D4: telemetry endpoints / beacon scan ----------
Write-Host "=== D4 telemetry static scan ==="
# Search the installed mcp + bin for telemetry-shaped strings
$telemetryHits = @()
$searchPaths = @(
    "C:\Users\Administrator\.mneme\bin",
    "C:\Users\Administrator\.mneme\mcp\src",
    "C:\Users\Administrator\.mneme\plugin"
)
$telemetryPatterns = @(
    'analytics\.', 'sentry\.io', 'datadoghq', 'telemetry\.', 'segment\.io',
    'mixpanel\.', 'amplitude\.', 'beacon', 'tracking-pixel', 'umami',
    'plausible', 'posthog'
)
foreach ($sp in $searchPaths) {
    if (-not (Test-Path $sp)) { continue }
    foreach ($pat in $telemetryPatterns) {
        $hits = Get-ChildItem $sp -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Length -lt 5MB } | Select-String -Pattern $pat -ErrorAction SilentlyContinue | Select-Object -First 3
        foreach ($h in $hits) {
            $telemetryHits += [pscustomobject]@{ pattern = $pat; file = $h.Path; line = $h.LineNumber; text = $h.Line.Substring(0, [Math]::Min(100, $h.Line.Length)) }
        }
    }
}
if ($telemetryHits.Count -eq 0) {
    Add-Result "D4-no-telemetry" "PASS" "no telemetry-shaped strings in installed assets"
} else {
    Add-Result "D4-no-telemetry" "INFO" "found candidates (manual review): $($telemetryHits | ConvertTo-Json -Compress -Depth 3)"
}

# ---------- D5: receipt sensitivity ----------
Write-Host "=== D5 receipt sensitivity scan ==="
$receiptDir = "C:\Users\Administrator\.mneme\install-receipts"
$senstHits = @()
if (Test-Path $receiptDir) {
    $receipts = Get-ChildItem $receiptDir -File -Filter "*.json" -ErrorAction SilentlyContinue
    $senstPatterns = @('password', 'secret', 'token', 'api[_-]?key', 'AKIA[0-9A-Z]{16}', 'private[_-]?key')
    foreach ($r in $receipts) {
        $body = Get-Content $r.FullName -Raw -ErrorAction SilentlyContinue
        foreach ($pat in $senstPatterns) {
            if ($body -match $pat) {
                $senstHits += [pscustomobject]@{ file = $r.Name; pattern = $pat }
            }
        }
    }
}
if ($senstHits.Count -eq 0) {
    Add-Result "D5-receipts-clean" "PASS" "no sensitive patterns in install-receipts"
} else {
    Add-Result "D5-receipts-clean" "FAIL" "patterns found: $($senstHits | ConvertTo-Json -Compress)"
}

# ---------- Output ----------
Write-Host "=== D-RESULTS-JSON ==="
$results | ConvertTo-Json -Depth 3
Write-Host "=== D-END ==="
$pass = ($results | Where-Object status -eq "PASS").Count
$fail = ($results | Where-Object status -eq "FAIL").Count
$info = ($results | Where-Object status -eq "INFO").Count
Write-Host "D_VERDICT: pass=$pass fail=$fail info=$info"
