# Install — Linux

Tested on Ubuntu 22.04 / 24.04, Debian 12, Fedora 40, Arch (rolling). Should work on any distro with glibc 2.31+.

## One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/release/install-linux.sh | bash
```

The script:

1. Detects your CPU arch (`x86_64` or `aarch64`)
2. Downloads `mneme-v0.4.0-linux-<arch>.tar.gz` from the GitHub release
3. Verifies SHA-256 against the release checksums file
4. Extracts to `~/.mneme/`
5. Symlinks `~/.mneme/bin/mneme` and `~/.mneme/bin/mnemeos` into a directory on PATH (auto-detects `~/.local/bin`, `/usr/local/bin`, etc.)
6. Starts the daemon as a systemd user unit (or via `nohup` if systemd is unavailable)

## Manual install

```bash
# 1. Download
ARCH=$(uname -m)            # x86_64 or aarch64
VERSION=v0.4.0
ARCHIVE=mneme-${VERSION}-linux-${ARCH}.tar.gz
curl -fsSLO https://github.com/omanishay-cyber/mneme/releases/download/${VERSION}/${ARCHIVE}

# 2. Verify
curl -fsSLO https://github.com/omanishay-cyber/mneme/releases/download/${VERSION}/release-checksums.json
sha256sum -c <(jq -r ".\"${ARCHIVE}\".sha256 + \"  ${ARCHIVE}\"" release-checksums.json)

# 3. Extract
mkdir -p ~/.mneme
tar -xzf ${ARCHIVE} -C ~/.mneme/

# 4. Symlink
ln -sf ~/.mneme/bin/mneme ~/.local/bin/mneme

# 5. Verify
mneme --version
```

## Models (optional)

Mneme works without local models — it falls back to a hashing-trick embedder that's good for triage but not for semantic recall. For real BGE-quality results:

```bash
mneme models install bge-small-en-v1.5
```

This downloads a 130 MB ONNX model + tokenizer to `~/.mneme/llm/`. The next `mneme build` automatically uses it.

For local LLM (`mneme why`, `concept extraction with summaries`):

```bash
mneme models install phi-3-mini-4k-q4
```

This is a 2 GB GGUF — it takes a few minutes on a typical home connection.

## Daemon control

```bash
mneme daemon status        # is it running?
mneme daemon start         # start manually
mneme daemon stop          # stop
mneme daemon logs          # tail the daemon log
```

The daemon also runs as a systemd user service. On most distros:

```bash
systemctl --user status mneme
systemctl --user enable --now mneme   # auto-start on login
```

## Distro-specific notes

### Ubuntu / Debian

The install script uses `curl`, `tar`, `jq`, `sha256sum` — all in `coreutils` + `curl` + `jq`. If any are missing:

```bash
sudo apt install -y curl jq
```

### Fedora

Same set, available via dnf:

```bash
sudo dnf install -y curl jq
```

### Arch

```bash
sudo pacman -S --needed curl jq
```

### Alpine / musl

`mneme` ships glibc binaries. On Alpine you need glibc-compat:

```bash
apk add --no-cache gcompat
```

Or build from source (see [Contributing](../contributing.md)).

## Hook integration

```bash
mneme install --platform=claude-code
mneme install --platform=cursor
mneme install --platform=codex
```

Each call is idempotent — re-running just refreshes the hook entries without duplicating them.

## Uninstall

```bash
mneme uninstall                # delete ~/.mneme/
systemctl --user disable --now mneme  # stop systemd service if registered
rm ~/.local/bin/mneme          # remove symlink
```
