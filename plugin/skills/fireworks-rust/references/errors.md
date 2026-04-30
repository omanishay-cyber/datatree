# fireworks-rust -- Error Handling

> thiserror, anyhow, From chains, Error::source.
> The Rust approach: errors are values, types carry context, and ? propagates.

---

## 1. Result and Option

Rust has no exceptions. Fallibility is expressed in the type system:

```rust
fn parse(s: &str) -> Result<i32, std::num::ParseIntError> { s.parse() }
fn first(v: &[i32]) -> Option<i32> { v.first().copied() }
```

- `Result<T, E>` -- either a value or an error.
- `Option<T>` -- either a value or nothing.

You cannot accidentally forget to handle an error. The compiler will complain about unused Results.

---

## 2. The ? Operator

`?` is sugar for match + early return.

```rust
fn load_config(path: &str) -> Result<Config, ConfigError> {
    let raw = std::fs::read_to_string(path)?;           // std::io::Error -> ConfigError
    let cfg: Config = toml::from_str(&raw)?;             // toml::de::Error -> ConfigError
    Ok(cfg)
}
```

Desugars to:

```rust
fn load_config(path: &str) -> Result<Config, ConfigError> {
    let raw = match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => return Err(e.into()),
    };
    let cfg: Config = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return Err(e.into()),
    };
    Ok(cfg)
}
```

`e.into()` uses the `From` impl to convert the source error into the function's error type. If the conversion doesn't exist, you get a compile error -- the type system makes error propagation explicit.

### ? on Option

```rust
fn first_char(s: Option<&str>) -> Option<char> {
    let c = s?.chars().next()?;
    Some(c)
}
```

Works the same way: returns `None` early on `None`.

### Mixing Result and Option

`?` doesn't automatically convert between them. Use `.ok_or`/`.ok_or_else` or `.ok()`:

```rust
fn get_first(s: Option<&str>) -> Result<char, MyError> {
    s.ok_or(MyError::Missing)?
        .chars()
        .next()
        .ok_or(MyError::Empty)
}
```

---

## 3. thiserror (Library Error Types)

For library code, define typed errors so callers can match on specific variants.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("order {0} not found")]
    NotFound(String),

    #[error("invalid order data: {reason}")]
    Invalid { reason: String },

    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("unexpected state")]
    Unexpected(#[source] Box<dyn std::error::Error + Send + Sync>),
}
```

### What thiserror Generates

- `impl std::error::Error for LoadError` -- the Error trait with source chains.
- `impl std::fmt::Display for LoadError` -- formatted messages per variant.
- `impl From<sqlx::Error> for LoadError` -- via `#[from]`.
- `impl From<std::io::Error> for LoadError` -- via `#[from]`.

### Key Attributes

```rust
#[error("text {field}")]      // Display format with struct-style fields
#[error("text {0}")]           // Display format with tuple-style fields
#[error(transparent)]          // Display passes through to inner error
#[from]                        // auto-generate From impl
#[source]                      // mark field as the error source without From
```

### transparent

```rust
#[derive(Error, Debug)]
pub enum MyError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

`transparent` is useful when wrapping an opaque error -- the display and source both pass through without adding decoration.

### Source Chains

```rust
fn load() -> Result<Config, LoadError> { ... }

if let Err(e) = load() {
    eprintln!("{}", e);                        // top-level message
    let mut source = e.source();
    while let Some(s) = source {
        eprintln!("  caused by: {}", s);
        source = s.source();
    }
}
```

Every error in the chain is printed. Critical for debugging layered errors.

---

## 4. anyhow (Binary Error Handling)

For binaries, you just need to propagate and log.

```rust
use anyhow::{Context, Result, bail, ensure};

fn main() -> Result<()> {
    let cfg = load_config("config.toml")
        .context("loading config at startup")?;

    let server = build_server(&cfg)
        .context("initialising server")?;

    server.run().context("running server")?;
    Ok(())
}

fn load_config(path: &str) -> Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path))?;
    let cfg: Config = toml::from_str(&raw)
        .context("parsing TOML")?;
    Ok(cfg)
}
```

### What anyhow Gives You

- `anyhow::Error` -- wraps any `std::error::Error + Send + Sync + 'static`.
- `anyhow::Result<T>` -- shorthand for `Result<T, anyhow::Error>`.
- `.context(...)` / `.with_context(|| ...)` -- adds a layer to the error chain.
- `bail!("...")` -- short for `return Err(anyhow!("..."))`.
- `ensure!(cond, "...")` -- short for `if !cond { bail!("...") }`.

### context vs with_context

```rust
// Eager -- context string computed always
.context("reading config")

// Lazy -- closure only runs on Err
.with_context(|| format!("reading {}", path))
```

Use `with_context` when the context string requires formatting; `context` for static strings. Saves a few allocations in the happy path.

### Display Chain

Printing an `anyhow::Error` with `{:?}` gives:

```
loading config at startup

Caused by:
    0: reading config.toml
    1: No such file or directory (os error 2)
```

