param(
    [switch]$SkipReinstall = $false
)
$ErrorActionPreference = "Continue"

# Phase Un -- Uninstall + cleanup
# Acceptance:
#   ~/.claude.json no longer contains an mneme entry
#   ~/.mneme/ either fully gone OR opt-in retention only
#   PATH no longer contains ~/.mneme/bin
#   Defender exclusions removed (or document if intentionally retained)
#   No orphan files in temp / appdata
#   Reinstall after uninstall works clean

$mneme = "C:\Users\Administrator\.mneme\bin\mneme.exe"
$results = @()

function Add-Result {
    param($Name, $Status, $Detail)
    $script:results += [pscustomobject]@{ name = $Name; status = $Status; detail = $Detail }
}

# ---------- Pre-state snapshot ----------
$preState = @{
    mneme_dir_exists       = Test-Path "$env:USERPROFILE\.mneme"
    claude_dir_exists      = Test-Path "$env:USERPROFILE\.claude"
    claude_json_exists     = Test-Path "$env:USERPROFILE\.claude.json"
    mneme_in_path          = ([Environment]::GetEnvironmentVariable("Path", "User") -split ";") | Where-Object { $_ -like "*\.mneme\*" }
    daemon_running         = (Get-Process mneme-daemon -ErrorAction SilentlyContinue) -ne $null
    workers_running        = (Get-Process mneme-* -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -ne 'mneme-daemon' }).Count
    defender_exclusions    = (Get-MpPreference).ExclusionPath | Where-Object { $_ -like "*\.mneme*" -or $_ -like "*\.claude*" }
    receipts_dir_exists    = Test-Path "$env:USERPROFILE\.mneme\install-receipts"
    receipts_count         = if (Test-Path "$env:USERPROFILE\.mneme\install-receipts") { (Get-ChildItem "$env:USERPROFILE\.mneme\install-receipts" -File -ErrorAction SilentlyContinue).Count } else { 0 }
}
Write-Host "=== Un-PRE-STATE ==="
$preState | ConvertTo-Json -Depth 3 | Write-Host

# ---------- Un.1: dry-run uninstall ----------
Write-Host "=== Un.1 dry-run uninstall ==="
$dryOut = & $mneme uninstall --platform claude-code --dry-run 2>&1 | Out-String
$dryCode = $LASTEXITCODE
Write-Host $dryOut
Add-Result "Un.1-dry-run" $(if ($dryCode -eq 0) { "PASS" } else { "FAIL" }) "exit=$dryCode"

# ---------- Un.2: real uninstall ----------
Write-Host "=== Un.2 real uninstall (claude-code) ==="
$realOut = & $mneme uninstall --platform claude-code 2>&1 | Out-String
$realCode = $LASTEXITCODE
Write-Host $realOut
Add-Result "Un.2-real-uninstall" $(if ($realCode -eq 0) { "PASS" } else { "FAIL" }) "exit=$realCode"

# ---------- Un.3: verify .claude.json no longer mentions mneme (in mcpServers) ----------
$claudeJson = "$env:USERPROFILE\.claude.json"
if (Test-Path $claudeJson) {
    $cj = Get-Content $claudeJson -Raw -ErrorAction SilentlyContinue
    if ($cj -match '"mneme"') {
        Add-Result "Un.3-claude-json-clean" "FAIL" "string 'mneme' still appears in .claude.json"
    } else {
        Add-Result "Un.3-claude-json-clean" "PASS" "no 'mneme' string in .claude.json"
    }
} else {
    Add-Result "Un.3-claude-json-clean" "PASS" ".claude.json absent post-uninstall"
}

# ---------- Un.4: verify ~/.mneme retention (per receipt contract) ----------
# Per design, uninstall doesn't necessarily wipe ~/.mneme -- opt-in retention is the contract
$mnemeDirPostUninstall = Test-Path "$env:USERPROFILE\.mneme"
$binPostUninstall = Test-Path "$env:USERPROFILE\.mneme\bin\mneme.exe"
$shardsPostUninstall = Test-Path "$env:USERPROFILE\.mneme\shards"
Add-Result "Un.4-mneme-dir-retention" "INFO" "mneme_dir=$mnemeDirPostUninstall bin=$binPostUninstall shards=$shardsPostUninstall (retention is design choice -- record actual)"

# ---------- Un.5: verify PATH no longer has ~/.mneme/bin ----------
$pathAfter = ([Environment]::GetEnvironmentVariable("Path", "User") -split ";") | Where-Object { $_ -like "*\.mneme\*" -or $_ -like "*\.mneme" }
if ($pathAfter -and $pathAfter.Count -gt 0) {
    Add-Result "Un.5-path-cleaned" "FAIL" "PATH still has: $($pathAfter -join '; ')"
} else {
    Add-Result "Un.5-path-cleaned" "PASS" "PATH no longer references .mneme"
}

