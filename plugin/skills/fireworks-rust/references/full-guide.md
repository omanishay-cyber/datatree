# fireworks-rust -- Full Guide

> The deep reference for Rust 1.75+ development.
> Loaded on demand from `SKILL.md`.

---

## 1. Project Layout

### Single Crate

```
mycrate/
  Cargo.toml
  Cargo.lock        # commit for binaries, usually gitignore for libraries
  src/
    lib.rs           # library crate root
    main.rs          # binary crate root
    bin/
      tool.rs        # additional binaries
    utils/
      mod.rs         # module root
      helpers.rs
  tests/
    integration.rs   # integration test
  benches/
    parser.rs        # criterion benchmark
  examples/
    basic.rs
```

A single package can be a library (`src/lib.rs`), a binary (`src/main.rs`), or both. Additional binaries go under `src/bin/`.

### Workspace

For anything with multiple crates:

```
myproject/
  Cargo.toml        # workspace root (no [package], has [workspace])
  Cargo.lock
  common/
    Cargo.toml
    src/lib.rs
  store/
    Cargo.toml
    src/lib.rs
  cli/
    Cargo.toml
    src/main.rs
  target/           # build output, gitignored
```

Mneme's workspace has `common`, `store`, `supervisor`, `parsers`, `scanners`, `brain`, `livebus`, `multimodal-bridge`, `cli`, `md-ingest`, `benchmarks`. Study its Cargo.toml for a real-world example of workspace dependency inheritance.

### Module System

```rust
// src/lib.rs
pub mod orders;          // either orders.rs or orders/mod.rs
pub mod payments {
    pub mod refund;      // payments/refund.rs
    mod private;         // not exported outside the crate
}

// src/orders.rs
pub struct Order { ... }
pub fn submit(o: Order) -> Result<(), Error> { ... }
```

Modules form a tree. `pub` controls visibility; `pub(crate)` limits to the current crate; `pub(super)` to the parent module.

---

## 2. Standard Types You Use Every Day

### String vs &str

- `String` owns its contents, allocated on the heap, growable.
- `&str` is a borrowed view into a string. Could point into a `String`, a string literal, or anywhere.

```rust
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

let owned: String = String::from("world");
let borrowed: &str = &owned;
let literal: &'static str = "hard-coded";

greet(&owned);       // OK
greet(borrowed);     // OK
greet(literal);      // OK
```

Rule: take `&str` as input, return `String` (or `&str` borrowed from input) as output. This is the Rust equivalent of Go's "accept interfaces, return structs".

### Vec<T>

```rust
let mut v: Vec<i32> = Vec::new();
v.push(1);
v.push(2);

let v2: Vec<i32> = vec![1, 2, 3];
let v3: Vec<i32> = (0..10).collect();
let doubled: Vec<i32> = v3.iter().map(|x| x * 2).collect();

// Pre-allocate when size is known
let mut v4: Vec<i32> = Vec::with_capacity(1000);
```

`Vec` is the default growable array. Its slice view is `&[T]` -- take that as input.

```rust
fn sum(values: &[i32]) -> i32 {
    values.iter().sum()
}
```

### HashMap, BTreeMap

```rust
use std::collections::HashMap;

let mut m: HashMap<String, i32> = HashMap::new();
m.insert("key".to_string(), 42);

if let Some(v) = m.get("key") {
    println!("{}", v);
}

// Entry API for get-or-insert patterns
*m.entry("counter".to_string()).or_insert(0) += 1;
```

`HashMap` is faster for lookup; `BTreeMap` keeps keys sorted. Default to `HashMap` unless you need ordering.

### Option

```rust
let x: Option<i32> = Some(5);

// Pattern matching
match x {
    Some(n) => println!("{}", n),
    None => println!("nothing"),
}

// if let
if let Some(n) = x {
    println!("{}", n);
}

// Combinators
let double = x.map(|n| n * 2);
let default = x.unwrap_or(0);
let computed = x.unwrap_or_else(|| expensive_compute());
let chained = x.and_then(|n| if n > 0 { Some(n) } else { None });
```