Printing with `{}` gives just the top level. Use `{:?}` in main for debugging clarity.

---

## 5. When to Use Which

| Scenario | Use |
|----------|-----|
| Library crate published to crates.io | thiserror |
| Binary crate (CLI, service) top layer | anyhow |
| Inner modules of a binary | Your own thiserror enums, anyhow at the edge |
| Quick script (no crates.io) | anyhow |
| Workspace with mixed crates | thiserror per library crate, anyhow in the binary |

### Wrapping thiserror in anyhow

```rust
// Library returns thiserror
fn parse_config() -> Result<Config, ConfigError> { ... }

// Binary wraps in anyhow
fn main() -> anyhow::Result<()> {
    let cfg = parse_config()
        .context("parsing config at startup")?;
    // ...
}
```

`anyhow::Error` accepts any `std::error::Error + Send + Sync + 'static`, so thiserror errors convert automatically.

---

## 6. Designing Good Error Enums

### Rule: One Variant Per Recoverable Failure Mode

Callers will match on your error. Give them one arm per meaningful decision they'd make.

```rust
// GOOD -- each variant is actionable
#[derive(Error, Debug)]
pub enum OrderError {
    #[error("order {0} not found")]
    NotFound(String),

    #[error("order {0} already shipped")]
    AlreadyShipped(String),

    #[error("customer {0} lacks funds")]
    InsufficientFunds(String),

    #[error("database error")]
    Db(#[from] sqlx::Error),
}

// Matcher knows what to do with each:
match submit(o) {
    Err(OrderError::NotFound(_)) => return StatusCode::NOT_FOUND,
    Err(OrderError::AlreadyShipped(_)) => return StatusCode::CONFLICT,
    Err(OrderError::InsufficientFunds(_)) => return StatusCode::PAYMENT_REQUIRED,
    Err(OrderError::Db(_)) => return StatusCode::INTERNAL_SERVER_ERROR,
    Ok(_) => return StatusCode::CREATED,
}
```

### Anti-Pattern: One Big Variant

```rust
// BAD
#[derive(Error, Debug)]
pub enum OrderError {
    #[error("error: {0}")]
    General(String),
}
```

This is just `Result<_, String>` with extra steps. The caller has to string-match to branch, which is brittle.

### Rule: Errors Should Travel

Errors cross threads and tasks. Give them `Send + Sync + 'static`:

```rust
#[derive(Error, Debug)]
pub enum OrderError {
    #[error("db error")]
    Db(#[from] sqlx::Error),  // sqlx::Error is Send + Sync
    // ...
}
```

If you need to wrap a trait object error:

```rust
#[derive(Error, Debug)]
pub enum OrderError {
    #[error("custom")]
    Custom(#[source] Box<dyn std::error::Error + Send + Sync>),
}
```

### Rule: Error Messages Are for Humans

```rust
#[error("order {0} not found in {1}")]
NotFound(String, &'static str),

// The Display output: "order abc123 not found in orders table"
```

Not for machine consumption. The variant itself is the machine-readable form.

---

## 7. Error Source and Chain

```rust
use std::error::Error;

let e: Box<dyn Error> = ...;

let mut current = Some(e.as_ref());
while let Some(err) = current {
    eprintln!("  {}", err);
    current = err.source();
}
```

The `Error::source` method walks the chain. thiserror generates it correctly; anyhow handles it automatically.

### Source vs Cause (Legacy)

Old code might use `Error::cause`. It's deprecated. Always implement/use `source`.

---

## 8. Common Patterns

### Optional Conversion via From

```rust
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid input")]
    Invalid,

    #[error("parse failed: {0}")]
    Underlying(#[from] std::num::ParseIntError),
}

fn parse_count(s: &str) -> Result<i32, ParseError> {
    let n: i32 = s.parse()?;  // ParseIntError -> ParseError::Underlying
    if n < 0 {
        return Err(ParseError::Invalid);
    }
    Ok(n)
}
```

### Error Mapping at Boundaries

Sometimes the inner error type shouldn't leak. Map it:

```rust
fn public_api(id: &str) -> Result<Order, PublicError> {
    internal_load(id).map_err(|e| match e {
        InternalError::NotFound(_) => PublicError::NotFound,
        InternalError::Db(_) => PublicError::Unavailable,
        InternalError::Timeout => PublicError::Unavailable,
    })
}
```

Public errors don't need every internal detail. Mapping at the boundary keeps the public API stable.

### Early Returns with bail!

```rust
fn validate(o: &Order) -> anyhow::Result<()> {
    if o.total_cents < 0 {
        anyhow::bail!("total is negative: {}", o.total_cents);
    }
    if o.items.is_empty() {
        anyhow::bail!("order has no items");
    }
    Ok(())
}
```

Cleaner than `return Err(anyhow!("..."))`.

### ensure! for Assertions

