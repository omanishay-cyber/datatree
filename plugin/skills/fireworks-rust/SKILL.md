---
name: fireworks-rust
version: 1.0.0
author: mneme
description: Use when writing Rust services, navigating the borrow checker, designing async code with tokio, building trait systems, handling errors with thiserror/anyhow, organising Cargo workspaces, writing integration tests with criterion benchmarks, or working with unsafe Rust and Miri. Covers Rust 1.75+, async-fn-in-trait, structured concurrency via JoinSet, pin semantics, and workspace-scale profile tuning.
triggers:
  - rust
  - cargo
  - tokio
  - async
  - borrow
  - lifetime
  - trait
  - enum
  - result
  - option
  - serde
  - clap
  - axum
  - actix
  - rusqlite
  - sqlx
  - wgpu
  - bevy
  - unsafe
  - pin
  - arc
  - mutex
  - rayon
tags:
  - rust
  - cargo
  - borrow-checker
  - lifetimes
  - async-rust
  - tokio
  - traits
  - serde
---

# FIREWORKS-RUST -- Production Rust 1.75+ Superbrain

> The definitive Rust skill for service and systems programming.
> Pairs with `fireworks-test`, `fireworks-performance`, `fireworks-debug`.

---

## 1. The Rust Protocol

Every Rust task moves through this pipeline. No skipping.

```
DESIGN --> WRITE --> cargo check --> cargo clippy -D warnings --> cargo test --> SHIP
```

1. **DESIGN** -- Sketch types first. Rust rewards thinking about ownership before writing code.
2. **WRITE** -- Idiomatic Rust: `Result` for fallible, `Option` for optional, no `unwrap` on non-trivial paths.
3. **cargo check** -- Zero errors. The compiler is your fastest reviewer.
4. **cargo clippy --all-targets -- -D warnings** -- Zero clippy findings. Treat warnings as errors.
5. **cargo test --workspace** -- All tests pass. New code has tests.
6. **SHIP** -- Only after every gate is green.

### Pre-Flight Checklist

- [ ] Does every `Result` have a real error path? Not just `?` everywhere.
- [ ] Is every `clone()` necessary, or is there a borrow that works?
- [ ] Is the error type `thiserror` (library) or `anyhow` (binary)?
- [ ] Are async trait methods boxed (`Box<dyn Future>`) or using `async fn in trait`?
- [ ] Is every `unsafe` block justified in a comment explaining the invariants?
- [ ] Are workspace dependencies in the root `Cargo.toml`, inherited via `workspace = true`?

### Self-Reference

Mneme itself is Rust. The workspace has 12 crates (`common`, `store`, `supervisor`, `parsers`, `scanners`, `brain`, `livebus`, `multimodal-bridge`, `cli`, `md-ingest`, `benchmarks`) plus a Tauri app. When in doubt about workspace layout, read `Cargo.toml` at the repo root. The patterns documented here are what mneme itself uses in production.

---

## 2. The Borrow Checker Mental Model

Ownership is the foundation. Three rules that are not negotiable:

1. Every value has exactly one owner at any moment.
2. When the owner goes out of scope, the value is dropped.
3. You can have either one mutable reference or any number of shared references, but not both at the same time.

### Shared `&T` vs Exclusive `&mut T`

```rust
let mut v = vec![1, 2, 3];

// Shared borrows: multiple readers, no writers
let r1 = &v;
let r2 = &v;
println!("{} {}", r1[0], r2[0]); // OK

// Exclusive borrow: single writer, no readers
let m = &mut v;
m.push(4); // OK

// Mixing is not allowed
let r = &v;
let m = &mut v;  // ERROR: cannot borrow `v` as mutable, already borrowed as immutable
```

The compiler reasons in terms of scope: references live from where they're taken to the last use. This is called Non-Lexical Lifetimes (NLL).

```rust
let mut v = vec![1, 2, 3];
let r = &v;
println!("{}", r[0]);       // last use of r
let m = &mut v;             // OK -- r is no longer live
m.push(4);
```

### Ownership Transfer (Move)

```rust
let s1 = String::from("hello");
let s2 = s1;                 // ownership moves to s2
// println!("{}", s1);        // ERROR: s1 is moved

// Copy types don't move; they copy
let x: i32 = 5;
let y = x;                   // both valid
println!("{} {}", x, y);     // OK
```

`i32`, `bool`, `char`, `f64`, tuples of `Copy` types -- all implement `Copy`. Everything else (String, Vec, Box) moves.

