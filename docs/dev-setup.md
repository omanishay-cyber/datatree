# Developer setup

You want to work on mneme itself (not just use it). Here's the ~20-minute setup
to build v0.3.2 from source.

If you only want to *use* mneme, follow [`docs/INSTALL.md`](INSTALL.md) instead -
the bootstrap installer pulls pre-built binaries and you don't need any of the
toolchains below.

## Prereqs

| Tool | Version | Install hint |
|---|---|---|
| Rust | 1.78+ stable | `winget install Rustlang.Rustup` / `curl https://sh.rustup.rs` |
| **Bun** | **1.3+** (HARD prereq for the MCP server + Vision SPA) | Win: `irm bun.sh/install.ps1 \| iex` * Unix: `curl -fsSL https://bun.sh/install \| bash` * `winget install Oven-sh.Bun` |
| Python | 3.10+ | `winget install Python.Python.3.12` / system package manager |
| Git | any recent | system package manager |
| C/C++ toolchain | platform default | Windows: VS 2022 Build Tools * macOS: Xcode CLT * Linux: `build-essential` |

> **Bun on PATH is a hard prerequisite for `mcp/src/index.ts` and the Vision
> dev server.** Both spawn `bun` directly. Without Bun on PATH the MCP server
> fails at boot and the Vision SPA never starts. Install Bun via the one-liner
> above before running any `bun run` script under `vision/` or `mcp/`.
>
> The standalone Tauri shell at `vision/tauri/` is build-only as of v0.3.2 -
> the shipped Vision UX is the Bun-served SPA at `http://127.0.0.1:7777/`.
> Tauri is excluded from the workspace and does not need to be built unless
> you're hacking on the desktop wrapper.

Nice-to-haves:
- **rust-analyzer** for your IDE - Rust inspection
- **Bun extension** for VS Code - Bun-flavoured TS completions
- **sqlite3** CLI - handy for inspecting shards

## Clone

```bash
git clone https://github.com/omanishay-cyber/mneme
cd mneme
```

## One-time build

```bash
# Rust workspace - 12 crates, 400+ transitive deps
cargo build --workspace            # debug build, ~5 min cold
# or
cargo build --workspace --release  # release build, ~10 min cold

# MCP server
cd mcp && bun install && cd ..

# Vision app
cd vision && bun install && cd ..

# Python multimodal sidecar (optional)
cd workers/multimodal && pip install -e . && cd ../..
```

### Optional multimodal extractors

`multimodal-bridge/` ships with PDF + Markdown extraction enabled by
default (pure Rust, zero system deps). OCR / audio / video are **opt-in**
because they pull in heavy native libraries that most users do not need:

| Extractor | Feature flag | Required system deps |
|---|---|---|
| Image OCR | `tesseract` | `tesseract-ocr` (`apt install tesseract-ocr` / `brew install tesseract` / `winget install UB-Mannheim.TesseractOCR`) |
| Audio (Whisper) | `whisper` | C++ toolchain + a Whisper GGML model on disk |
| Video frames | `ffmpeg` | `libavformat` / `libavcodec` / `libavutil` |

Build commands:

```bash
# Image OCR only
cargo build -p mneme-multimodal --features tesseract

# Everything (CI convenience)
cargo build -p mneme-multimodal --features all-extractors
```

Tesseract is not bundled because it adds ~50 MB of native binaries and forces
every user to install a C++ toolchain even if they only ever extract PDFs.

## Run the daemon

```bash
# Foreground (Ctrl+C to stop):
cargo run --bin mneme-supervisor -- start

# Or use the built binary directly:
./target/debug/mneme-supervisor.exe start   # Windows
./target/debug/mneme-supervisor start       # macOS/Linux
```

The supervisor spawns 22 worker processes auto-scaled to your CPU count -
`1 (store) + num_cpus (parsers) + num_cpus/2 (scanners) + 1 (md-ingest) + 1 (brain) + 1 (livebus) + ...` -
and binds `http://127.0.0.1:7777/health`. On a typical 8-core dev box that's
`1 + 8 + 4 + 1 + 1 + 1 = 16` per-class workers (the rest are dispatch + queue
workers). Hit it:

```bash
curl http://127.0.0.1:7777/health
```

## Make your first build

```bash
# In another terminal, with the daemon running:
cargo run --bin mneme -- build .

# You should see:
# walked:  374 files
# indexed: 50+
# nodes:   1000+
# edges:   2000+
# shard:   ~/.mneme/projects/<sha>/
```

Pass `--rebuild` to force a full re-parse (added in v0.3.2 / B11):

```bash
cargo run --bin mneme -- build . --rebuild
```

## Development loop

### Add a new MCP tool

