# fireworks-rust -- Cargo Workspaces

> Workspace dependencies, profile tuning, feature flags, publishing.
> Scaling from one crate to many.

---

## 1. When to Use a Workspace

Any project with more than about 2000 lines or more than one binary/library is a workspace candidate.

Indicators you should split:

- One crate with multiple binaries (CLI, server, worker).
- Library code you want to publish separately.
- Tests and benchmarks that need to compile independently.
- Integration tests that need heavy shared infrastructure.
- Team structure where different engineers own different subsystems.

Mneme is a workspace of 11+ crates: `common`, `store`, `supervisor`, `parsers`, `scanners`, `brain`, `livebus`, `multimodal-bridge`, `cli`, `md-ingest`, `benchmarks`. Each is small, testable, and has a single responsibility. That structure makes compile times manageable and dependencies explicit.

---

## 2. Workspace Root

```toml
# Cargo.toml at the workspace root
[workspace]
resolver = "2"
members = [
    "common",
    "store",
    "supervisor",
    "cli",
]

# Optional: exclude some paths (e.g., generated code, experimental crates)
exclude = [
    "experimental/scratch",
]
```

`resolver = "2"` is the current default; use it for any new workspace.

Member paths are directories relative to the root. Each member has its own `Cargo.toml`.

---

## 3. Workspace-Wide Package Settings

```toml
[workspace.package]
version = "0.3.0"
edition = "2021"
rust-version = "1.78"
license = "Apache-2.0"
authors = ["Your Name"]
repository = "https://github.com/example/myproject"
homepage = "https://github.com/example/myproject"
description = "My project"
```

Each member crate inherits with `workspace = true`:

```toml
# common/Cargo.toml
[package]
name = "myproject-common"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
```

Bumping the version happens in one place. Crates stay in sync.

---

## 4. Workspace Dependencies

```toml
# Root Cargo.toml
[workspace.dependencies]
# External
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"

# Internal (members referenced by path)
myproject-common = { path = "common", version = "0.3.0" }
myproject-store = { path = "store", version = "0.3.0" }
```

Member Cargo.toml inherits:

```toml
# store/Cargo.toml
[dependencies]
myproject-common = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }

# Member-specific dependencies still allowed
rusqlite = { version = "0.32", features = ["bundled"] }
```

### Why This Matters

- One version of each dependency across the whole workspace. No "crate A uses tokio 1.30, crate B uses tokio 1.40" surprises.
- Bumping a dependency: change the root, rebuild the workspace. That's it.
- Enabling features: can be done at the root for consistency.

### Mixed Versions (When Necessary)

Sometimes a crate legitimately needs a different version (a generated code crate, a vendor-pinned dep). Override per-member:

```toml
# special-crate/Cargo.toml
[dependencies]
tokio = "1.30"  # explicitly pinned, overrides workspace
```

Cargo will have both versions in the dependency graph. Rare but supported.

---

## 5. The target/ Directory

Workspace builds share a single `target/` directory at the root. This matters:

- All members build into the same place.
- Shared dependency compilations are reused across crates.
- `cargo clean` at the root cleans everything.

Gitignore the target directory:

```
# .gitignore
/target
```

For CI, cache `~/.cargo/registry` and `target/` aggressively -- builds are slow from scratch.

---

## 6. Commands Against a Workspace

```bash
# Build everything
cargo build --workspace

# Test everything
cargo test --workspace

# Clippy across all targets
cargo clippy --workspace --all-targets -- -D warnings

# Format everything
cargo fmt --all

# Just one member
cargo build -p myproject-common
cargo test -p myproject-store

# Run a specific binary member
cargo run -p myproject-cli -- --flag value
```

The `-p` flag selects a single member. `--workspace` (or `--all`) touches everything.

---

## 7. Release Profile Tuning

```toml
# Root Cargo.toml
[profile.release]
lto = "fat"              # aggressive link-time optimisation
codegen-units = 1         # all in one unit for max inlining
panic = "abort"           # no unwinding machinery
strip = "symbols"         # smaller binary
```

### lto

- `false` (default) -- no LTO.
- `"thin"` -- parallel LTO, modest wins, fast.
- `"fat"` -- single-threaded LTO across the whole binary. Slow build, biggest wins.

For binaries shipped to users, `"fat"` is typically worth the build time.

### codegen-units

- Default: 16 in release. Parallel compilation, less optimisation.
- Set to 1 for maximum inlining at the cost of slower builds.

Pair with `lto = "fat"` for best results.

