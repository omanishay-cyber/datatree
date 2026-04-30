# mneme :: install lifecycle

This document explains how mneme installs, where things land on disk, and
how to back out cleanly. The install scripts are deliberately small and
boring -- you should be able to read every one of them in a couple of
minutes.

---

## Quick start

### macOS / Linux

```sh
sh scripts/install-bundle.sh
```

### Windows (PowerShell 5.1+ or PowerShell 7+)

```powershell
pwsh .\scripts\install-bundle.ps1
```

That single command will:

1. Detect your OS and architecture.
2. Run a read-only health check (`check-runtime`).
3. If anything required is missing, ask before installing it via your
   platform's package manager.
4. Install the mneme supervisor.
5. Download/copy the required ONNX models (only `bge-small` is required;
   ~33 MB).
6. Start the mneme daemon.
7. Print the next step (install the OpenCode/Claude Code plugin).

If you want a dry run, just run `check-runtime` first -- it never modifies
anything:

```sh
sh   scripts/check-runtime.sh
pwsh .\scripts\check-runtime.ps1
```

---

## What gets installed where

| Component           | Location                                              | Owner                |
| ------------------- | ----------------------------------------------------- | -------------------- |
| Bun                 | `~/.bun/bin/bun` (Unix) or winget default (Windows)   | Bun's installer      |
| Python 3            | OS package manager default                            | OS package manager   |
| Tesseract           | OS package manager default                            | OS package manager   |
| ffmpeg              | OS package manager default                            | OS package manager   |
| SQLite              | bundled into `mneme-store` (rusqlite `bundled`)    | mneme binary      |
| Models (ONNX)       | `~/.mneme/llm/<model>/`                            | mneme             |
| Supervisor binary   | `~/.mneme/bin/mneme-supervisor`                 | mneme             |
| Logs                | `~/.mneme/logs/install.log`                        | mneme             |
| Install manifest    | `~/.mneme/install-manifest.json`                   | mneme             |

`MNEME_HOME` overrides the default `~/.mneme` location for everything
except deps installed by the OS package manager.

---

## Platform-specific notes

### Windows

- `winget` is built into Windows 11 and current Windows 10 builds. The
  installer falls back to `scoop` then `choco` if neither is present.
- Some installers (notably `Python.Python.3.12` via winget) update `PATH`
  but only for **new** shell sessions. The bundle script refreshes the
  current session's `PATH`, but if anything still doesn't resolve, just
  open a fresh terminal and re-run.
- Tesseract from `UB-Mannheim.TesseractOCR` is the maintained Windows
  build. The default install includes English language data.
- The bundle script does not require admin. winget itself may prompt for
  UAC for individual packages -- accept the elevation prompt.

### macOS

- The installer prefers Homebrew. If `brew` is missing and you pass
  `--auto-install`, it offers to run the official Homebrew installer
  first.
- On Apple Silicon, Homebrew lives at `/opt/homebrew`; on Intel Macs at
  `/usr/local`. The script adds the right one to `PATH` automatically.
- For Tesseract you may want extra languages: `brew install tesseract-lang`.

### Linux

- Distro-detection is done from `/etc/os-release`. Supported families:
  Debian/Ubuntu (`apt`), Fedora/RHEL (`dnf`), Arch (`pacman`), openSUSE
  (`zypper`).
- `apt`/`dnf`/`pacman` operations require `sudo`. The script invokes
  `sudo` only at those exact moments and never holds elevation.
- Bun has no official Debian/RPM package; the script uses the official
  `curl -fsSL https://bun.sh/install | bash` flow when you opt in with
  `--auto-install`. On Arch it uses the `bun-bin` AUR package if `yay` is
  available, otherwise falls back to the official installer.

---

## Offline / air-gapped install (`--from <dir>`)

mneme never reaches the internet on its own. With `--from <dir>` it
also avoids invoking the package manager for things you've pre-downloaded.