### Result

```rust
let r: Result<i32, String> = Ok(5);

match r {
    Ok(n) => println!("{}", n),
    Err(e) => eprintln!("{}", e),
}

// ? operator propagates
fn compute() -> Result<i32, String> {
    let a = parse_int("5")?;
    let b = parse_int("10")?;
    Ok(a + b)
}
```

---

## 3. Iterators and Closures

Iterators are lazy; nothing happens until consumed.

```rust
let v = vec![1, 2, 3, 4, 5];

// Lazy -- no work done
let mapped = v.iter().map(|x| x * 2);

// Consumes iterator -- now it runs
let doubled: Vec<i32> = mapped.collect();
```

### Common Iterator Methods

```rust
// Transform
iter.map(|x| x * 2)
iter.filter(|x| x > &0)
iter.filter_map(|x| x.checked_mul(2))

// Terminate
iter.collect::<Vec<_>>()
iter.sum::<i32>()
iter.count()
iter.max()
iter.min()
iter.find(|x| x > &10)
iter.any(|x| x > 10)
iter.all(|x| x > 0)

// Reduce
iter.fold(0, |acc, x| acc + x)
iter.reduce(|a, b| a + b)

// Zip / enumerate / chain
iter1.zip(iter2)
iter.enumerate()
iter1.chain(iter2)
```

### Closures

```rust
let add = |a, b| a + b;
let increment = |x| x + 1;

// Captures by reference
let prefix = String::from("Hello, ");
let greet = |name: &str| format!("{}{}", prefix, name);

// Move captures (takes ownership)
let owned = String::from("owned");
let consume = move |suffix: &str| format!("{}{}", owned, suffix);
// `owned` is no longer accessible here -- moved into the closure
```

Closures implement `Fn`, `FnMut`, or `FnOnce` depending on how they use captures:

- `Fn` -- captures by shared reference. Can be called any number of times concurrently.
- `FnMut` -- captures by mutable reference. Single caller at a time.
- `FnOnce` -- consumes captures. Can be called once.

---

## 4. Macros

### Declarative Macros (macro_rules!)

```rust
macro_rules! vec2 {
    () => (Vec::new());
    ($($x:expr),+ $(,)?) => ({
        let mut v = Vec::new();
        $(v.push($x);)+
        v
    });
}

let v = vec2![1, 2, 3];
```

Declarative macros are pattern-based. Useful for repetitive code patterns (logging helpers, test fixtures) where a function wouldn't do.

### Procedural Macros (derive, attribute, function)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Order {
    id: String,
    total_cents: i64,
}
```

`#[derive(...)]` runs compiler plugins that generate impl blocks. The standard derives (`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Default`) cover most needs. `Serialize` and `Deserialize` come from serde.

Writing your own procedural macros is a separate crate kind; see the `syn` and `quote` crates.

### Common Macro Idioms

```rust
println!("{}", x);         // formatted print
eprintln!("{}", e);        // to stderr
format!("{}-{}", a, b);    // format to String
write!(f, "{}", x);        // write to any Write (std::fmt::Formatter)
dbg!(expr);                 // debug print and return expr
todo!()                     // unimplemented, compiles
unimplemented!()            // same but explicit
unreachable!()              // panics if reached
assert!(cond);              // panic if false
assert_eq!(a, b);           // panic with helpful message
```

---

## 5. Serde: Serialization

Serde is the universal serialisation framework.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    #[serde(rename = "totalCents")]
    pub total_cents: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

// To/from JSON
let json = serde_json::to_string(&order)?;
let parsed: Order = serde_json::from_str(&json)?;

