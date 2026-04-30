# fireworks-rust -- Async Rust

> Futures, Pin, Send+Sync, JoinSet, cancellation.
> The complete async Rust model with the idioms that actually work.

---

## 1. Futures and the Runtime

A `Future` in Rust is a state machine. `async fn` and `async { }` blocks are sugar that produce anonymous `impl Future` values.

```rust
async fn fetch(url: &str) -> Vec<u8> {
    // compiler generates a state machine here
    let resp = reqwest::get(url).await.unwrap();
    resp.bytes().await.unwrap().to_vec()
}

// Equivalent shape (simplified):
fn fetch(url: &str) -> impl Future<Output = Vec<u8>> {
    async move {
        let resp = reqwest::get(url).await.unwrap();
        resp.bytes().await.unwrap().to_vec()
    }
}
```

Critical property: **futures do nothing until polled**. Creating a future is cheap; running it requires a runtime.

### The Runtime (tokio)

```rust
#[tokio::main]
async fn main() {
    fetch("https://example.com").await;
}
```

`#[tokio::main]` expands to:

```rust
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        fetch("https://example.com").await;
    });
}
```

For library code that doesn't want to impose a runtime on callers, stay runtime-agnostic with traits like `AsyncRead`, `AsyncWrite` from futures or tokio. For applications, commit to tokio.

### spawn vs block_on

```rust
tokio::spawn(async {
    // runs concurrently on the runtime
});

// block_on is synchronous -- blocks the calling thread
runtime.block_on(async { ... });
```

- `spawn` is fire-and-forget. Returns a `JoinHandle` you can await.
- `block_on` is for bridging sync and async. Never call inside an async context -- it deadlocks the current executor thread.

### spawn_blocking

For synchronous work that would otherwise block the async runtime:

```rust
let result = tokio::task::spawn_blocking(move || {
    // CPU-bound or blocking I/O work
    expensive_computation(data)
}).await.unwrap();
```

This runs on a separate blocking thread pool. Use when you have genuinely blocking code you can't convert to async.

---

## 2. Send + Sync Bounds

Tokio's default runtime is multi-threaded. When you `.await` on that runtime, your future may resume on a different thread. So the future must be `Send`: all locals held across an await point must be `Send`.

### The Classic Error

```rust
use std::rc::Rc;

async fn bad() {
    let x = Rc::new(42);
    some_await().await;    // ERROR: Rc is not Send
    println!("{}", x);
}
```

Error message:

```
error[E0277]: `Rc<i32>` cannot be sent between threads safely
```

Fix: use `Arc` instead.

```rust
use std::sync::Arc;

async fn good() {
    let x = Arc::new(42);
    some_await().await;    // OK: Arc is Send + Sync
    println!("{}", x);
}
```

### The Guard-Across-Await Problem

```rust
use std::sync::Mutex;

async fn bad(m: &Mutex<i32>) {
    let guard = m.lock().unwrap();   // MutexGuard is !Send on some platforms
    some_await().await;              // held across await -- may deadlock or fail Send bound
    println!("{}", *guard);
}
```

Fix: don't hold `std::sync::MutexGuard` across await points. Use `tokio::sync::Mutex` or release the lock before awaiting.

```rust
use tokio::sync::Mutex;

async fn good(m: &Mutex<i32>) {
    let guard = m.lock().await;
    some_await().await;              // OK: tokio MutexGuard is Send
    println!("{}", *guard);
}
```

Or release before awaiting:

```rust
async fn also_good(m: &std::sync::Mutex<i32>) {
    let value = {
        let guard = m.lock().unwrap();
        *guard  // copy the value out
    };  // guard dropped here
    some_await().await;
    println!("{}", value);
}
```

### Explicit Single-Threaded (LocalSet)

```rust
let local = tokio::task::LocalSet::new();
local.run_until(async {
    let x = Rc::new(42);  // OK here -- no cross-thread movement
    tokio::task::spawn_local(async move {
        println!("{}", x);
    }).await.unwrap();
}).await;
```

`LocalSet` pins tasks to the current thread. Lets you use `Rc` and `RefCell` in async code. Pay with losing multi-threaded parallelism.

---

## 3. Pin and Self-Referential Futures

`Pin<P>` prevents a value from being moved. It's needed because async state machines can be self-referential.

