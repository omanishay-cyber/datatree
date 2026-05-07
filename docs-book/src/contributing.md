# Contributing

Mneme is under the **Mneme Personal-Use License** — source-available, not
open-source. Contributions are still welcome on
[GitHub](https://github.com/omanishay-cyber/mneme), but please read this
first: by submitting a PR you assign copyright on your contribution to
the project (per LICENSE §2(b)). This keeps the codebase under a single
owner so the license model stays coherent. If that's a deal-breaker for
you, that's fair — just open an issue or email instead.

## Build from source

```bash
git clone https://github.com/omanishay-cyber/mneme
cd mneme/source

# Rust workspace
cargo build --workspace --release

# MCP server (TypeScript)
cd mcp
bun install --frozen-lockfile
bunx tsc --noEmit            # type-check

# Vision SPA
cd ../vision
bun install --frozen-lockfile
bun run build
```

## Local gates before pushing

The repo has 5 mandatory gates that must pass before any push (we get burned by CI failures otherwise):

```bash
cd source

# 1. Rustfmt
cargo fmt --all -- --check

# 2. Workspace cargo check
cargo check --workspace

# 3. Home-dir-discipline gate (no dirs::home_dir() in mneme paths)
bash scripts/check-home-dir-discipline.sh

# 4. Vision SPA TypeScript check
cd vision && bunx tsc --noEmit && cd ..

# 5. MCP TypeScript check
cd mcp && bun install --frozen-lockfile && bunx tsc --noEmit && cd ..
```

If any gate fails, fix locally before pushing — a failing CI 16 minutes after push is worse for everyone than a 30-second local check.

## Test surface

```bash
# Rust workspace — 747+ tests
cargo test --workspace --lib

# Targeted test runs (faster iteration)
cargo test -p mneme-parsers --lib resolver
cargo test -p mneme-cli --lib commands::pretool_grep_read
cargo test -p mneme-daemon --lib api_graph

# Integration tests
cargo test -p mneme-store --test migrations
```

## Coding conventions

- **Rustfmt** — required. Default config (`rustfmt.toml` shipped at repo root).
- **No `dirs::home_dir()` for mneme paths** — use `common::PathManager`. The home-dir-discipline gate enforces this.
- **No `dbg!` / `println!` left in committed code** — use `tracing::info!` / `tracing::warn!` etc.
- **Single-writer-per-shard** — every SQL write goes through `store.inject` or `store.query.write`. No direct `Connection::execute` outside the writer task.
- **Hook fail-open** — every hook subcommand returns `Ok(())` from its entrypoint. Errors get swallowed via `.ok()` / `.unwrap_or_default()`.
- **Stable exit codes** — defined in `cli/src/error.rs::CliError::exit_code`. Never change them; only add new variants.

## Versioning

NEVER hand-edit version strings. Use the bumper script:

```bash
bash scripts/bump-version.sh 0.4.0 0.4.1 [--dry-run]
```

The script knows about every version-tied file in the repo (Cargo.toml, package.json, install scripts, GH Actions tag references, etc.). Hand-edits drift; the script doesn't.

## PR checklist

- [ ] All 5 local gates green
- [ ] `cargo test --workspace --lib` green
- [ ] CHANGELOG entry under the active version
- [ ] If the change touches a public API: rustdoc updated
- [ ] If the change touches a hook surface: `mneme/src/hooks/*.ts` design-of-record sibling updated to match
- [ ] If the change touches a schema: SCHEMA_VERSION bumped + migration block added
- [ ] If the change touches the install path: tested on Linux + macOS + Windows VM (or noted as deferred to a maintainer)

## Reading order for new contributors

1. [Architecture](./concepts/architecture.md) — what's running, where, and why
2. [Symbol resolver](./concepts/resolver.md) — the Genesis keystone work
3. [Self-ping enforcement](./concepts/self-ping.md) — the AI integration story
4. `cli/src/main.rs` — the CLI dispatch tree, every subcommand maps to one handler
5. `supervisor/src/api_graph.rs` — the HTTP API the vision SPA + external automation talk to

## Contact

GitHub Issues for bug reports + feature requests. PRs welcome — small focused PRs land faster than big multi-feature ones.

— Anish & Kruti