// To/from TOML, YAML, CBOR, etc. -- each is a separate crate
```

### Useful Attributes

- `#[serde(rename = "...")]` -- rename field in serialised form.
- `#[serde(default)]` -- use Default::default() if missing on deserialize.
- `#[serde(skip_serializing_if = "...")]` -- omit field on serialize.
- `#[serde(flatten)]` -- inline a struct's fields into the parent.
- `#[serde(tag = "type")]` on enums -- internally tagged JSON.
- `#[serde(rename_all = "camelCase")]` on struct -- rename all fields.

### serde_json Common Patterns

```rust
// Parse into Value for dynamic handling
let v: serde_json::Value = serde_json::from_str(raw)?;
let name = v["user"]["name"].as_str().unwrap_or("");

// Build Value inline
let v = serde_json::json!({
    "id": 1,
    "name": "alice",
    "tags": ["a", "b"],
});
```

---

## 6. HTTP Frameworks

### axum (Tokio-native)

```rust
use axum::{
    routing::{get, post},
    Router, Json, extract::Path,
};

async fn get_order(Path(id): Path<String>) -> Json<Order> {
    let o = load(&id).await.unwrap();
    Json(o)
}

async fn create_order(Json(req): Json<CreateOrder>) -> Json<Order> {
    let o = create(req).await.unwrap();
    Json(o)
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/orders/:id", get(get_order))
        .route("/orders", post(create_order));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

axum is the current default. Built on tokio + hyper + tower.

### actix-web

```rust
use actix_web::{web, App, HttpServer, Responder};

async fn get_order(path: web::Path<String>) -> impl Responder {
    format!("order: {}", path)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().route("/orders/{id}", web::get().to(get_order))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}
```

actix-web is older, very mature, very fast. Uses its own actor runtime on top of tokio.

### Pick axum for new projects unless you have specific reasons otherwise.

---

## 7. Database: sqlx and rusqlite

### sqlx (async, compile-time checked)

```rust
use sqlx::postgres::PgPool;

#[derive(sqlx::FromRow)]
struct Order {
    id: String,
    total_cents: i64,
}

async fn load(pool: &PgPool, id: &str) -> Result<Order, sqlx::Error> {
    sqlx::query_as!(
        Order,
        "SELECT id, total_cents FROM orders WHERE id = $1",
        id,
    )
    .fetch_one(pool)
    .await
}
```

`query_as!` checks the SQL against the live schema at compile time. If the schema changes, the code stops compiling.

### rusqlite (sync, SQLite-specific)

```rust
use rusqlite::{Connection, params};

fn load(conn: &Connection, id: &str) -> Result<Order, rusqlite::Error> {
    conn.query_row(
        "SELECT id, total_cents FROM orders WHERE id = ?1",
        params![id],
        |row| Ok(Order {
            id: row.get(0)?,
            total_cents: row.get(1)?,
        }),
    )
}
```

Mneme uses rusqlite extensively for SQLite access. Sync, simple, battle-tested.

---

## 8. Concurrency Primitives

### std::sync

```rust
use std::sync::{Arc, Mutex, RwLock};

let state = Arc::new(Mutex::new(Vec::new()));

// Thread A
let a = Arc::clone(&state);
std::thread::spawn(move || {
    let mut v = a.lock().unwrap();
    v.push(1);
});

// Thread B
let b = Arc::clone(&state);
std::thread::spawn(move || {
    let v = b.lock().unwrap();
    println!("{:?}", *v);
});
```

`Mutex::lock()` returns a `Result<MutexGuard, PoisonError>`. Poison means a previous holder panicked. In production, you usually just `.unwrap()` -- a poisoned mutex is a bug to fix.

### tokio::sync (async-aware)

```rust
use tokio::sync::Mutex as TokioMutex;
use std::sync::Arc;

let state = Arc::new(TokioMutex::new(Vec::new()));