### panic = "abort"

Default in release is `unwind` -- a panic runs destructors and unwinds the stack. With `abort`, a panic immediately terminates the process. Benefits:

- Smaller binary (no unwinding code).
- Often 10-20% faster in hot paths.
- Simpler debugging (process dies where the panic happens).

Only set `panic = "abort"` in binary crates, not library crates. Libraries that set it force the setting on their dependents.

### strip

- `"none"` -- keep debug symbols.
- `"debuginfo"` -- strip debug info, keep public symbols.
- `"symbols"` -- strip everything. Smallest binary, hardest to debug.

For production binaries, `"symbols"` is fine -- use `cargo build --release --profile=release-debug` for a version with symbols if you need to investigate prod crashes.

### Debug Profile Tweaks

```toml
[profile.dev]
debug = 2                # full debug info (default)
opt-level = 0             # no optimisation (default)
incremental = true        # default; fast iterative builds

[profile.dev.package."*"]
opt-level = 3             # compile deps with optimisation even in dev
```

That last block is useful when a dependency is slow in debug mode (e.g., image processing, cryptography, parsers). Your code stays fast to rebuild; their code runs fast at runtime.

### Custom Profiles

```toml
[profile.release-debug]
inherits = "release"
debug = true
strip = false
```

Define as many as you need. `cargo build --profile=release-debug`.

---

## 8. Feature Flags

Features let users opt in to optional functionality.

```toml
# mycrate/Cargo.toml
[features]
default = ["json"]
json = ["dep:serde_json"]
yaml = ["dep:serde_yaml"]
postgres = ["dep:sqlx", "sqlx/postgres"]
sqlite = ["dep:rusqlite"]

[dependencies]
serde_json = { version = "1", optional = true }
serde_yaml = { version = "0.9", optional = true }
sqlx = { version = "0.7", optional = true, features = ["runtime-tokio"] }
rusqlite = { version = "0.32", optional = true }
```

Users pick features at install:

```toml
[dependencies]
mycrate = { version = "1", default-features = false, features = ["postgres"] }
```

### `dep:` Syntax

The `dep:serde_json` form (Rust 2021+) explicitly references the optional dependency. Without it, the feature name and dep name must match and you can accidentally expose dep names as feature names.

### Gating Code

```rust
#[cfg(feature = "postgres")]
pub mod postgres {
    pub fn connect() { ... }
}

#[cfg(all(feature = "postgres", feature = "sqlite"))]
compile_error!("cannot enable both postgres and sqlite");
```

Use `compile_error!` to reject combinations that don't make sense.

### Testing Feature Combinations

```bash
cargo test --no-default-features                  # bare
cargo test --no-default-features --features json
cargo test --no-default-features --features postgres
cargo test --all-features                          # maximum
```

CI should test at least: default, no-default, all-features. For many feature combinations, use a matrix job.

---

## 9. Cross-Compilation

```bash
# Install a target
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-apple-darwin

# Build for it
cargo build --target x86_64-unknown-linux-musl --release
cargo build --target aarch64-apple-darwin --release
```

Add a `.cargo/config.toml` for project defaults:

```toml
# .cargo/config.toml
[build]
target = "x86_64-unknown-linux-musl"

[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
```

For more complex cross-compilation (Android, iOS, embedded), look at `cross` or `cargo-zigbuild` which handle the C toolchain.

---

## 10. Path Dependencies vs Registry Dependencies

```toml
# Path -- local, no version constraint
myproject-common = { path = "../common" }

# Registry -- from crates.io
serde = "1"

# Both -- use path for local dev, fall back to version for publishing
myproject-common = { path = "../common", version = "0.3.0" }
```

The `version` field is required if you plan to publish. When you run `cargo publish`, path dependencies without versions are rejected.

For workspace members, use the workspace dependency pattern (see section 4) which handles this automatically.

---

## 11. Publishing to crates.io

### Set Up

```bash
cargo login <YOUR_TOKEN>
```

Token from https://crates.io/me.

### Publish a Single Crate

```bash
cd mycrate
cargo publish --dry-run      # verify everything is ready
cargo publish
```

### Publishing a Workspace

crates.io only accepts one crate at a time. Publish in dependency order:

```bash
cargo publish -p myproject-common
cargo publish -p myproject-store    # depends on common
cargo publish -p myproject-cli      # depends on both
```

Each publish needs to wait a few seconds for the registry to index before the next one can resolve it. Some workspaces use `cargo-workspaces` or `cargo-release` to automate this.