### When You Need Explicit Lifetimes

The compiler elides lifetimes in most cases. You write them only when the elision rules don't cover your case.

```rust
// Lifetime needed -- which input does the output reference?
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}

// Lifetime elided -- one input, one output
fn first_word(s: &str) -> &str {
    s.split(' ').next().unwrap_or("")
}

// Struct holding a reference -- lifetime required
struct Parser<'a> {
    source: &'a str,
    position: usize,
}
```

Rule of thumb: if the compiler asks for a lifetime, ask first whether you can refactor to own the data (take `String`) or return a new value. Explicit lifetimes often indicate a design that could be simpler.

> Deep dive: [references/borrow-checker.md](references/borrow-checker.md)

---

## 3. Async Rust: Tokio and the Send+Sync Bounds

Async Rust is compile-time scheduled. `async fn` returns an anonymous `impl Future`, which does nothing until awaited.

### Basic Async

```rust
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let a = task_a();     // creates a future; does not run
    let b = task_b();
    let (_, _) = tokio::join!(a, b);  // runs both concurrently
}

async fn task_a() {
    sleep(Duration::from_secs(1)).await;
    println!("a done");
}
```

### Send + Sync Bounds

Tokio's default runtime is multi-threaded. Futures that cross `.await` on this runtime must be `Send`. Data shared across threads must be `Sync`.

```rust
use std::rc::Rc;

// Rc is !Send -- this fails
async fn bad() {
    let x = Rc::new(42);
    some_await().await;
    println!("{}", x);
}

// Arc is Send + Sync -- this works
async fn good() {
    let x = std::sync::Arc::new(42);
    some_await().await;
    println!("{}", x);
}
```

If you see `error[E0277]: Rc<...> cannot be sent between threads safely`, swap to `Arc`.

### Structured Concurrency with JoinSet

```rust
use tokio::task::JoinSet;

async fn fetch_all(urls: Vec<String>) -> Result<Vec<Bytes>, Error> {
    let mut set = JoinSet::new();
    for url in urls {
        set.spawn(async move { fetch(url).await });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        let bytes = res??; // outer ? for JoinError, inner for fetch error
        results.push(bytes);
    }
    Ok(results)
}
```

`JoinSet` is the Rust equivalent of `asyncio.TaskGroup` or Go's `errgroup`. Tasks are tracked; you join them all.

### Cancellation Is Async-Aware

A future dropped mid-await stops immediately. This is the cancellation-can-happen-anywhere rule -- any `.await` point is a potential cancellation point. Design with this in mind:

```rust
async fn save_then_notify(db: &Db, n: &Notifier) -> Result<(), Error> {
    db.save(&record).await?;      // (1) if dropped after this, record is saved but no notify
    n.send("saved").await?;        // (2) if dropped after this, notify happened but outer caller doesn't know
    Ok(())
}
```

If you need atomicity across await points, use a transaction or a compensating action.

> Deep dive: [references/async-rust.md](references/async-rust.md)

---

## 4. Traits: Design and Dyn vs Impl

Traits are Rust's abstraction mechanism. They come in two flavours at the call site: static dispatch (`impl Trait`) and dynamic dispatch (`dyn Trait`).

### Static Dispatch (impl Trait)

```rust
pub trait OrderLoader {
    fn load(&self, id: &str) -> Result<Order, LoadError>;
}

fn process(loader: impl OrderLoader, id: &str) {
    let order = loader.load(id).unwrap();
}
```

Each call to `process` with a different concrete type generates its own monomorphised code. Zero overhead at runtime; larger binary.

### Dynamic Dispatch (dyn Trait)

```rust
fn process(loader: &dyn OrderLoader, id: &str) {
    let order = loader.load(id).unwrap();
}

fn choose() -> Box<dyn OrderLoader> {
    if some_condition() {
        Box::new(PostgresLoader::new())
    } else {
        Box::new(MockLoader::new())
    }
}
```

A single compiled function handles all concrete types via a vtable. Binary is smaller, but there's a pointer indirection at call time.

### Pick One

| Use Case | Pick |
|----------|------|
| Called many times in a hot loop | `impl Trait` |
| Heterogeneous collection | `Box<dyn Trait>` |
| Dependency injection in long-lived struct | `dyn Trait` |
| Return from factory function | `Box<dyn Trait>` |

### Object Safety

Not all traits can be `dyn`. Object-safe rules (simplified):