let a = Arc::clone(&state);
tokio::spawn(async move {
    let mut v = a.lock().await;
    v.push(1);
});
```

The async versions (`tokio::sync::Mutex`, `RwLock`) yield to the runtime instead of blocking. Critical for async code -- `std::sync::Mutex` inside async code blocks the whole runtime thread.

Rule: async code uses `tokio::sync::*`. Sync code uses `std::sync::*`. Don't mix.

### Channels

```rust
// Tokio unbounded MPSC
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
        println!("{}", msg);
    }
});

tx.send("hello".to_string()).unwrap();

// Bounded
let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
tx.send("hello".to_string()).await.unwrap();  // .await backpressures when full

// Broadcast: 1 producer, many consumers
let (tx, _rx) = tokio::sync::broadcast::channel::<String>(16);

// Watch: 1 producer, many consumers, only latest value
let (tx, rx) = tokio::sync::watch::channel::<i32>(0);

// Oneshot: single value, one-time
let (tx, rx) = tokio::sync::oneshot::channel::<String>();
```

---

## 9. clap: CLI Parsing

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mycli", version)]
struct Cli {
    #[arg(short, long, env = "LOG_LEVEL", default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve HTTP API
    Serve {
        #[arg(short, long, default_value = "0.0.0.0:3000")]
        bind: String,
    },
    /// Run database migrations
    Migrate,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { bind } => { serve(&bind); }
        Command::Migrate => { migrate(); }
    }
}
```

clap is the de-facto CLI framework. The derive form (shown) is the modern idiom.

---

## 10. tracing: Structured Logging

```rust
use tracing::{info, error, instrument, Level};
use tracing_subscriber::{EnvFilter, fmt};

fn init_tracing() {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();
}

#[instrument(skip(db))]
async fn load_order(db: &Db, id: &str) -> Result<Order, Error> {
    info!("loading order");
    let o = db.load(id).await?;
    info!(order_id = %o.id, total = o.total_cents, "loaded");
    Ok(o)
}
```

- `#[instrument]` wraps the function body in a tracing span.
- `skip(db)` excludes noisy fields from the span.
- `info!(field = value, "message")` emits a structured event.
- `%x` uses `Display`; `?x` uses `Debug`.

For production, use `tracing-subscriber` with the JSON formatter and configure via `RUST_LOG`.

```bash
RUST_LOG=info,mycrate=debug ./myservice
```

---

## 11. Environment and Config

```rust
use std::env;

let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
```

For structured config, use `config` or `envy`:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    database_url: String,
    log_level: String,
    #[serde(default = "default_port")]
    port: u16,
}

fn default_port() -> u16 { 3000 }

let cfg: Config = envy::prefixed("APP_").from_env().unwrap();
```

---

## 12. Deps: Common Crates You'll Reach For

| Crate | Use |
|-------|-----|
| tokio | async runtime |
| serde | serialisation |
| serde_json | JSON |
| reqwest | async HTTP client |
| axum | HTTP server |
| sqlx | async DB, compile-time checked |
| rusqlite | sync SQLite |
| clap | CLI parsing |
| thiserror | library error types |
| anyhow | binary error handling |
| tracing | structured logging |
| tracing-subscriber | log output config |
| chrono / time | date and time |
| uuid | UUID generation |
| regex | regular expressions |
| once_cell | lazy statics |
| parking_lot | faster Mutex/RwLock |
| dashmap | concurrent HashMap |
| rayon | data parallelism |
| criterion | benchmarking |
| proptest / quickcheck | property-based testing |
| mockall | mocking for tests |

---

## 13. Cross-References

- [borrow-checker.md](borrow-checker.md) -- ownership in depth
- [async-rust.md](async-rust.md) -- Pin, Send+Sync, structured concurrency
- [traits-design.md](traits-design.md) -- trait objects, object safety
- [errors.md](errors.md) -- thiserror, anyhow, From chains
- [cargo-workspaces.md](cargo-workspaces.md) -- workspace layout and profiles
- [unsafe-and-miri.md](unsafe-and-miri.md) -- unsafe code and verification
