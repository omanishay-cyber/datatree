# fireworks-python -- Asyncio Pitfalls

> TaskGroup, cancellation, blocking detection, mixing sync/async, locks.
> The common ways asyncio code goes wrong and how to avoid them.

---

## 1. The Event Loop Model in One Page

- A single event loop runs in one thread.
- Coroutines (`async def` functions) are scheduled onto the loop.
- When a coroutine `await`s something, it yields control back to the loop until the awaited thing completes.
- While one coroutine is running synchronous Python code, no other coroutine makes progress.

The critical consequence: **any blocking call in an async function stops the entire loop**. Not just that coroutine -- every coroutine, every task, every timer, every scheduled callback.

This is why the overwhelming majority of asyncio bugs are "the event loop stalls because someone called something synchronous."

---

## 2. Pitfall: Blocking in async def

### Detection

Enable asyncio debug mode:

```python
import asyncio
asyncio.run(main(), debug=True)
```

Or set `PYTHONASYNCIODEBUG=1` as an environment variable.

In debug mode, coroutines that run for more than 100ms without yielding are logged as warnings. In production-like code, anything over 10ms is suspect.

### Common Blocking Culprits

```python
async def bad():
    time.sleep(1)              # synchronous sleep
    requests.get(url)          # synchronous HTTP
    open("/etc/foo").read()    # synchronous file I/O
    hashlib.sha256(big_blob)   # CPU-bound work
    subprocess.run(["cmd"])    # synchronous subprocess
```

### Fixes

```python
async def good():
    await asyncio.sleep(1)

    async with aiohttp.ClientSession() as s:
        async with s.get(url) as r:
            data = await r.text()

    # Or httpx async client
    async with httpx.AsyncClient() as c:
        r = await c.get(url)

    # For sync I/O on files
    loop = asyncio.get_running_loop()
    data = await loop.run_in_executor(None, lambda: open("/etc/foo").read())
    # Or the modern shortcut
    data = await asyncio.to_thread(_read_file, "/etc/foo")
```

`asyncio.to_thread` (3.9+) is the idiomatic "offload to a thread" call. It runs the callable in the default thread pool and returns a coroutine.

### For CPU-Bound Work

Threading does not help (GIL). Use a process pool:

```python
from concurrent.futures import ProcessPoolExecutor

executor = ProcessPoolExecutor()

async def compute():
    loop = asyncio.get_running_loop()
    result = await loop.run_in_executor(executor, expensive_function, arg)
```

Or drop to native code (Rust extension, NumPy, Cython).

---

## 3. Pitfall: asyncio.gather Error Handling

### Default Behaviour

```python
results = await asyncio.gather(
    fetch("a"),
    fetch("b"),
    fetch("c"),
)
```

If any coroutine raises, `gather`:

1. Cancels the remaining coroutines (best-effort).
2. Re-raises the first exception.

But note: the cancellations are best-effort. If a coroutine already finished, its result is discarded. If a coroutine raised while another was also raising, only the first exception surfaces.

### return_exceptions=True

```python
results = await asyncio.gather(
    fetch("a"),
    fetch("b"),
    fetch("c"),
    return_exceptions=True,
)

for r in results:
    if isinstance(r, BaseException):
        log.error("fetch failed", exc_info=r)
    else:
        process(r)
```

Turns exceptions into return values. Now you must check every element -- easy to forget, easy to pass an exception downstream as "data".

### Prefer TaskGroup

```python
async def fetch_all(urls: list[str]) -> list[bytes]:
    async with asyncio.TaskGroup() as tg:
        tasks = [tg.create_task(fetch(u)) for u in urls]
    # After the async-with block: all tasks done (or cancelled due to failure)
    return [t.result() for t in tasks]
```

If any task raises, TaskGroup:

1. Cancels the remaining tasks.
2. Raises an `ExceptionGroup` containing all failures.

Structured concurrency: the lifetime of child tasks is bounded by the enclosing `async with`. You cannot accidentally leak a task past the group.

### Handling ExceptionGroup

```python
try:
    async with asyncio.TaskGroup() as tg:
        for url in urls:
            tg.create_task(fetch(url))
except* HTTPError as eg:
    for err in eg.exceptions:
        log.error("http error", exc_info=err)
except* TimeoutError as eg:
    for err in eg.exceptions:
        log.warning("timeout", exc_info=err)
```

`except*` (3.11+) matches exception subgroups. You can have multiple `except*` clauses, each matching a type.

---

## 4. Pitfall: Cancellation

### CancelledError Is BaseException