- No generic methods (generic parameters on methods, not on the trait itself).
- No `Self: Sized` requirements.
- All methods take `&self`, `&mut self`, or `Box<Self>` (not `self` by value, with some exceptions).

```rust
trait Clonable {
    fn clone_it(&self) -> Self;  // NOT object safe -- returns Self
}

trait Logger {
    fn log(&self, msg: &str);    // object safe
}
```

If you need both behaviours, split into two traits.

### From / Into / TryFrom

```rust
impl From<&str> for OrderID {
    fn from(s: &str) -> Self {
        OrderID(s.to_string())
    }
}

// From<T> for U automatically gives you Into<U> for T
let id: OrderID = "abc".into();

// For fallible conversions, TryFrom
impl TryFrom<i64> for Tier {
    type Error = InvalidTier;
    fn try_from(n: i64) -> Result<Self, InvalidTier> {
        match n {
            0 => Ok(Tier::Free),
            1 => Ok(Tier::Pro),
            _ => Err(InvalidTier(n)),
        }
    }
}
```

Every boundary conversion should use `From`/`Into`/`TryFrom`. It composes with `?`:

```rust
fn parse(raw: &str) -> Result<Order, ParseError> {
    let bytes: Vec<u8> = raw.bytes().collect();
    let parsed: RawOrder = serde_json::from_slice(&bytes)?;  // uses From<serde_json::Error>
    Ok(parsed.into())
}
```

> Deep dive: [references/traits-design.md](references/traits-design.md)

---

## 5. Error Handling: thiserror and anyhow

Rust has no exceptions. Errors are values (`Result<T, E>`). The question is what `E` is.

### Libraries: thiserror

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("order {0} not found")]
    NotFound(String),

    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("invalid data: {0}")]
    Invalid(String),
}
```

Library users need to match on specific errors. Give them a typed enum.

- `#[from]` generates `impl From<sqlx::Error> for LoadError`, which enables `?`.
- `#[error("...")]` implements `Display` with a formatted message.
- `#[source]` (or `#[from]`) implements `Error::source` for chain walking.

### Binaries: anyhow

```rust
use anyhow::{Result, Context};

fn load_config() -> Result<Config> {
    let raw = std::fs::read_to_string("config.toml")
        .context("reading config.toml")?;
    let cfg: Config = toml::from_str(&raw)
        .context("parsing config.toml")?;
    Ok(cfg)
}
```

At the top of a program, you just want to propagate and log. `anyhow::Error` is an opaque error type that wraps anything `std::error::Error + Send + Sync`, with a useful `Display` and a chain for debugging.

- `.context(...)` adds a human-readable layer to the error chain.
- `anyhow::Result<T>` is shorthand for `Result<T, anyhow::Error>`.
- Not suitable for libraries -- callers cannot match on specific variants.

### Rule of Thumb

| You are writing | Use |
|-----------------|-----|
| A library crate | thiserror |
| A binary crate (CLI, service) | anyhow at the top, thiserror for internal libraries |
| A single-file script | anyhow |
| A workspace with mixed crates | Each library uses thiserror; the binary uses anyhow |

### The ? Operator

```rust
fn load(path: &str) -> Result<Config, LoadError> {
    let raw = std::fs::read_to_string(path)?;   // std::io::Error -> LoadError via From
    let cfg = toml::from_str(&raw)?;             // toml::de::Error -> LoadError via From
    Ok(cfg)
}
```

`?` is syntactic sugar for:

```rust
match expr {
    Ok(v) => v,
    Err(e) => return Err(e.into()),
}
```

`.into()` uses the `From` impl. If the types don't line up, add an `#[from]` variant or `impl From`.

> Deep dive: [references/errors.md](references/errors.md)

---

## 6. Cargo Workspaces

For non-trivial projects, use a workspace. Mneme is a workspace of 12 crates, a pattern worth studying.

### Root Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "common",
    "store",
    "supervisor",
    "cli",
]

[workspace.package]
version = "0.3.0"
edition = "2021"
rust-version = "1.78"
license = "Apache-2.0"
authors = ["Your Name"]

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1", features = ["derive"] }
thiserror = "1.0"
anyhow = "1.0"

# Internal crates with workspace paths
mypackage-common = { path = "common", version = "0.3.0" }