# ---------- Un.6: Defender exclusions ----------
$exAfter = (Get-MpPreference).ExclusionPath | Where-Object { $_ -like "*\.mneme*" -or $_ -like "*\.claude*" }
if ($exAfter -and $exAfter.Count -gt 0) {
    Add-Result "Un.6-defender-exclusions" "INFO" "still present: $($exAfter -join '; ') -- user must manually remove or document as intentional"
} else {
    Add-Result "Un.6-defender-exclusions" "PASS" "no .mneme/.claude exclusions remain"
}

# ---------- Un.7: orphan procs ----------
$leftover = Get-Process mneme-* -ErrorAction SilentlyContinue
if ($leftover) {
    Add-Result "Un.7-no-orphan-procs" "FAIL" "still alive: $($leftover.ProcessName -join ',')"
} else {
    Add-Result "Un.7-no-orphan-procs" "PASS" "all mneme procs gone"
}

# ---------- Un.8: orphan files in temp / appdata ----------
$tempOrphans = @()
foreach ($p in @("$env:TEMP", "$env:LOCALAPPDATA", "$env:APPDATA")) {
    if (Test-Path $p) {
        $tempOrphans += Get-ChildItem $p -Recurse -ErrorAction SilentlyContinue -Filter "*mneme*" -File 2>$null | Select-Object -First 5
    }
}
if ($tempOrphans.Count -gt 0) {
    Add-Result "Un.8-temp-appdata" "INFO" "found $($tempOrphans.Count) mneme-named files: $(($tempOrphans | ForEach-Object FullName) -join '; ')"
} else {
    Add-Result "Un.8-temp-appdata" "PASS" "no mneme orphans in temp/appdata"
}

# ---------- Un.9: nuclear path -- Remove-Item -Recurse -Force ~/.mneme ----------
Write-Host "=== Un.9 nuclear cleanup attempt ==="
$nuke = $true
try {
    if (Test-Path "$env:USERPROFILE\.mneme") {
        # First stop daemon if any zombies
        Get-Process mneme-* -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep 1
        Remove-Item -Recurse -Force "$env:USERPROFILE\.mneme" -ErrorAction Stop
    }
    Add-Result "Un.9-nuclear-rm" "PASS" "wiped ~/.mneme cleanly, no stuck procs"
} catch {
    $nuke = $false
    Add-Result "Un.9-nuclear-rm" "FAIL" "$_"
}

# ---------- Un.10: reinstall after uninstall ----------
if (-not $SkipReinstall) {
    Write-Host "=== Un.10 reinstall after uninstall ==="
    # We need the install zip available on the VM. Look for it.
    $zip = Get-ChildItem "C:\Users\Administrator\" -Filter "mneme-v*.zip" -File -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $zip) {
        Add-Result "Un.10-reinstall" "SKIP" "no install zip found at C:\Users\Administrator\mneme-v*.zip"
    } else {
        $stage = "C:\Users\Administrator\un10-reinstall"
        if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
        New-Item -ItemType Directory -Path $stage -Force | Out-Null
        Expand-Archive -Path $zip.FullName -DestinationPath $stage -Force
        $installPs1 = Get-ChildItem $stage -Filter "install.ps1" -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $installPs1) {
            Add-Result "Un.10-reinstall" "FAIL" "no install.ps1 in extracted zip"
        } else {
            $installOut = & powershell -NoProfile -ExecutionPolicy Bypass -File $installPs1.FullName 2>&1 | Out-String
            $installCode = $LASTEXITCODE
            $newBin = Test-Path "$env:USERPROFILE\.mneme\bin\mneme.exe"
            Add-Result "Un.10-reinstall" $(if ($installCode -eq 0 -and $newBin) { "PASS" } else { "FAIL" }) "exit=$installCode bin_exists=$newBin"
        }
    }
} else {
    Add-Result "Un.10-reinstall" "SKIP" "explicitly skipped by -SkipReinstall flag"
}

# ---------- Output ----------
Write-Host "=== Un-RESULTS-JSON ==="
$results | ConvertTo-Json -Depth 3
Write-Host "=== Un-END ==="
$pass = ($results | Where-Object status -eq "PASS").Count
$fail = ($results | Where-Object status -eq "FAIL").Count
Write-Host "Un_VERDICT: pass=$pass fail=$fail total=$($results.Count)"