```python
async def work():
    try:
        await long_operation()
    except Exception:
        # CancelledError is NOT caught here (good)
        handle_error()
    except BaseException:
        # CancelledError IS caught here (be careful)
        cleanup()
        raise  # Re-raise unless you truly want to absorb cancellation
```

In Python 3.8+, `CancelledError` inherits from `BaseException`, not `Exception`. It propagates through `except Exception:` handlers by default.

### Never Absorb Cancellation Silently

```python
# WRONG
async def bad():
    try:
        await asyncio.sleep(10)
    except asyncio.CancelledError:
        pass  # work continues, but cancelling code thinks task is gone
    await do_more_work()

# RIGHT
async def good():
    try:
        await asyncio.sleep(10)
    except asyncio.CancelledError:
        # cleanup that's safe during cancellation
        await close_resource()
        raise
    await do_more_work()
```

The cancellation contract: if you're cancelled, either re-raise or raise a different exception. Never continue as if cancellation didn't happen.

### Shielding

```python
# Some cleanup must complete even if we're cancelled
async def write_then_cancel_safely():
    try:
        await work()
    finally:
        await asyncio.shield(save_state())
```

`asyncio.shield` protects the inner coroutine from cancellation. If the outer `await asyncio.shield(x)` is cancelled, the task running `x` continues running but the await raises.

Use shield sparingly. It complicates reasoning.

### Timeouts

```python
# Python 3.11+
try:
    async with asyncio.timeout(5.0):
        data = await fetch(url)
except TimeoutError:
    log.warning("fetch timed out")

# Older alternative
try:
    data = await asyncio.wait_for(fetch(url), timeout=5.0)
except asyncio.TimeoutError:
    ...
```

The context manager form is preferred. It can wrap multiple awaits and handles shielded operations correctly.

### Graceful Shutdown

```python
async def main():
    tasks = [asyncio.create_task(worker(i)) for i in range(10)]
    await asyncio.gather(*tasks, return_exceptions=True)

async def shutdown():
    tasks = [t for t in asyncio.all_tasks() if t is not asyncio.current_task()]
    for t in tasks:
        t.cancel()
    await asyncio.gather(*tasks, return_exceptions=True)
```

On SIGINT/SIGTERM, cancel all tasks, wait for them to finish cleaning up, then exit.

---

## 5. Pitfall: Forgotten await

The single most common asyncio bug.

```python
# WRONG -- returns a coroutine object, never runs it
fetch(url)

# WRONG
result = fetch(url)
process(result)  # 'result' is a coroutine, not data

# RIGHT
result = await fetch(url)
process(result)
```

mypy catches this if `fetch` is properly annotated:

```python
async def fetch(url: str) -> bytes: ...

r = fetch(url)      # r has type Coroutine[Any, Any, bytes], not bytes
r = await fetch(url)  # r has type bytes
```

### Detection

Run with asyncio debug mode on. "coroutine was never awaited" warnings at shutdown are your clue.

Use `ruff` with the `RUF` rules -- it flags common async mistakes.

---

## 6. Pitfall: Fire-and-Forget Tasks

```python
# WRONG -- task reference is dropped, task may be garbage-collected mid-execution
asyncio.create_task(do_thing())

# RIGHT -- keep a reference
background_tasks: set[asyncio.Task] = set()

def start_background(coro):
    task = asyncio.create_task(coro)
    background_tasks.add(task)
    task.add_done_callback(background_tasks.discard)
```

Python's asyncio docs explicitly warn about this. A task whose reference is lost can be garbage-collected. The fix is to keep a strong reference until the task completes.

Better yet, use TaskGroup:

```python
async def main():
    async with asyncio.TaskGroup() as tg:
        tg.create_task(worker_a())
        tg.create_task(worker_b())
        # Both tasks run to completion (or failure) before TaskGroup exits
```

---

## 7. Pitfall: asyncio.Lock vs threading.Lock

```python
# In async code
import asyncio
lock = asyncio.Lock()

async def critical():
    async with lock:
        ...
```

```python
# In threaded code
import threading
lock = threading.Lock()

def critical():
    with lock:
        ...
```

Two locks for two worlds. Mixing them is a bug:

- `asyncio.Lock` in threaded code: not thread-safe. Races on internal state.
- `threading.Lock` in async code: blocks the event loop while held.

### Other asyncio Primitives

```python
asyncio.Semaphore(10)   # limit concurrent work
asyncio.Event()          # signal between coroutines
asyncio.Queue(maxsize=100)  # async-compatible queue
asyncio.BoundedSemaphore(10)  # semaphore that errors on over-release
```

---

## 8. Pitfall: Reading Streams Without Flow Control

### aiohttp