[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

### Child Crate Inherits

```toml
# common/Cargo.toml
[package]
name = "mypackage-common"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }
```

`workspace = true` means "use the version specified in the root". Every crate stays in sync; bumping a dependency happens in one place.

### Release Profile Tuning

```toml
[profile.release]
lto = "fat"              # aggressive link-time optimisation
codegen-units = 1        # maximum inlining (slow build, fast binary)
panic = "abort"          # no unwinding machinery in the binary
strip = "symbols"        # smaller binary
```

Trade-off: release builds are slower to compile but produce smaller, faster binaries. For a CLI or service, the right trade.

For libraries published to crates.io, don't set `panic = "abort"` in the library's Cargo.toml; leave that to the binary crate.

> Deep dive: [references/cargo-workspaces.md](references/cargo-workspaces.md)

---

## 7. Testing and Benchmarks

### Unit Tests

```rust
// src/orders.rs
pub fn parse_id(s: &str) -> Option<String> {
    s.strip_prefix("ord_").map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_id() {
        assert_eq!(parse_id("ord_abc"), Some("abc".to_string()));
    }

    #[test]
    fn rejects_missing_prefix() {
        assert_eq!(parse_id("abc"), None);
    }
}
```

`#[cfg(test)]` omits the module from non-test builds. Tests live next to the code they test.

### Integration Tests

```
mycrate/
  src/
    lib.rs
  tests/
    integration.rs
```

`tests/` files are compiled as separate crates that import your library via its public API. Each `.rs` is its own binary, so tests across files don't share state.

### Async Tests

```rust
#[tokio::test]
async fn submits_order() {
    let svc = OrderService::new_test().await;
    let result = svc.submit("ord_1").await.unwrap();
    assert_eq!(result.status, Status::Pending);
}
```

### Criterion Benchmarks

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "parser"
harness = false
```

```rust
// benches/parser.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_parse(c: &mut Criterion) {
    c.bench_function("parse order id", |b| {
        b.iter(|| mycrate::parse_id(criterion::black_box("ord_abc")))
    });
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
```

```bash
cargo bench
```

Criterion produces statistical analysis (mean, median, std dev, outlier detection) and can generate HTML reports. Much more sophisticated than the built-in `#[bench]`.

### cargo test --workspace

Runs every test in every crate. The default CI gate.

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Clippy and Fmt in CI

Never skip them. `rustfmt` is the canonical formatter (no debates). Clippy catches real bugs and enforces idioms. `-D warnings` treats every clippy lint as an error in CI.

---

## 8. Common Types and Patterns

### Option

```rust
let x: Option<i32> = Some(5);
let y: Option<i32> = None;

// Pattern matching
match x {
    Some(n) => println!("{}", n),
    None => println!("nothing"),
}

// Combinators
let double: Option<i32> = x.map(|n| n * 2);
let or_default = y.unwrap_or(0);
let or_else = y.unwrap_or_else(|| compute_default());
let chained = x.and_then(|n| lookup(n));

// ? operator
fn get_first_name(user: Option<&User>) -> Option<&str> {
    Some(user?.name.as_str())
}
```

Never use `.unwrap()` or `.expect(...)` on user-derived data. Only use them when you have static proof the value is `Some` (e.g., constructed inline).

### Result

```rust
let r: Result<i32, String> = Ok(5);

match r {
    Ok(n) => println!("{}", n),
    Err(e) => println!("{}", e),
}

// Combinators
let doubled: Result<i32, String> = r.map(|n| n * 2);
let mapped_err = r.map_err(|e| format!("failed: {}", e));
let default = r.unwrap_or(0);
let chained = r.and_then(|n| if n > 0 { Ok(n) } else { Err("negative".to_string()) });
```

### Arc, Rc, Box

- `Box<T>` -- owned pointer to heap-allocated T. Single owner.
- `Rc<T>` -- reference counted, single-threaded. Multiple owners, same thread.
- `Arc<T>` -- atomic reference counted. Multiple owners across threads.
- `Arc<Mutex<T>>` -- shared mutable state across threads.
- `Arc<RwLock<T>>` -- many readers, few writers.

```rust
use std::sync::{Arc, Mutex};

let state = Arc::new(Mutex::new(Vec::new()));
let cloned = Arc::clone(&state);
tokio::spawn(async move {
    let mut v = cloned.lock().unwrap();
    v.push(42);
});
```

### Interior Mutability

`Cell<T>` and `RefCell<T>` let you mutate through a `&T`. The borrow check becomes runtime instead of compile-time. `RefCell` panics on violation.

Use sparingly. If you find yourself reaching for `RefCell` often, your design probably wants `&mut` instead.

---

## 9. Unsafe Rust: The Four Superpowers

`unsafe` lets you do four things the compiler normally forbids:

1. Dereference a raw pointer (`*const T`, `*mut T`).
2. Call an unsafe function or method.
3. Access or modify a mutable static variable.
4. Implement an unsafe trait.

```rust
unsafe fn dangerous() {
    // preconditions must hold
}

unsafe {
    let r: *const i32 = &5;
    println!("{}", *r);
}
```

### When to Use Unsafe

- FFI to C libraries.
- Performance-critical paths where you can prove safety by construction.
- Building safe abstractions over hardware, intrinsics, or OS APIs.

### The Unsafe Contract

Every `unsafe` block documents its safety invariants. Example:

```rust
/// # Safety
/// - `ptr` must be non-null and valid for reads of `len * size_of::<T>()`.
/// - The memory must not be mutated during the function's execution.
/// - T must be Copy or the caller must not read the data after this returns.
unsafe fn from_raw_parts<T>(ptr: *const T, len: usize) -> Vec<T> {
    // implementation
}
```

No `unsafe` without a comment. That is a hard rule.

### Miri: Undefined Behaviour Detector

```bash
rustup +nightly component add miri
cargo +nightly miri test
```

Miri interprets your code at the MIR level and catches undefined behaviour: use-after-free, invalid pointer reads, data races, uninitialised memory, out-of-bounds access. Run it on any code that uses `unsafe`.

> Deep dive: [references/unsafe-and-miri.md](references/unsafe-and-miri.md)

---

## 10. Wrong vs Right -- Quick Reference

| Anti-Pattern | Why It's Wrong | Correct Pattern |
|--------------|----------------|-----------------|
| `.unwrap()` on Result from I/O | Panics on error | `?` or explicit handling |
| `.clone()` to dodge borrow checker | Unnecessary allocation | Restructure to borrow |
| `String` everywhere for function args | Forces allocation | `&str` where possible |
| `Arc<Mutex<T>>` on read-heavy data | Contention | `Arc<RwLock<T>>` or snapshot pattern |
| `async fn` returning `!Send` futures on tokio | Runtime error on spawn | Swap `Rc` for `Arc`, `RefCell` for `Mutex` |
| `panic!` for recoverable errors | Crashes the process | `Result<_, E>` |
| `println!` for logging | No level, no structure | `tracing` with spans and fields |
| Shared `&mut` across threads | Undefined behaviour | `Arc<Mutex<_>>` |
| `std::thread::sleep` in async | Blocks the runtime | `tokio::time::sleep` |
| `block_on` inside an async context | Deadlocks | `spawn_blocking` or refactor |
| Nested locks without order | Deadlock | Define a canonical lock order |
| Public struct with all pub fields | Leaks implementation | Private fields + getters |

---

## 11. Iron Law

```
NO RUST CODE WITHOUT EXPLICIT OWNERSHIP AND ERROR PROPAGATION.

Every Result has a real error path, not just ?.
Every clone is justified.
Every unsafe is documented.
Every async fn returning !Send is an intentional choice.
Every workspace dependency is in the root Cargo.toml.
cargo clippy -D warnings passes.
```

---

## 12. Compound Skill Chaining

| Chain To | When | What It Adds |
|----------|------|--------------|
| `fireworks-test` | After implementation | cargo test patterns, proptest, criterion |
| `fireworks-performance` | When optimising | cargo flamegraph, perf, allocation tracking |
| `fireworks-debug` | On crash or hang | rust-gdb, println-debug, log-tracing |
| `fireworks-security` | On HTTP/auth code | Secret handling, input validation, unsafe audit |
| `fireworks-architect` | New service design | Actor patterns, hexagonal, event sourcing |

---

## 13. Reference Files Index

| File | Coverage |
|------|----------|
| [references/full-guide.md](references/full-guide.md) | Overview, project layout, common patterns |
| [references/borrow-checker.md](references/borrow-checker.md) | Ownership, borrowing, lifetimes, NLL, PhantomData |
| [references/async-rust.md](references/async-rust.md) | Futures, Pin, Send+Sync, JoinSet, cancellation |
| [references/traits-design.md](references/traits-design.md) | Dyn vs impl, object safety, blanket impls, marker traits |
| [references/errors.md](references/errors.md) | thiserror deep dive, anyhow context, From chains |
| [references/cargo-workspaces.md](references/cargo-workspaces.md) | Workspace structure, profile tuning, feature flags |
| [references/unsafe-and-miri.md](references/unsafe-and-miri.md) | The four superpowers, invariants, Miri workflow |

---

## 14. Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