### The Problem

```rust
async fn example() {
    let s = String::from("hello");
    let r = &s;                // reference into s
    some_await().await;         // suspend -- state machine stores both s and r
    println!("{}", r);
}
```

The state machine at the suspension point contains both `s` and a pointer into `s`. If the state machine were moved, the pointer would dangle.

### The Solution

The runtime places the future in a fixed memory location (on the heap via `Box::pin`) and then never moves it. `Pin` is the type-system enforcement of this.

```rust
// 95% of the time you don't think about Pin. You write async fn.
// You see it when:
// - Implementing Future manually.
// - Returning Pin<Box<dyn Future + Send>> from a trait method.

use std::pin::Pin;

trait FetchService {
    fn fetch(&self, url: String) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + '_>>;
}
```

### async fn in Traits (Stable in 1.75+)

Since Rust 1.75, you can write `async fn` in traits directly:

```rust
trait FetchService {
    async fn fetch(&self, url: String) -> Vec<u8>;
}
```

The compiler generates the right future type. One caveat: trait objects (`dyn FetchService`) require a crate like `async-trait` or manual Pin<Box<dyn Future>> for now.

### async-trait (Still Useful)

```rust
use async_trait::async_trait;

#[async_trait]
pub trait FetchService: Send + Sync {
    async fn fetch(&self, url: String) -> Vec<u8>;
}

// Usable as dyn FetchService
let svc: Box<dyn FetchService> = Box::new(HttpFetcher);
```

The macro expands each `async fn` into `fn -> Pin<Box<dyn Future + Send>>`. Small overhead (heap allocation per call) in exchange for `dyn`-compatibility.

---

## 4. Cancellation

Any `.await` point is a potential cancellation point. If the future is dropped at that point, execution stops immediately.

### The Contract

When a future is dropped:

- Its state is destructed via `Drop::drop` on each local.
- No more code runs.
- Any I/O or resources held are released via their Drop impls.

This is clean in principle. In practice, it means you cannot assume code after an `.await` will run.

### Cancellation Bugs

```rust
async fn bad(db: &Db, n: &Notifier) -> Result<(), Error> {
    db.save(&record).await?;
    // If cancelled here, the record is saved but the notify never happens.
    n.send("saved").await?;
    Ok(())
}
```

If the caller drops this future between the two awaits, you have half-done work. Rust doesn't prevent this; you must design for it.

### Compensating Actions

```rust
async fn save_then_notify(db: &Db, n: &Notifier, record: Record) -> Result<(), Error> {
    let guard = RollbackOnDrop::new(db, &record);
    db.save(&record).await?;
    n.send("saved").await?;
    guard.commit();  // disarm the rollback
    Ok(())
}

struct RollbackOnDrop<'a> {
    db: &'a Db,
    record: &'a Record,
    armed: bool,
}

impl Drop for RollbackOnDrop<'_> {
    fn drop(&mut self) {
        if self.armed {
            // best-effort rollback, sync
            let _ = self.db.blocking_rollback(self.record);
        }
    }
}
```

RAII guards that only run cleanup if not disarmed. Common pattern for cancel-safety.

### Shielding

```rust
use tokio::task;

async fn cleanup_task() {
    task::spawn(async {
        // runs independently -- not cancelled by parent being dropped
        save_state().await;
    });
}
```

Spawning a task detaches it from the parent's cancellation. Use for cleanup that must complete.

---

## 5. Concurrency Primitives

### tokio::join!

Run multiple futures concurrently, wait for all.

```rust
let (a, b, c) = tokio::join!(
    fetch("a"),
    fetch("b"),
    fetch("c"),
);
```

All futures run concurrently on the current task. If any panics, all are dropped. If you need error short-circuiting, use `try_join!`:

```rust
let (a, b, c) = tokio::try_join!(
    fetch_fallible("a"),
    fetch_fallible("b"),
    fetch_fallible("c"),
)?;
```

### tokio::select!

Race multiple futures, take the first to complete.

```rust
tokio::select! {
    data = fetch(url) => {
        println!("got {:?}", data);
    }
    _ = tokio::time::sleep(Duration::from_secs(5)) => {
        println!("timeout");
    }
    _ = shutdown_signal() => {
        println!("shutting down");
    }
}
```