```python
# WRONG -- reads entire response into memory at once
async with session.get(url) as resp:
    data = await resp.read()  # OK for small responses

# RIGHT for large responses
async with session.get(url) as resp:
    async for chunk in resp.content.iter_chunked(8192):
        process(chunk)
```

### asyncio streams

```python
async def echo(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
    while not reader.at_eof():
        data = await reader.read(4096)  # bounded read
        writer.write(data)
        await writer.drain()  # pause if peer is slow
    writer.close()
```

`await writer.drain()` is the flow-control mechanism. It pauses sending if the peer's buffer is full, preventing unbounded memory growth.

---

## 9. Pitfall: Signal Handling

```python
async def main():
    loop = asyncio.get_running_loop()
    stop = asyncio.Event()

    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stop.set)

    await stop.wait()
    await shutdown()
```

On Windows, `loop.add_signal_handler` is not available. Use `signal.signal` with a fallback.

---

## 10. Common Asyncio Gotchas

### Creating a New Loop in a Library

Don't. Libraries should use `asyncio.get_running_loop()` inside coroutines. Never `asyncio.new_event_loop()` in library code -- that's application-level configuration.

### Mixing Sync and Async with loop.run_until_complete

```python
# WRONG -- called inside an already-running loop
def sync_func():
    return loop.run_until_complete(async_func())  # RuntimeError

# RIGHT -- call from sync code at the top
def main():
    asyncio.run(async_main())
```

`asyncio.run` creates a new loop, runs the coroutine, and cleans up. Call it exactly once at the top of a program.

### DNS Lookups Block

The default `getaddrinfo` implementation is synchronous. Under heavy load, it blocks the loop.

Fix: use `aiodns` for async DNS resolution, or configure a connection pool with a custom resolver.

### Database Drivers

Use async-native drivers:

- Postgres: `asyncpg`, or `psycopg3` in async mode
- MySQL: `aiomysql`
- Redis: `redis-py` in async mode
- SQLite: `aiosqlite`

Do not use synchronous DB drivers inside async code. Wrap in `asyncio.to_thread` as a last resort, but expect performance to be limited.

---

## 11. Testing Async Code

### pytest-asyncio

```python
import pytest

@pytest.mark.asyncio
async def test_fetch():
    result = await fetch("https://example.com")
    assert result.status == 200
```

Or with `asyncio_mode = "auto"` in pyproject.toml, drop the marker.

### Async Fixtures

```python
@pytest.fixture
async def session():
    async with aiohttp.ClientSession() as s:
        yield s

@pytest.mark.asyncio
async def test_with_session(session):
    async with session.get("https://example.com") as r:
        assert r.status == 200
```

### AsyncMock

```python
from unittest.mock import AsyncMock

async def test_with_async_mock():
    mock = AsyncMock()
    mock.fetch.return_value = b"data"

    # Usage in code under test
    data = await mock.fetch(url)
    assert data == b"data"
    mock.fetch.assert_awaited_once_with(url)
```

---

## 12. FastAPI Async Patterns

```python
from fastapi import FastAPI

app = FastAPI()

@app.get("/orders/{id}")
async def get_order(id: str) -> Order:
    o = await svc.get(id)
    return o

@app.post("/orders")
async def create_order(req: CreateOrder) -> OrderResponse:
    async with asyncio.TaskGroup() as tg:
        customer_task = tg.create_task(customer_svc.get(req.customer_id))
        inventory_task = tg.create_task(inventory_svc.check(req.items))
    # both complete in parallel

    customer = customer_task.result()
    stock = inventory_task.result()
    return await create_with(customer, stock, req)
```

---

## 13. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `time.sleep` in async | `await asyncio.sleep` |
| `requests.get` in async | `httpx.AsyncClient` or `aiohttp.ClientSession` |
| `asyncio.create_task(x)` without keeping ref | TaskGroup or `background_tasks.add(task)` |
| `except Exception: pass` catching CancelledError | Explicitly handle CancelledError |
| Synchronous DB call in async endpoint | async driver (asyncpg, aiomysql) |
| `asyncio.gather` for structured work | `TaskGroup` |
| Reading full response for big files | `iter_chunked` |
| `loop.run_until_complete` in library code | Make the caller pass a coroutine |
| Per-request `aiohttp.ClientSession` | Create once, reuse across requests |
| Calling async function without await | Always await; mypy catches this |

---

## 14. Cross-References

- [typing-deep-dive.md](typing-deep-dive.md) -- typing async functions
- [pytest-patterns.md](pytest-patterns.md) -- pytest-asyncio, AsyncMock
- [gil-and-perf.md](gil-and-perf.md) -- when async helps and when it doesn't
- [full-guide.md](full-guide.md) -- general Python discipline
