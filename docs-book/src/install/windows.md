# Install — Windows

Tested on Windows 10 22H2, Windows 11 23H2 / 24H2, and the WinDev2407 evaluation VM. Both x64 and ARM64 supported.

## winget (recommended)

```powershell
winget install Anish.Mneme
```

Or, for the brand alias:

```powershell
winget install Anish.Mnemeos
```

`winget upgrade Anish.Mneme` keeps it current. The manifest is in [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).

## PowerShell one-liner (no winget)

```powershell
iwr -useb https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/bootstrap-install.ps1 | iex
```

The bootstrap script:

1. Detects `x64` or `arm64`
2. Downloads `mneme-windows-<arch>.zip` from the latest GitHub release
3. Verifies SHA-256 against the release checksums file
4. Extracts to `%USERPROFILE%\.mneme\`
5. Adds `%USERPROFILE%\.mneme\bin` to your user PATH (persistent)
6. Registers `mneme-daemon` as a Scheduled Task (auto-start on logon)

## pip (any OS with Python)

```powershell
pip install mnemeos
```

The `mnemeos` package is a thin Python wrapper around the bootstrap installer — calling `mneme` after `pip install` runs the real Rust binary the wrapper download-and-extracted on first invocation.

## Manual install

```powershell
$Arch = "x64"                 # or "arm64"
$Archive = "mneme-windows-$Arch.zip"

# Download
Invoke-WebRequest -Uri "https://github.com/omanishay-cyber/mneme/releases/latest/download/$Archive" -OutFile $Archive

# Verify SHA-256
Invoke-WebRequest -Uri "https://github.com/omanishay-cyber/mneme/releases/latest/download/release-checksums.json" -OutFile release-checksums.json
$expected = (Get-Content release-checksums.json | ConvertFrom-Json).$Archive.sha256
$actual = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLower()
if ($expected -ne $actual) { throw "checksum mismatch" }

# Extract
$Dest = "$env:USERPROFILE\.mneme"
New-Item -ItemType Directory -Path $Dest -Force | Out-Null
Expand-Archive -Path $Archive -DestinationPath $Dest -Force

# PATH
$bin = "$Dest\bin"
$path = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $path.Contains($bin)) {
    [Environment]::SetEnvironmentVariable("Path", "$bin;$path", "User")
}

# Verify (in a NEW shell so PATH is loaded)
mneme --version
```

## Why two binaries (`mneme.exe` + `mneme-hook.exe`)

Windows shows a console window for every command-line process unless the binary is built for the GUI subsystem. Claude Code fires `mneme` as a hook on every `UserPromptSubmit` and `PreToolUse` — that's potentially dozens of console flashes per minute.

Genesis ships `mneme-hook.exe`, a separate Windows GUI-subsystem binary that handles the 3 hook subcommands (`userprompt-submit`, `pretool-edit-write`, `pretool-grep-read`) without flashing. The platform integration writes hook entries pointing at `mneme-hook.exe`; everything else (`mneme build`, `mneme recall`, etc.) still uses `mneme.exe` so terminal output works normally.

## Scheduled Task

The installer registers a `MnemeDaemon` Scheduled Task that runs the daemon at logon:

```powershell
schtasks /Query /TN MnemeDaemon              # is it registered?
schtasks /End /TN MnemeDaemon                # stop
schtasks /Run /TN MnemeDaemon                # start
schtasks /Delete /TN MnemeDaemon /F          # unregister
```

Or use the CLI:

```powershell
mneme daemon status
mneme daemon start
mneme daemon stop
```

## Hook integration

```powershell
mneme install --platform=claude-code
```

Writes:

- `%APPDATA%\Claude\claude_code_config.json` MCP entry
- `%APPDATA%\Claude\settings.json` hook entries (Layers 1/2/3)

Restart Claude Code; `mneme: connected` should appear with 50 tools.

## Uninstall

```powershell
mneme uninstall                   # delete %USERPROFILE%\.mneme
schtasks /Delete /TN MnemeDaemon /F  # remove auto-start
```

`winget uninstall Anish.Mneme` does both in one shot.

## Troubleshooting

### "mneme is not recognized"

PATH wasn't refreshed after install. Open a new PowerShell window or run:

```powershell
$env:Path = "$env:USERPROFILE\.mneme\bin;" + $env:Path
```

### "An impostor mneme.exe is on PATH"

The install script warns when it detects a non-mneme `mneme.exe` (typically a small PyPI launcher stub from an unrelated package). Remove it:

```powershell
Get-Command mneme | Format-List Source     # find the path
Remove-Item <path>                          # delete
```

### Defender takes 5 seconds to scan the new binary

Expected on first run after a `mneme self-update`. The post-swap health check waits 5 s for `--version` to exit; on a fresh 56 MB unsigned binary, Defender's real-time scan can run right up to that boundary. If you hit a spurious rollback, retry — the second attempt is fast because Defender has cached the scan.