1. On a network-connected machine, mirror the artifacts you need into a
   single folder, e.g.

   ```
   /mnt/usb/mneme-mirror/
     bun                       # Bun binary or installer
     bun.tar.gz
     tesseract-installer.exe
     ffmpeg.zip
     bge-small.onnx            # already-extracted model file
   ```

2. Move that folder to the offline machine.

3. Run the bundle with `--from`:

   ```sh
   sh   scripts/install-bundle.sh   --from /mnt/usb/mneme-mirror
   pwsh .\scripts\install-bundle.ps1 -From C:\mirror\mneme
   ```

The runtime installer will look for `<dep>` / `<dep>.tar.gz` / `<dep>.exe`
inside the mirror dir before falling back to the package manager. If you
do not pass `--auto-install` alongside `--from`, anything still missing
after the mirror lookup will be reported as missing rather than
installed.

---

## Uninstall

To reverse everything mneme installed (and optionally leave shared
tools alone):

```sh
sh   scripts/uninstall-runtime.sh   --keep-shared
pwsh .\scripts\uninstall-runtime.ps1 -KeepShared
```

`--keep-shared` reads `~/.mneme/install-manifest.json` and skips
removing any tool listed under `preexisting`. Without that flag, every
runtime dep that was installed *by mneme* is removed via the package
manager. The manifest is updated as items are removed.

To wipe the mneme state directory entirely (models, logs, manifest):

```sh
rm -rf ~/.mneme                       # Unix
Remove-Item $env:USERPROFILEmneme -Recurse -Force   # Windows
```

To uninstall the supervisor and stop the daemon, use the existing
`stop-daemon` and `uninstall` scripts in this folder.

---

## Troubleshooting

| Symptom                                                     | Likely cause                                           | Fix                                                                                      |
| ----------------------------------------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| `bun: command not found` immediately after install          | Bun installed to `~/.bun/bin` but `PATH` not refreshed | Open a new shell, or `export PATH="$HOME/.bun/bin:$PATH"`                                |
| `winget: term not recognised`                               | Old Windows 10 build without App Installer             | Install [App Installer](https://apps.microsoft.com/store/detail/9NBLGGH4NNS1) from Store |
| Tesseract install succeeds but `tesseract --version` fails  | UB-Mannheim install dir missing from `PATH`            | Add `C:\Program Files\Tesseract-OCR` to `PATH`, or open a new shell                      |
| `apt-get: command not found`                                | You're on a non-Debian distro                          | The script auto-detects -- if it failed to, file an issue with `/etc/os-release` content |
| Whisper inference is slow                                   | ffmpeg missing or wrong CPU codec path                 | Re-run `check-runtime` -- if ffmpeg shows OK, the issue is model size; try `tiny`        |
| Model file present but `check-runtime` shows missing        | Model in wrong dir                                     | Models must live in `~/.mneme/llm/<name>/model.onnx`                                  |
| `sudo: a password is required` mid-install                  | Non-interactive shell                                  | Re-run from an interactive terminal, or pre-cache sudo with `sudo -v` first              |
| Install hangs on macOS at "fetching formula"                | Homebrew first-run network init                        | Cancel, run `brew update` once manually, then re-run                                     |
| `install-bundle` exits at step 4                            | Supervisor build failed (Rust toolchain)               | Install the Rust toolchain (`rustup`), then re-run                                       |
| Two Pythons on PATH (system + Homebrew)                     | macOS default Python 3.9 collides with brew 3.12       | The script accepts either; explicit `python3.12` from brew is preferred                  |

If something fails, the full transcript is in
`~/.mneme/logs/install.log`. That log is the first thing to check.

---

## Idempotency guarantee

Every script in this folder is safe to re-run. If a dependency is already
present at the right version, it is skipped. The install manifest is
overwritten with the *current* state on every run, so a re-run after
manual changes will reflect reality.