1. Add input/output Zod schemas to `mcp/src/types.ts`
2. Create `mcp/src/tools/your_tool.ts` - follow the pattern in `mcp/src/tools/blast_radius.ts`
3. If you need a new DB query shape, add a helper to `mcp/src/store.ts`
4. Drop the file into the tools folder while the daemon is running - hot-reload picks it up in 250 ms

### Add a new Tree-sitter language

1. Add the grammar crate to `parsers/Cargo.toml` behind a feature flag
2. Register in `parsers/src/language.rs`:
   - Add variant to the `Language` enum
   - Add file-extension mapping to `from_extension`
   - Add `tree_sitter_language()` arm
3. Add per-language query patterns to `parsers/src/query_cache.rs`
4. `cargo build --features your_lang`

### Add a new scanner (currently 11 built-in)

1. Create `scanners/src/scanners/your_rule.rs` - copy `theme.rs` as a template
2. Implement the `Scanner` trait: `name()`, `applies_to(file)`, `scan(file, content, ast)`
3. Register in `scanners/src/registry.rs`
4. `cargo build -p mneme-scanners`

The full list of built-in scanners (see [`docs/architecture.md`](architecture.md#the-11-built-in-scanners)):
`theme`, `types_ts`, `security`, `a11y`, `perf`, `drift`, `ipc`, `markdown_drift`,
`secrets`, `refactor`, `architecture`.

### Add a new vision view

1. Create `vision/src/views/YourView.tsx` - copy `ForceGalaxy.tsx` as a template
2. Add an entry to `vision/src/views/index.ts`
3. The vision app needs **two** processes running side by side: the Vite SPA
   (port `5173`) and the Bun API server (port `7777`) that serves graph data.
   Open two terminals:

   ```bash
   # Terminal 1 - Vite SPA (UI)
   cd vision && bun run dev

   # Terminal 2 - Bun API server (graph data)
   cd vision && bun run serve
   ```

   Or use the bundled shortcut that starts both with `concurrently`:

   ```bash
   cd vision && bun run dev:full
   ```

## Inspect a shard directly

```bash
# Find the shard directory
ls ~/.mneme/projects/

# Open graph.db with the sqlite3 CLI
sqlite3 ~/.mneme/projects/<sha>/graph.db

sqlite> SELECT COUNT(*) FROM nodes;
sqlite> SELECT kind, COUNT(*) FROM nodes GROUP BY kind;
sqlite> SELECT qualified_name FROM nodes WHERE kind='function' LIMIT 5;
sqlite> SELECT source_qualified, target_qualified FROM edges WHERE kind='calls' LIMIT 5;
```

## Tests

```bash
# Rust unit tests
cargo test --workspace

# MCP server
cd mcp && bun test

# Multimodal sidecar
cd workers/multimodal && pytest
```

v0.3.2 ships with `cargo test --workspace` fully green - parsers, supervisor,
store, scanners, brain, md-ingest, cli, livebus, common, multimodal-bridge,
benchmarks all pass. Test counts grow with the v0.3.2 hotfix wave (52-fix
audit cycle plus B-007/B-017+ regression tests).

## Debugging

```bash
# Maximum verbosity
MNEME_LOG=trace cargo run --bin mneme-supervisor -- start

# Single-subsystem trace
MNEME_LOG=mneme_store=trace,info cargo run --bin mneme-supervisor -- start

# Inspect the daemon's log ring over IPC
cargo run --bin mneme -- daemon logs
```

See [`docs/env-vars.md`](env-vars.md) for the full `MNEME_*` environment
reference.

## CI

`.github/workflows/ci.yml` runs on every push:
- Rust build + clippy + tests on Ubuntu / macOS / Windows
- MCP server `bun install` + `tsc --noEmit`
- Vision app `bun install` + `tsc --noEmit`
- Cargo audit (RUSTSEC) - **block-on-fail**
- Cargo deny (license / bans / duplicates) - **block-on-fail**
- Doctor cross-platform path tests - **block-on-fail**
- E2E build + recall + blast on a real repo - **block-on-fail**
- LICENSE header check

A separate release workflow builds the 6 platform binaries (Win / Mac / Linux
x x64 / arm64) and uploads them to the release as ZIP / tarball assets used
by the bootstrap installers.

## Code style

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full rules. Summary:
- **Rust** - `cargo fmt`, clippy warnings are errors, no `unwrap()` on user-input paths
- **TypeScript** - strict mode, no `any`, zod at the boundary, named exports only
- **Python** - strict type hints, pydantic at IPC boundaries, no blocking I/O

## Where to ask

- [GitHub Issues](https://github.com/omanishay-cyber/mneme/issues) - bugs
- [GitHub Discussions](https://github.com/omanishay-cyber/mneme/discussions) - design questions, "is this a good idea?"

---

[← back to README](../README.md)
