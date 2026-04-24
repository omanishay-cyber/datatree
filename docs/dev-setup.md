# Developer setup

You want to work on mneme itself (not just use it). Here's the ~20-minute setup.

## Prereqs

| Tool | Version | Install hint |
|---|---|---|
| Rust | 1.78+ stable | `winget install Rustlang.Rustup` / `curl https://sh.rustup.rs` |
| Bun | 1.3+ | `winget install Oven-sh.Bun` / `curl -fsSL https://bun.sh/install \| bash` |
| Python | 3.10+ | `winget install Python.Python.3.12` / system package manager |
| Git | any recent | system package manager |
| C/C++ toolchain | platform default | Windows: VS 2022 Build Tools · macOS: Xcode CLT · Linux: `build-essential` |

Nice-to-haves:
- **rust-analyzer** for your IDE — Rust inspection
- **Bun extension** for VS Code — Bun-flavoured TS completions
- **sqlite3** CLI — handy for inspecting shards

## Clone

```bash
git clone https://github.com/omanishay-cyber/mneme
cd mneme
```

## One-time build

```bash
# Rust workspace — 10 crates, 400+ transitive deps
cargo build --workspace            # debug build, ~5 min cold
# or
cargo build --workspace --release  # release build, ~10 min cold

# MCP server
cd mcp && bun install && cd ..

# Vision app
cd vision && bun install && cd ..

# Python multimodal sidecar
cd workers/multimodal && pip install -e . && cd ../..
```

## Run the daemon

```bash
# Foreground (Ctrl+C to stop):
cargo run --bin mneme-supervisor -- start

# Or use the built binary directly:
./target/debug/mneme-supervisor.exe start   # Windows
./target/debug/mneme-supervisor start       # macOS/Linux
```

The supervisor spawns 40 workers and binds `http://127.0.0.1:7777/health`. Hit it:
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

## Development loop

### Add a new MCP tool

1. Add input/output Zod schemas to `mcp/src/types.ts`
2. Create `mcp/src/tools/your_tool.ts` — follow the pattern in `mcp/src/tools/blast_radius.ts`
3. If you need a new DB query shape, add a helper to `mcp/src/store.ts`
4. Drop the file into the tools folder while the daemon is running — hot-reload picks it up in 250 ms

### Add a new Tree-sitter language

1. Add the grammar crate to `parsers/Cargo.toml` behind a feature flag
2. Register in `parsers/src/language.rs`:
   - Add variant to the `Language` enum
   - Add file-extension mapping to `from_extension`
   - Add `tree_sitter_language()` arm
3. Add per-language query patterns to `parsers/src/query_cache.rs`
4. `cargo build --features your_lang`

### Add a new scanner

1. Create `scanners/src/scanners/your_rule.rs` — copy `theme.rs` as a template
2. Implement the `Scanner` trait: `name()`, `applies_to(file)`, `scan(file, content, ast)`
3. Register in `scanners/src/registry.rs`
4. `cargo build -p mneme-scanners`

### Add a new vision view

1. Create `vision/src/views/YourView.tsx` — copy `ForceGalaxy.tsx` as a template
2. Add an entry to `vision/src/views/index.ts`
3. `cd vision && bun run dev`

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

v0.3.0 ships with `cargo test --workspace` fully green (280+ tests, 0 failed, 0 ignored) — parsers, supervisor, store, scanners, brain, md-ingest, cli, livebus all pass (includes 30 new supervisor/common tests for the job-dispatch path, 4 new brain tests for the ONNX inference path, and 7 new scanner tests).

## Debugging

```bash
# Maximum verbosity
MNEME_LOG=trace cargo run --bin mneme-supervisor -- start

# Single-subsystem trace
MNEME_LOG=mneme_store=trace,info cargo run --bin mneme-supervisor -- start

# Inspect the daemon's log ring over IPC
cargo run --bin mneme -- daemon logs
```

## CI

`.github/workflows/ci.yml` runs on every push:
- Rust build + clippy + tests (Ubuntu / macOS / Windows)
- MCP server `bun install` + `tsc --noEmit`
- Vision app `bun install` + `tsc --noEmit`
- Python multimodal `pip install` + `compileall`
- LICENSE header check

v0.2.x CI runs green on the core build/test/tsc steps; `continue-on-error` is now limited to a shrinking set of legacy checks (vision TS errors + some platform-specific bench paths).

## Code style

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full rules. Summary:
- **Rust** — `cargo fmt`, clippy warnings are errors, no `unwrap()` on user-input paths
- **TypeScript** — strict mode, no `any`, zod at the boundary, named exports only
- **Python** — strict type hints, pydantic at IPC boundaries, no blocking I/O

## Where to ask

- [GitHub Issues](https://github.com/omanishay-cyber/mneme/issues) — bugs
- [GitHub Discussions](https://github.com/omanishay-cyber/mneme/discussions) — design questions, "is this a good idea?"

---

[← back to README](../README.md)
