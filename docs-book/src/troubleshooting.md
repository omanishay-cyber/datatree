# Troubleshooting

Common issues and what they mean. Searchable above (top-right corner).

## "http://localhost:7777/ is empty"

**This is expected when no project is built yet** — it's not a bug. The daemon serves the vision SPA but needs at least one indexed project to show data.

**Fix:**

```bash
cd ~/your-project
mneme build .          # one-time index pass
```

Then refresh the browser. Or use `mneme view` from inside a project — it auto-attaches the project hash:

```bash
cd ~/your-project
mneme view             # opens http://localhost:7777/?project=<hash>
```

**Diagnose the empty state:**

```bash
curl -s http://localhost:7777/api/health | jq .
# Returns { "status": "ok", "uptime_s": ... } when the daemon is alive

ls ~/.mneme/projects/
# Should list one directory per built project

curl -s http://localhost:7777/api/graph/status | jq .
# Returns node/edge/file counts; all-zero = nothing built yet
```

If `/api/health` doesn't respond, the daemon isn't running. `mneme daemon start` brings it up.

## "mneme: command not found" after install

PATH wasn't refreshed. Open a new shell or source your profile:

- bash/zsh: `export PATH="$HOME/.mneme/bin:$PATH"`
- fish: `set -Ux fish_user_paths $HOME/.mneme/bin $fish_user_paths`
- PowerShell: `$env:Path = "$env:USERPROFILE\.mneme\bin;" + $env:Path`

Persistent fix (Linux/macOS): add the export to `~/.bashrc` / `~/.zshrc`. Persistent fix (Windows): the install script writes the user PATH for you; if it didn't, run the bootstrap installer again.

## "mneme: connected — 0 tools" in Claude Code

The MCP entry registered but the bun-installed `node_modules` is empty (the v0.3.2 install bug B1). Re-run install:

```bash
mneme install --platform=claude-code --force
```

Verify:

```bash
ls ~/.mneme/mcp/node_modules/zod/package.json
# Should exist; if not, the bun install failed during install
```

## `recall_concept` returns README chunks, not the function

Two things to check:

1. **Have you re-built since upgrading to the Genesis keystone?** v0.3.x embeddings are file-anchored. The schema migration v1→v2 clears them on first build, but the build has to actually run. `mneme build .` from any indexed project triggers it.

2. **Is BGE actually loaded?** `mneme doctor` reports the active embedder backend at the bottom. If it says `hashing-trick`, the BGE model isn't installed:

   ```bash
   mneme models install bge-small-en-v1.5
   ```

   Then re-build.

## Self-update rolled back

The post-swap health check spawned the new `mneme --version`, didn't get exit 0 within 5 s, and restored the `.old` backups. Most common cause on Windows: Defender's first-run scan of the new 56 MB unsigned binary takes longer than 5 s. Retry:

```bash
mneme self-update
```

Second run is fast because Defender has cached the scan. If it rolls back twice in a row, the binary genuinely doesn't run on your machine — file an issue with `mneme doctor` output and the OS version.

## ForceGalaxy edges aren't showing

Three layers can drop edges silently:

1. **Node window mismatch** — fixed in Genesis (`?limit=` parameter symmetric across `/nodes`, `/edges`, `/layout`)
2. **Server-side INNER JOIN** — fixed in Genesis (Item #111)
3. **No edges in graph.db** — possible on a fresh `--no-edges` build or a project whose parser doesn't yet emit call edges

Check edge count:

```bash
curl -s "http://localhost:7777/api/graph/status?project=<hash>" | jq .edges
```

Zero = parser ran but no calls were extracted. For Rust, this could be a v0.3.x build before Item #93 (Rust call edges) shipped — re-run `mneme build .`.

## "Failed to bind 127.0.0.1:7777"

Another process holds the port. The daemon doesn't try alternate ports.

```bash
# Linux/macOS
lsof -i :7777
# Find the offending PID and kill it, or change Mneme's listen port

# Windows
Get-NetTCPConnection -LocalPort 7777 | Format-List
```

If it's a stale `mneme-daemon` from a previous run:

```bash
mneme daemon stop      # graceful
# or:
pkill -f mneme-daemon  # force
```

## Schema migration "may take a moment"

The v1→v2 migration runs `DELETE FROM embeddings` and `UPDATE nodes SET embedding_id = NULL`. On a 100K-row shard this can take 5-30 s and locks the WAL. The migration emits a `tracing::info` line so it's visible if you have `MNEME_LOG=info`:

```text
[INFO ] applying schema migration (may take a moment on large shards) layer=Graph from=1 to=2
```

If the migration appears to hang for more than 60 s, kill the daemon, restart, and re-run `mneme build`. The migration is transactional — a partial run leaves the shard at the previous version, and the next attempt retries cleanly.

## "Cargo lockfile drift" / Bun lockfile mismatch

The release ships with locked dependencies. If you're building from source:

```bash
cd source
cargo build --workspace --release   # uses Cargo.lock
cd mcp
bun install --frozen-lockfile        # uses bun.lockb
cd ../vision
bun install --frozen-lockfile
```

Don't `bun install` without `--frozen-lockfile` unless you intend to update — accidental updates can drift the MCP server's `zod` / `@modelcontextprotocol/sdk` versions.

## Audit subprocess timeout

Audits run a separate `mneme-scanners` worker pool with a wall-clock cap. On large projects (>10K files) the cap can fire before all scanners finish. As of v0.3.2 (B12 fix), partial findings stream to `findings.db` per-batch — even on timeout you get the work that completed.

For long audits:

```bash
mneme audit --timeout=1800     # 30 min cap
```

## Daemon dies mid-build

The daemon supervises worker processes; if a worker dies, the supervisor restarts it. If the supervisor itself dies (rare), the build aborts. Check:

```bash
mneme daemon logs              # tail recent log
~/.mneme/log/daemon.log        # full log
```

A common cause is the BGE model file getting locked by another process (e.g. an antivirus mid-scan during model load). Kill the daemon, wait 30 s, restart.

## Hook flashes a console window on Windows

You're using the `mneme.exe` CLI binary as your hook target instead of `mneme-hook.exe`. The platform integration writes the right path automatically; if you've edited Claude Code's `settings.json` by hand, point `command` at `~/.mneme/bin/mneme-hook.exe` instead.

## "DLL load failed" / ORT error

The bundled `onnxruntime.dll` (1.24.4) didn't load on first BGE inference. The most common cause is a different `onnxruntime.dll` already in `System32` from another tool — Windows resolves the system DLL first. The install script normally pins the bundled DLL via `ORT_DYLIB_PATH`; if that env var isn't set:

```powershell
$env:ORT_DYLIB_PATH = "$env:USERPROFILE\.mneme\bin\onnxruntime.dll"
mneme build .
```

To make persistent, add the env var to your user profile (Windows Settings → System → About → Advanced → Environment Variables).

## Filing an issue

Include:

- `mneme --version` output
- `mneme doctor` output (full)
- OS + arch
- The exact command that failed
- The full error message

[GitHub Issues →](https://github.com/omanishay-cyber/mneme/issues)