Other futures are dropped. Great for timeouts, cancellation, shutdown signals.

### JoinSet

For structured concurrency with dynamic number of tasks.

```rust
use tokio::task::JoinSet;

async fn fetch_all(urls: Vec<String>) -> Result<Vec<Bytes>, Error> {
    let mut set = JoinSet::new();

    for url in urls {
        set.spawn(async move { fetch(url).await });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        let bytes = res??;  // outer ? for JoinError, inner for fetch error
        results.push(bytes);
    }
    Ok(results)
}
```

`JoinSet` is the Rust equivalent of Python's `asyncio.TaskGroup` or Go's `errgroup`. Dropping the set cancels all contained tasks.

### FuturesUnordered

Run N futures, yield results as they complete.

```rust
use futures::stream::{FuturesUnordered, StreamExt};

async fn process_all(futures: Vec<impl Future<Output = i32>>) -> i32 {
    let mut set: FuturesUnordered<_> = futures.into_iter().collect();
    let mut total = 0;
    while let Some(result) = set.next().await {
        total += result;
    }
    total
}
```

Like JoinSet but doesn't spawn tasks -- runs all futures on the current task. Lower overhead; cannot span threads.

---

## 6. Streams

Streams are the async analog of iterators.

```rust
use futures::StreamExt;

async fn process(mut stream: impl Stream<Item = i32> + Unpin) {
    while let Some(item) = stream.next().await {
        println!("{}", item);
    }
}
```

Common sources:

- `tokio::sync::mpsc::Receiver` -- via `UnboundedReceiverStream` wrapper
- SSE/websocket connections
- Database query results
- File line iterators via `tokio::io::BufReader`

### Stream Combinators

```rust
use futures::StreamExt;

let sum: i32 = stream
    .filter(|x| async move { x > &0 })
    .map(|x| x * 2)
    .take(100)
    .fold(0, |acc, x| async move { acc + x })
    .await;
```

Note the `async move {}` inside `filter` and `fold` -- stream combinators take async closures.

---

## 7. Timers and Timeouts

### Sleep

```rust
use tokio::time::{sleep, Duration};

sleep(Duration::from_millis(100)).await;
```

### Timeout

```rust
use tokio::time::timeout;

match timeout(Duration::from_secs(5), fetch(url)).await {
    Ok(Ok(data)) => println!("{:?}", data),
    Ok(Err(e)) => println!("fetch error: {}", e),
    Err(_) => println!("timed out"),
}
```

### Interval (Periodic Tick)

```rust
let mut interval = tokio::time::interval(Duration::from_secs(1));
loop {
    interval.tick().await;
    check_health().await;
}
```

First `tick()` returns immediately. Subsequent ticks wait until the period elapses.

---

## 8. Channels (tokio::sync)

### mpsc (Multi-Producer Single-Consumer)

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);  // bounded

let tx2 = tx.clone();
tokio::spawn(async move {
    tx.send("a".to_string()).await.unwrap();
});
tokio::spawn(async move {
    tx2.send("b".to_string()).await.unwrap();
});

while let Some(msg) = rx.recv().await {
    println!("{}", msg);
}
```

- Bounded channels apply backpressure: `send().await` pauses when full.
- Unbounded can exhaust memory under producer-heavy load. Use sparingly.

### oneshot

```rust
let (tx, rx) = tokio::sync::oneshot::channel::<Result<i32, Error>>();

tokio::spawn(async move {
    let result = compute().await;
    let _ = tx.send(result);   // ignore error if receiver gone
});

match rx.await {
    Ok(result) => println!("{:?}", result),
    Err(_) => println!("sender dropped"),
}
```

For a single send/receive. Lighter than `mpsc` for that case.

### broadcast

```rust
let (tx, _rx) = tokio::sync::broadcast::channel::<String>(16);

let mut rx1 = tx.subscribe();
let mut rx2 = tx.subscribe();

tx.send("hello".to_string()).unwrap();

println!("{}", rx1.recv().await.unwrap());  // "hello"
println!("{}", rx2.recv().await.unwrap());  // "hello"
```

Every subscriber gets a copy. Useful for fan-out events (e.g., system shutdown signal).

### watch

```rust
let (tx, rx) = tokio::sync::watch::channel::<i32>(0);