### Required Metadata

```toml
[package]
name = "mycrate"
version = "1.0.0"
description = "What it does in one line"
license = "MIT OR Apache-2.0"
repository = "https://github.com/example/mycrate"
documentation = "https://docs.rs/mycrate"
readme = "README.md"
keywords = ["async", "http", "server"]
categories = ["web-programming::http-server"]
```

`license`, `description`, and `repository` are required for publishing. Others are strongly recommended.

### Include / Exclude

```toml
[package]
include = [
    "src/**/*",
    "Cargo.toml",
    "README.md",
    "LICENSE*",
]
# OR
exclude = [
    "tests/fixtures/large-blob.bin",
    "benchmarks",
]
```

Keep the published tarball small. Default includes everything except `target/`, `.git/`, and a few other paths.

### Verify the Tarball

```bash
cargo package --list       # preview what goes in
cargo package              # build the tarball locally
```

The tarball ends up in `target/package/`. Open it with `tar` to verify.

---

## 12. Version Management

SemVer for libraries:

- Breaking API change -> MAJOR bump.
- New API, no breaks -> MINOR bump.
- Bug fix, no API change -> PATCH bump.

Pre-1.0 (0.x.y): minor bumps can break; patch bumps cannot.

### Coordinated Workspace Versioning

Option A: All members share a single version.

```toml
[workspace.package]
version = "0.3.0"
```

Simple, but forces unnecessary bumps (bumping `cli` requires bumping `common` too).

Option B: Per-member versions, internal deps pinned to path + version.

```toml
[workspace.dependencies]
myproject-common = { path = "common", version = "0.3.0" }
myproject-store = { path = "store", version = "0.5.2" }
```

More flexible but requires careful bumping.

### Tools

- `cargo-release` -- automates version bumps, tags, and publishes.
- `cargo-workspaces` -- similar; handles the workspace case specifically.

Both generate a changelog from commits if you follow conventional commit style.

---

## 13. Cargo.lock

### Commit or Ignore?

- Binary/application workspace: commit `Cargo.lock`. Reproducible builds.
- Library crate (published to crates.io): gitignore `Cargo.lock`. Library users must resolve themselves.

Most workspaces are a mix: commit the lock at the root; crates within are published as libraries but use the lock for development.

### Updating

```bash
cargo update                       # update all to latest within constraints
cargo update -p serde              # just one
cargo update -p serde --precise 1.0.195  # specific version
```

After updating, run the full test suite. Cargo's constraint solver is quite good, but breaking API changes within a semver range do happen.

---

## 14. Common Workspace Structures

### Binary + Supporting Libraries

```
myproject/
  Cargo.toml          # workspace
  common/             # shared types
  store/              # DB access
  server/             # HTTP server
  cli/                # command-line tool
  worker/             # background worker
```

Common at the bottom; server/cli/worker depend on it.

### Library with Example Binaries

```
mylib/
  Cargo.toml          # workspace
  mylib/              # the published library
  mylib-cli/          # example CLI using mylib
  mylib-server/       # example server using mylib
```

Library crate is what gets published; binaries are for development/examples.

### Plugin Architecture

```
myapp/
  Cargo.toml          # workspace
  core/               # core types and traits
  plugins/            # plugin crates
    plugin-a/
    plugin-b/
    plugin-c/
  app/                # main app that loads plugins
```

Each plugin implements traits from `core`. The app dynamically loads them (via cdylib + libloading) or statically links them.

---

## 15. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| Version drift (same dep, multiple versions) | Use workspace.dependencies |
| Path deps without `version` | Add version so it publishes |
| Massive single crate (10k+ lines) | Split into workspace members |
| Publishing one workspace member without updating internal deps | Use cargo-release |
| Committing target/ | Gitignore it |
| Manually bumping 11 versions per release | cargo-release or cargo-workspaces |
| No LTO on release binaries | Enable at least "thin" |
| panic = "abort" in a library crate | Only set in binary crates |
| Platform-specific code without cfg | Use `#[cfg(target_os = "...")]` |
| Optional features without testing combinations | CI matrix covering default, none, all |

---

## 16. Cross-References

- [full-guide.md](full-guide.md) -- project layout
- [errors.md](errors.md) -- thiserror in library crates vs anyhow in binary crates
- [async-rust.md](async-rust.md) -- tokio as a workspace-wide dependency
- [unsafe-and-miri.md](unsafe-and-miri.md) -- Miri in CI across a workspace
