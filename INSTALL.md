# Installing mneme

> **TL;DR for v0.1.0:** from source only. Prebuilt binaries ship in v0.1.1.

mneme is a multi-process daemon (Rust supervisor + Bun MCP server). Multimodal extraction is pure Rust — no Python sidecar as of v0.2. The install process has three paths, sized for three kinds of users.

## Prerequisites

| Platform | You need |
|---|---|
| **Windows 10 / 11** | Rust 1.78+, Bun 1.3+, git, ~2 GB free disk |
| **macOS 12+** | Rust 1.78+, Bun 1.3+, Xcode CLT, ~2 GB free disk |
| **Linux (Ubuntu / Fedora / Arch)** | Rust 1.78+, Bun 1.3+, `build-essential` (or distro equivalent) |

Optional runtime deps — install if you want the matching features:

| Feature | Requires |
|---|---|
| Multimodal ingest (PDF / image / audio / video) | Tesseract 5+, ffmpeg |
| Local LLM (Phi-3 concept extraction) | 4 GB RAM free at runtime |
| Voice navigation | whisper-cpp binary (v0.2+) |

The `mneme install-runtime` command (see below) checks for these and gives you the exact platform-specific command to install any missing ones.

---

## Path 1 — From source (current only path)

```bash
# Clone
git clone https://github.com/omanishay-cyber/mneme
cd mneme

# Build the Rust workspace (produces 9 binaries, ~1 min on warm cache)
cargo build --release --workspace

# Install MCP server dependencies (200 packages)
cd mcp && bun install && cd ..

# Install vision app dependencies (~450 packages)
cd vision && bun install && cd ..

# v0.2: multimodal extraction is now pure Rust (crate `mneme-multimodal`),
# built as part of the cargo workspace above. No Python sidecar install.

# Copy binaries to the supervised bin dir
# POSIX:
mkdir -p ~/.mneme/bin
cp target/release/mneme* target/release/parse-worker* target/release/brain.exe 2>/dev/null ~/.mneme/bin/

# Windows PowerShell equivalent:
# New-Item -ItemType Directory -Force -Path "$env:USERPROFILEmneme\bin"
# Copy-Item target\release\*.exe "$env:USERPROFILEmneme\bin\"

# Install into every AI tool on your machine
./target/release/mneme install
```

Then start the daemon:

```bash
./target/release/mneme-supervisor start
```

## Path 2 — Marketplace plugin install (requires v0.1.1 + binary release)

```bash
# In any Claude Code project:
/plugin marketplace add github:omanishay-cyber/mneme
/plugin install mneme
```

> **v0.1.0 status:** the marketplace manifest is shipped, but the post-install step that downloads prebuilt binaries isn't wired yet. Use Path 1 for v0.1.0. v0.1.1 closes this gap.

## Path 3 — Bundle installer script (POSIX + Windows, alpha)

```bash
# macOS / Linux:
curl -fsSL https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install-bundle.sh | bash

# Windows PowerShell:
iwr https://raw.githubusercontent.com/omanishay-cyber/mneme/main/scripts/install-bundle.ps1 | iex
```

The bundle installer:
1. Checks / installs Rust, Bun, Python, Tesseract, ffmpeg (via your system package manager)
2. Clones + builds mneme
3. Downloads the bundled ONNX model (bge-small, 33 MB)
4. Starts the supervisor
5. Runs `mneme install` to configure every detected AI tool

> **v0.1.0 status:** script works on a machine that already has the core dev tools. Full auto-install of Rust/Bun/Python on a blank box is exercised in v0.1.1.

---

## Verify the install

```bash
mneme --version              # should print 0.1.0
mneme doctor                 # runs in-process + IPC checks; prints SLA snapshot
mneme daemon status          # live per-child JSON from the supervisor
curl http://127.0.0.1:7777/health   # raw daemon health endpoint
```

If `mneme doctor` prints the SLA snapshot and `daemon status` lists 40 running workers, you're done.

---

## Uninstall

```bash
# Stop the daemon
mneme daemon stop

# Remove the installed files
mneme uninstall              # reverses `mneme install` across every platform

# Optional: remove all per-project data too
rm -rf ~/.mneme/             # nukes projects, caches, snapshots, logs
```

---

## Troubleshooting

### "supervisor is not reachable"

The daemon isn't running. Start it:
```bash
mneme-supervisor start
```

On Windows, you can also install it as a service that auto-starts on login:
```bash
mneme-supervisor install    # registers DatatreeDaemon service
```

### "C# grammar skipped" warning

Expected in v0.1.0. The `tree-sitter-c-sharp` crate version (v15 ABI) doesn't match the Tree-sitter runtime (v13–14 ABI). C# files are simply not indexed; every other language works. Bumping pending in v0.2.

### "bun not found" when running supervisor-spawned MCP server

Only affects the optional supervisor-managed MCP child, which v0.1.0 intentionally doesn't spawn (Claude Code starts `mneme mcp stdio` itself). If you want to run the MCP server outside Claude Code, set `DATATREE_BUN` env var to the absolute path of your `bun` binary.

### Build fails on Windows with "link.exe not found"

You need the Visual Studio Build Tools (the C/C++ workload). Install via:
```
winget install Microsoft.VisualStudio.2022.BuildTools
```

### I see another GitHub account name in the repo's contributors

That's a GitHub-side email-to-account mapping. If your commit email is verified on a different GitHub account, GitHub credits that account. Fix: set your git user.email to the repo owner's noreply email:
```bash
git config user.email "229182351+omanishay-cyber@users.noreply.github.com"
```

### "MCP tools return empty responses"

Run `mneme build .` in your project first. The MCP tools read from the project's `graph.db` shard; if you haven't built it, queries return empty results.

---

## What gets installed where

```
~/.mneme/
├── bin/                    # Rust binaries (supervisor, store, parsers, scanners, livebus, brain, multimodal-bridge, cli)
├── mcp/                    # Bun MCP server source (copied from repo)
├── vision/                 # Vision app source (copied from repo)
├── projects/<sha>/         # per-project shard: graph.db, history.db, tasks.db, …
├── snapshots/              # hourly rolling snapshots of each project
├── cache/                  # docs cache, embedding cache, multimodal cache
├── logs/                   # supervisor.log, per-worker logs
└── supervisor.pipe         # Windows: discovery file for PID-scoped named pipe

~/.claude.json              # Claude Code MCP server registration (mneme added)
~/CLAUDE.md                 # mneme block injected (marker-based, idempotent)
~/AGENTS.md                 # universal agents file (Codex / OpenCode / Cursor / ...)
~/.codex/config.toml        # Codex MCP server entry
~/.cursor/mcp.json          # Cursor MCP server entry
... and so on for every AI tool that was auto-detected
```

All file writes are marker-wrapped (`<!-- mneme-start v1.0 -->` / `<!-- mneme-end -->`) so `mneme install` is safely idempotent — run it as many times as you want.

---

## Next steps

- Read [docs/architecture.md](docs/architecture.md) to understand the system
- Read [docs/mcp-tools.md](docs/mcp-tools.md) for the 33+ MCP tool reference
- Read [docs/faq.md](docs/faq.md) for common questions
- Open an [Issue](https://github.com/omanishay-cyber/mneme/issues) or a [Discussion](https://github.com/omanishay-cyber/mneme/discussions) if you hit something not covered here