tokio::spawn(async move {
    let mut rx = rx;
    loop {
        rx.changed().await.unwrap();
        println!("new value: {}", *rx.borrow());
    }
});

tx.send(42).unwrap();
tx.send(43).unwrap();
```

Holds only the latest value. Subscribers miss intermediate values but always see the current state. Good for config reloads and heartbeats.

---

## 9. tokio::sync::Mutex vs std::sync::Mutex

| Property | std::sync::Mutex | tokio::sync::Mutex |
|----------|------------------|---------------------|
| Blocking | Yes (kernel mutex) | No (yields to runtime) |
| Overhead | Lower | Higher |
| Safe across await | Sometimes risky | Yes |
| Use in async | Only if not held across await | Default for async |
| Use in sync | Yes | Use `blocking_lock()` (discouraged) |

Rule: async code uses `tokio::sync::Mutex`. Sync code uses `std::sync::Mutex`. If you hold a lock across an await, it must be tokio's.

For very hot paths where you can guarantee no await while the lock is held, `parking_lot::Mutex` is even faster than std's.

---

## 10. Structured Async Logging (tracing)

```rust
use tracing::{info, instrument};

#[instrument(skip(db))]
async fn load_order(db: &Db, id: &str) -> Result<Order, Error> {
    info!("loading order");
    let o = db.load(id).await?;
    info!(order_id = %o.id, total = o.total_cents, "loaded");
    Ok(o)
}
```

`#[instrument]` wraps the function in a span. When combined with the tokio console or tracing-subscriber, you get per-span timing, async-aware stack traces, and structured JSON logs.

### Tokio Console

```rust
// In Cargo.toml
console-subscriber = "0.2"

// In main
console_subscriber::init();
```

Then run with:

```bash
tokio-console http://127.0.0.1:6669
```

Live view of every task, their state (running/waiting/idle), how long they've been stuck, and on what. Indispensable for debugging async issues.

---

## 11. Common Async Patterns

### Retry with Backoff

```rust
async fn retry<F, Fut, T, E>(mut f: F, max_attempts: u32) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_err = None;
    for attempt in 0..max_attempts {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                eprintln!("attempt {} failed: {}", attempt + 1, e);
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
            }
        }
    }
    Err(last_err.unwrap())
}
```

### Rate Limiting

```rust
use tokio::sync::Semaphore;

let sem = Arc::new(Semaphore::new(10));

for req in requests {
    let permit = sem.clone().acquire_owned().await.unwrap();
    tokio::spawn(async move {
        let _permit = permit;  // held until task completes
        process(req).await;
    });
}
```

Semaphore bounds concurrent tasks. Never spawn so many tasks that the runtime chokes.

### Graceful Shutdown

```rust
use tokio::signal;

#[tokio::main]
async fn main() {
    let server = tokio::spawn(run_server());

    tokio::select! {
        _ = server => { eprintln!("server exited"); }
        _ = signal::ctrl_c() => {
            eprintln!("received Ctrl-C, shutting down");
            // optional: wait for in-flight requests via a shutdown channel
        }
    }
}
```

---

## 12. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `std::thread::sleep` in async | `tokio::time::sleep` |
| `std::sync::MutexGuard` held across await | `tokio::sync::Mutex` or release first |
| `block_on` inside async | `spawn_blocking` or refactor |
| Ignoring the JoinHandle | `let _ = tokio::spawn(...)` explicitly or await it |
| Unbounded channels for backpressure-sensitive paths | Use bounded; .await propagates backpressure |
| Heavy CPU work directly in async fn | `spawn_blocking` or `rayon` thread pool |
| `Rc` / `RefCell` on multi-threaded runtime | `Arc` / `tokio::sync::Mutex` |
| Spawning a task without tracking it | Keep `JoinHandle` or use `JoinSet` |
| Not handling CancelledError semantics | Treat cancellation as a first-class state |
| `futures::join_all` for many tasks | `JoinSet` -- handles errors cleanly |

---

## 13. Cross-References

- [borrow-checker.md](borrow-checker.md) -- Send/Sync auto traits
- [traits-design.md](traits-design.md) -- async trait methods, dyn Trait
- [errors.md](errors.md) -- async error propagation
- [full-guide.md](full-guide.md) -- tokio, axum, sqlx patterns