```rust
fn transfer(from: &mut Account, to: &mut Account, cents: i64) -> anyhow::Result<()> {
    anyhow::ensure!(cents > 0, "amount must be positive: {}", cents);
    anyhow::ensure!(from.balance >= cents, "insufficient balance");
    from.balance -= cents;
    to.balance += cents;
    Ok(())
}
```

### Converting with .map_err

```rust
fn load() -> Result<Order, LoadError> {
    let data = fetch_bytes()
        .map_err(|e| LoadError::Io(e.to_string()))?;
    ...
}
```

Use when you don't want to add a `From` impl but need one-off conversion.

### Optional ? via ok_or

```rust
fn get_field(v: &Value, field: &str) -> Result<String, ParseError> {
    v[field].as_str()
        .ok_or(ParseError::MissingField(field.to_string()))
        .map(String::from)
}
```

Converts Option to Result for `?` compatibility.

---

## 9. Error Handling in Async

Async error handling works the same way -- `Result`, `?`, thiserror, anyhow -- except now errors cross await points and may span threads.

```rust
async fn fetch(url: &str) -> Result<Bytes, FetchError> {
    let resp = reqwest::get(url).await?;                // reqwest::Error
    if !resp.status().is_success() {
        return Err(FetchError::BadStatus(resp.status()));
    }
    let bytes = resp.bytes().await?;
    Ok(bytes)
}

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("HTTP error")]
    Http(#[from] reqwest::Error),
    #[error("bad status: {0}")]
    BadStatus(reqwest::StatusCode),
}
```

Since errors may cross threads in tokio's multi-threaded runtime, they must be `Send + Sync`. Most thiserror-derived enums are automatically.

### Error Context in Spawned Tasks

```rust
async fn process() -> anyhow::Result<()> {
    let handle = tokio::spawn(async {
        do_work().await.context("do_work inside spawned task")
    });

    handle.await
        .context("joining task")?
        .context("work itself")?;

    Ok(())
}
```

Three layers of error possible: spawn failed, join panicked, work failed. Each deserves context.

---

## 10. Panics vs Errors

### When to Panic

- Invariant violations where continuing corrupts state.
- Bugs that should never happen in correct code (unreachable match arms).
- Initialisation failure where there's no recovery path.

```rust
let config: &Config = CONFIG.get().expect("CONFIG must be initialised before use");
```

### When to Return an Error

- Anything involving external input.
- Anything that could plausibly fail at runtime.
- Library code (libraries should almost never panic).

```rust
let n: i32 = s.parse().map_err(|e| MyError::BadInput(s.to_string(), e))?;
```

### unwrap() Rules

- In main.rs at the top level -- fine, it makes errors visible.
- In tests -- fine.
- In production library code -- almost never. Use `expect(...)` with an explanation if you're certain:

```rust
let lock = MUTEX.lock().expect("mutex poisoned -- previous holder panicked");
```

### expect_err / unwrap_err

For tests asserting a specific error type:

```rust
#[test]
fn rejects_empty_input() {
    let err = parse("").unwrap_err();
    assert!(matches!(err, ParseError::Empty));
}
```

---

## 11. Error Helpers for Production Code

### Logging an Error Before Propagating

```rust
use tracing::error;

fn load() -> anyhow::Result<Config> {
    inner_load().map_err(|e| {
        error!(error = ?e, "failed to load config");
        e
    })
}
```

Prefer logging at the layer where you have context. The top-level HTTP handler logs; inner layers just propagate.

### Converting Errors to HTTP Status

```rust
impl axum::response::IntoResponse for OrderError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            OrderError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            OrderError::AlreadyShipped(_) => (StatusCode::CONFLICT, self.to_string()),
            OrderError::InsufficientFunds(_) => (StatusCode::PAYMENT_REQUIRED, self.to_string()),
            OrderError::Db(_) => {
                tracing::error!(error = ?self, "db error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
        };
        (status, message).into_response()
    }
}
```

Pattern: match on error variant, decide status and whether to leak details. Log internal errors; never reveal them.

---

## 12. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| Using `.unwrap()` on I/O results in production | Return a Result; propagate with ? |
| `Result<_, String>` instead of proper types | Use thiserror enum |
| `Result<_, Box<dyn Error>>` in library API | Define your own error enum |
| Giant one-variant enum with a message field | Split into multiple meaningful variants |
| Mapping every error to String and losing source | Preserve source chain via #[source] or #[from] |
| `panic!` for recoverable errors | Return Err |
| Logging and re-throwing at every layer | Log at the edge only |
| `.expect("TODO")` in committed code | Handle it or delete the call |
| Swallowing errors silently (`let _ = foo()`) | Either log or propagate |
| `io::Result<()>` with manually constructed errors | Use `io::Error::new(kind, msg)` or a custom error type |

---

## 13. Cross-References

- [traits-design.md](traits-design.md) -- From/Into traits for conversion
- [async-rust.md](async-rust.md) -- error propagation across await points
- [full-guide.md](full-guide.md) -- tracing for error logging
- [cargo-workspaces.md](cargo-workspaces.md) -- thiserror in library crates
