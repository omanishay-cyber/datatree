# Install — macOS

Tested on macOS 13 (Ventura), 14 (Sonoma), 15 (Sequoia). Apple Silicon native; Intel Macs run the arm64 binary via Rosetta 2 (a one-time prompt installs Rosetta if it isn't present).

## One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-mac.sh | bash
```

Same flow as Linux. The mac-specific bits the installer handles:

- `xattr -cr ~/.mneme/bin/*` to clear the macOS quarantine attribute (otherwise Gatekeeper blocks the binary on first run)
- Detects Apple Silicon vs Intel and installs the matching tarball
- Uses `launchd` instead of systemd for the auto-start service

## Manual install

```bash
ARCH=arm64                       # or x86_64 on Intel Macs (uses Rosetta)
VERSION=v0.4.0
ARCHIVE=mneme-${VERSION}-darwin-${ARCH}.tar.gz
curl -fsSLO https://github.com/omanishay-cyber/mneme/releases/download/${VERSION}/${ARCHIVE}

# Verify
curl -fsSLO https://github.com/omanishay-cyber/mneme/releases/download/${VERSION}/release-checksums.json
shasum -a 256 -c <(jq -r ".\"${ARCHIVE}\".sha256 + \"  ${ARCHIVE}\"" release-checksums.json)

# Extract
mkdir -p ~/.mneme
tar -xzf ${ARCHIVE} -C ~/.mneme/

# Clear quarantine (REQUIRED — Gatekeeper would block otherwise)
xattr -cr ~/.mneme/bin/

# Symlink
ln -sf ~/.mneme/bin/mneme /usr/local/bin/mneme

# Verify
mneme --version
```

## Apple Silicon vs Intel

| Mac | Binary | Performance |
|----|----|----|
| M1 / M2 / M3 / M4 | `darwin-arm64` (native) | Native speed |
| Intel | `darwin-arm64` via Rosetta 2 | ~5-10% overhead |

A separate `darwin-x86_64` build was deferred — the v0.4.0 multi-arch matrix builds arm64 only. The install script transparently uses Rosetta for Intel users; the first `mneme` invocation prompts to install Rosetta if missing. After that initial prompt, `mneme` runs identically on both architectures.

## Models

Same as Linux. The bundled installer ships without models by default to keep the archive under 60 MB; install on demand:

```bash
mneme models install bge-small-en-v1.5    # 130 MB embedding model
mneme models install phi-3-mini-4k-q4     # 2 GB optional LLM
```

## launchd auto-start

The install script writes `~/Library/LaunchAgents/com.mneme.daemon.plist` and runs:

```bash
launchctl load -w ~/Library/LaunchAgents/com.mneme.daemon.plist
```

To check / control:

```bash
launchctl list | grep mneme              # is it loaded?
launchctl unload ~/Library/LaunchAgents/com.mneme.daemon.plist  # stop
launchctl load -w ~/Library/LaunchAgents/com.mneme.daemon.plist # restart
```

Or just use the CLI:

```bash
mneme daemon status
mneme daemon start
mneme daemon stop
```

## Hook integration

```bash
mneme install --platform=claude-code
```

Writes:

- `~/Library/Application Support/Claude/claude_code_config.json` MCP entry
- `~/Library/Application Support/Claude/settings.json` hook entries (3 — Layer 1/2/3)

Restart Claude Code; the panel should show `mneme: connected` with 50 tools.

## Uninstall

```bash
mneme uninstall
launchctl unload ~/Library/LaunchAgents/com.mneme.daemon.plist
rm ~/Library/LaunchAgents/com.mneme.daemon.plist
rm /usr/local/bin/mneme
```

## Troubleshooting

### "mneme is damaged and can't be opened"

Gatekeeper. The install script normally clears the quarantine attribute, but if you copied the archive manually:

```bash
xattr -cr ~/.mneme/bin/
```

### Rosetta prompt doesn't appear

Force-install Rosetta:

```bash
softwareupdate --install-rosetta --agree-to-license
```

### `mneme: command not found` after install

Symlink target wasn't on PATH. Add `~/.mneme/bin` to your PATH:

```bash
echo 'export PATH="$HOME/.mneme/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```
