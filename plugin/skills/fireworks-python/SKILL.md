---
name: fireworks-python
version: 1.0.0
author: mneme
description: Use when writing Python services with strict typing (mypy --strict), designing async I/O with asyncio, building APIs with FastAPI/Flask/Django, modelling with pydantic/dataclasses/TypedDict, working around the GIL with multiprocessing, packaging with pyproject.toml + uv/poetry, writing pytest fixtures, or profiling with cProfile/line_profiler. Covers Python 3.11+, Protocol typing, generic TypeVar/ParamSpec, asyncio cancellation, and modern packaging.
triggers:
  - python
  - py
  - asyncio
  - await
  - aiohttp
  - fastapi
  - flask
  - django
  - pydantic
  - sqlalchemy
  - pytest
  - mypy
  - ruff
  - black
  - pip
  - poetry
  - uv
  - venv
  - pyproject
  - dataclass
  - typeddict
  - protocol
  - gil
tags:
  - python
  - typing
  - async
  - pytest
  - pydantic
  - dataclasses
  - gil
---

# FIREWORKS-PYTHON -- Modern Python 3.11+ Superbrain

> The definitive Python skill for production services.
> Pairs with `fireworks-test`, `fireworks-performance`, and `fireworks-debug`.

---

## 1. The Python Protocol

Every Python task moves through this pipeline.

```
DESIGN --> WRITE --> mypy --strict --> ruff --> pytest --> SHIP
```

1. **DESIGN** -- Sketch types and data flow first. Pick sync vs async deliberately.
2. **WRITE** -- Idiomatic Python: type hints everywhere, no `Any`, structural typing via Protocol.
3. **mypy --strict** -- Zero errors. Type discipline is non-negotiable.
4. **ruff check** -- Lint and format. Replaces black, isort, flake8, pylint, pyupgrade.
5. **pytest** -- All tests pass. New code has tests.
6. **SHIP** -- Only after every gate is green.

### Pre-Flight Checklist

- [ ] Are public functions and classes fully type-annotated?
- [ ] Is `Any` justified anywhere? (almost always: it isn't)
- [ ] Sync or async? Picked deliberately, not by habit.
- [ ] If async: do all blocking calls use the async equivalent (or `asyncio.to_thread`)?
- [ ] Are exceptions chained with `raise ... from err`?
- [ ] Is `pyproject.toml` the single source of truth for build, deps, lint, mypy?

---

## 2. Strict Typing Discipline

Modern Python is type-checked. Period. The runtime is dynamic; the editor and CI are not.

### mypy --strict

```toml
# pyproject.toml
[tool.mypy]
strict = true
python_version = "3.11"
warn_unused_ignores = true
warn_redundant_casts = true
warn_return_any = true
disallow_any_explicit = true
```

`disallow_any_explicit` is the toughest setting. It makes every `Any` show up as a code smell. Use `cast`, `Protocol`, or generic typevars instead.

### Type Hints on Every Public Surface

```python
def submit_order(
    order: Order,
    notifier: Notifier,
    *,
    retry_count: int = 3,
    timeout: float = 30.0,
) -> SubmitResult:
    ...
```

Three rules:

1. Every parameter is annotated.
2. Every return type is annotated, including `-> None`.
3. Optional and keyword-only are explicit.

### Use the Modern Syntax

```python
# Python 3.10+
def f(x: int | str) -> list[int] | None: ...

# OLD (3.9 and earlier) -- avoid in new code
from typing import Optional, Union, List
def f(x: Union[int, str]) -> Optional[List[int]]: ...
```

The modern syntax is shorter, faster, and the future. Only use `Optional`/`Union`/`List` if you must support Python 3.9.

> Deep dive: [references/typing-deep-dive.md](references/typing-deep-dive.md)

---

## 3. Protocol vs ABC vs TypedDict

Three ways to describe shapes. Each has a sweet spot.

### Protocol (Structural Typing)

Use when you want duck typing with type checking. The implementer doesn't need to inherit anything; structural compatibility is enough.

```python
from typing import Protocol

class Notifier(Protocol):
    def send(self, message: str) -> None: ...

# Any class with a matching send() method satisfies Notifier.
class EmailNotifier:
    def send(self, message: str) -> None:
        smtp.send(message)

class SlackNotifier:
    def send(self, message: str) -> None:
        webhook.post(message)

def notify_all(notifiers: list[Notifier], msg: str) -> None:
    for n in notifiers:
        n.send(msg)

notify_all([EmailNotifier(), SlackNotifier()], "deploy complete")
```

Protocols can also be runtime-checkable with `@runtime_checkable`, but use that sparingly -- it falls back to attribute presence, not signature.

### ABC (Nominal Typing)

Use when you want explicit subclassing as a contract. Caller cares about identity ("is-a Notifier"), not just shape.

```python
from abc import ABC, abstractmethod

class Notifier(ABC):
    @abstractmethod
    def send(self, message: str) -> None: ...

class EmailNotifier(Notifier):
    def send(self, message: str) -> None: ...
```

You cannot instantiate `Notifier` directly, and you must inherit to satisfy it. Useful for plug-in systems where you want a registry of all subclasses.

### TypedDict

Use for typed JSON payloads, config dicts, or anything you would have used as `dict[str, Any]`.

```python
from typing import TypedDict

class Order(TypedDict):
    id: str
    total_cents: int
    customer_id: str
    notes: str | None

def submit(o: Order) -> None: ...
submit({"id": "abc", "total_cents": 1000, "customer_id": "c1", "notes": None})
```

TypedDict allows `total=False` for partial dicts, and inheritance via subclassing. Great for loosely structured data; not a replacement for dataclasses or pydantic when you have validation/methods.

### Decision Table

| Need | Use |
|------|-----|
| Type-check duck-typed code, no inheritance required | Protocol |
| Force a contract via subclassing, register subclasses | ABC |
| Typed dict-like (JSON, config) without methods | TypedDict |
| Immutable record with methods, validation light | dataclass(frozen=True) |
| Heavy validation, parsing, JSON schema | pydantic.BaseModel |

---

## 4. dataclass vs pydantic.BaseModel

Both give you typed records. The choice depends on what you need.

### dataclass

```python
from dataclasses import dataclass, field
from datetime import datetime

@dataclass(frozen=True, slots=True, kw_only=True)
class Order:
    id: str
    total_cents: int
    created_at: datetime = field(default_factory=datetime.utcnow)
    notes: str | None = None
```

Pros:

- Standard library, no dependency.
- `slots=True` eliminates `__dict__`, halves memory.
- `frozen=True` makes instances hashable and immutable.
- `kw_only=True` forces keyword arguments at construction (no positional ambiguity).

Cons:

- No runtime validation. `Order(id=42, total_cents="oops")` doesn't fail until something else uses the bad value.
- No JSON serialisation built in.

Use when: data is internal, validation is enforced upstream, and performance matters.

### pydantic.BaseModel

```python
from pydantic import BaseModel, Field
from datetime import datetime

class Order(BaseModel):
    id: str = Field(min_length=1, max_length=64)
    total_cents: int = Field(ge=0)
    created_at: datetime = Field(default_factory=datetime.utcnow)
    notes: str | None = None

# Raises ValidationError on bad input
o = Order(id="abc", total_cents=1000)
o.model_dump()        # dict
o.model_dump_json()   # JSON string
Order.model_validate_json(json_str)  # parse + validate
```

Pros:

- Runtime validation with rich constraint syntax.
- Built-in JSON serialisation.
- FastAPI uses it natively for request/response models.

Cons:

- External dependency.
- Slower than dataclass at construction.
- pydantic v2 syntax differs from v1; check version.

Use when: data crosses an API boundary or comes from untrusted input.

### Rule of Thumb

- API request/response, config from disk -> pydantic
- Internal records, performance-sensitive -> dataclass(slots=True, frozen=True)
- Quick scratch -> NamedTuple or @dataclass

---

## 5. Generics: TypeVar and ParamSpec

### TypeVar (typed generic functions)

```python
from typing import TypeVar

T = TypeVar("T")

def first(items: list[T]) -> T | None:
    return items[0] if items else None

x: int | None = first([1, 2, 3])
y: str | None = first(["a", "b"])
```

### Bounded TypeVars

```python
from typing import TypeVar
from numbers import Number

NumT = TypeVar("NumT", bound=Number)

def total(values: list[NumT]) -> NumT:
    if not values:
        raise ValueError("empty")
    result = values[0]
    for v in values[1:]:
        result = result + v  # type: ignore[operator]
    return result
```

### Generic Classes (Python 3.12+ syntax)

```python
# Python 3.12+
class Cache[K, V]:
    def __init__(self) -> None:
        self._items: dict[K, V] = {}

    def get(self, key: K) -> V | None:
        return self._items.get(key)

    def put(self, key: K, value: V) -> None:
        self._items[key] = value

cache: Cache[str, int] = Cache()
```

Pre-3.12, use `Generic[K, V]`:

```python
from typing import Generic, TypeVar

K = TypeVar("K")
V = TypeVar("V")

class Cache(Generic[K, V]): ...
```

### ParamSpec (typed decorators)

```python
from typing import ParamSpec, TypeVar, Callable
from functools import wraps
import time

P = ParamSpec("P")
R = TypeVar("R")

def timed(func: Callable[P, R]) -> Callable[P, R]:
    @wraps(func)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
        start = time.perf_counter()
        try:
            return func(*args, **kwargs)
        finally:
            print(f"{func.__name__}: {time.perf_counter() - start:.3f}s")
    return wrapper

@timed
def submit(order_id: str, total: int) -> bool: ...
```

`ParamSpec` preserves the wrapped function's signature, including positional and keyword arguments. Without it, decorators erase types.

> Deep dive: [references/typing-deep-dive.md](references/typing-deep-dive.md)

---

## 6. Asyncio: The Pitfalls

Async Python is fast for I/O-bound workloads. It is also a minefield.

### Pitfall 1: Blocking Inside async def

```python
import asyncio
import time

# WRONG -- blocks the event loop, every other coroutine pauses
async def fetch(url: str) -> bytes:
    time.sleep(1)            # synchronous sleep, blocks the loop
    return urlopen(url).read()  # synchronous I/O

# RIGHT
async def fetch(url: str) -> bytes:
    await asyncio.sleep(1)   # async-friendly sleep
    async with aiohttp.ClientSession() as s:
        async with s.get(url) as resp:
            return await resp.read()
```

If you need to call sync code, push it to a thread:

```python
data = await asyncio.to_thread(blocking_function, arg1, arg2)
```

This runs the function in the default thread pool, freeing the event loop.

### Pitfall 2: gather Error Handling

```python
# Default behaviour: first exception cancels siblings, propagates immediately
results = await asyncio.gather(fetch("a"), fetch("b"), fetch("c"))

# Collect all results, success and failure
results = await asyncio.gather(
    fetch("a"), fetch("b"), fetch("c"),
    return_exceptions=True,
)
for r in results:
    if isinstance(r, Exception):
        log.error("fetch failed", exc_info=r)
```

`return_exceptions=True` is dangerous if you forget to check -- exceptions become return values. Always pair with explicit error handling.

### Pitfall 3: Cancellation

```python
async def long_task() -> None:
    try:
        await asyncio.sleep(60)
    except asyncio.CancelledError:
        # cleanup
        await rollback()
        raise  # MUST re-raise unless you have a reason not to
```

`asyncio.CancelledError` is a `BaseException` (not `Exception`) in 3.8+. It propagates through normal `except Exception` handlers.

If you catch and don't re-raise, you've absorbed the cancellation -- the cancelling code thinks the task is gone, but it's actually still running. Always re-raise.

### Pitfall 4: TaskGroup (3.11+)

Prefer `TaskGroup` over `gather` in modern code:

```python
async def fetch_all(urls: list[str]) -> list[bytes]:
    results: list[bytes] = []
    async with asyncio.TaskGroup() as tg:
        tasks = [tg.create_task(fetch(u)) for u in urls]
    return [t.result() for t in tasks]
```

If any task raises, the group cancels remaining tasks and re-raises an `ExceptionGroup`. Structured concurrency for free.

### Pitfall 5: asyncio.Lock vs threading.Lock

```python
# In async code -- use asyncio.Lock
import asyncio
lock = asyncio.Lock()

async def critical_section():
    async with lock:
        ...

# In threading code -- use threading.Lock
import threading
tlock = threading.Lock()
```

Mixing them is a bug. `asyncio.Lock` is not thread-safe; `threading.Lock` blocks the event loop.

> Deep dive: [references/asyncio-pitfalls.md](references/asyncio-pitfalls.md)

---

## 7. The GIL: When It Matters

The Global Interpreter Lock means only one thread executes Python bytecode at a time. Implications:

| Workload | GIL Impact | Solution |
|----------|------------|----------|
| I/O-bound (HTTP, DB, disk) | Minimal -- GIL released during I/O | `asyncio` or `threading` |
| CPU-bound, pure Python | Severe -- threading gives no parallelism | `multiprocessing` or Rust extension |
| CPU-bound, NumPy/Pandas | Often fine -- C code releases GIL | Stay on threads |
| Mixed | Profile -- depends on ratio | Profile first |

### multiprocessing for CPU Parallelism

```python
from multiprocessing import Pool

def process_chunk(chunk: list[int]) -> int:
    return sum(x * x for x in chunk)

if __name__ == "__main__":
    chunks = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    with Pool() as pool:
        results = pool.map(process_chunk, chunks)
    print(sum(results))
```

Each worker is a separate process with its own GIL. Note: `multiprocessing` serialises arguments to send between processes, which is slow for large objects -- prefer `multiprocessing.shared_memory` or memory-mapped files for big data.

### Free-Threaded Python (3.13+)

Python 3.13 introduced an experimental no-GIL build (`python3.13t`). Most code works unmodified, but library compatibility is still maturing. Worth testing if you're CPU-bound and willing to pin to 3.13+.

> Deep dive: [references/gil-and-perf.md](references/gil-and-perf.md)

---

## 8. Packaging: pyproject.toml + uv

### Modern Layout

```
mypackage/
  pyproject.toml          # single source of truth
  src/
    mypackage/
      __init__.py
      core.py
  tests/
    test_core.py
  README.md
  LICENSE
```

The `src/` layout prevents accidentally importing the local source instead of the installed package -- a frequent source of "works on my machine" bugs.

### pyproject.toml Skeleton

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "mypackage"
version = "0.1.0"
description = "Brief one-line description"
requires-python = ">=3.11"
dependencies = [
    "pydantic>=2.0",
    "httpx>=0.25",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.0",
    "pytest-asyncio>=0.23",
    "mypy>=1.8",
    "ruff>=0.3",
]

[tool.mypy]
strict = true
python_version = "3.11"

[tool.ruff]
line-length = 100
target-version = "py311"

[tool.ruff.lint]
select = ["E", "F", "I", "N", "UP", "B", "SIM", "RUF"]

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]
```

### uv: The Modern Package Manager

```bash
# Install uv (one of the supported methods)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Create venv + install
uv venv
uv pip install -e ".[dev]"

# Run anything in the venv
uv run pytest
uv run mypy src

# Lock dependencies
uv pip compile pyproject.toml -o requirements.txt
```

`uv` is 10-100x faster than `pip`/`poetry` and is becoming the de-facto modern tool. For new projects, default to `uv`.

### Choosing Between pip / poetry / uv

| Tool | Pick When |
|------|-----------|
| pip | Simple project, no lockfile needed, deployed to constrained env |
| poetry | Existing project on poetry, you need its dependency resolver |
| uv | Greenfield project, want speed, want to standardise on one tool |

> Deep dive: [references/packaging-modern.md](references/packaging-modern.md)

---

## 9. Pytest Patterns

### Fixtures

```python
import pytest

@pytest.fixture
def order() -> Order:
    return Order(id="abc", total_cents=1000)

def test_submit(order: Order) -> None:
    assert submit(order) is True
```

Fixtures are dependency injection. Pytest matches parameter names to fixture names.

### Parametrize

```python
@pytest.mark.parametrize("input,expected", [
    ("ord_abc", "abc"),
    ("ord_xyz", "xyz"),
    ("", None),
])
def test_parse_id(input: str, expected: str | None) -> None:
    assert parse_id(input) == expected
```

Each parameter combination becomes its own test case. The `pytest -k` flag lets you select by name.

### Async Tests

```python
@pytest.mark.asyncio
async def test_fetch() -> None:
    async with aiohttp.ClientSession() as s:
        result = await fetch(s, "/health")
    assert result.status == 200
```

Or set `asyncio_mode = "auto"` in pyproject.toml and drop the marker.

### monkeypatch

```python
def test_with_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("API_URL", "https://test.example")
    assert load_config().api_url == "https://test.example"

def test_with_attr(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("mymodule.now", lambda: datetime(2026, 1, 1))
    assert format_today() == "2026-01-01"
```

Cleaner than `mock.patch` for simple substitutions. Auto-reverts at test end.

### Mocking

For complex mocks, use `unittest.mock`:

```python
from unittest.mock import Mock, patch, AsyncMock

def test_with_mock() -> None:
    notifier = Mock(spec=Notifier)
    submit(order, notifier)
    notifier.send.assert_called_once_with("order submitted")

@patch("mymodule.requests.get")
def test_with_patch(mock_get: Mock) -> None:
    mock_get.return_value.json.return_value = {"ok": True}
    assert fetch_data() == {"ok": True}
```

`spec=Notifier` limits the mock to the real class's interface -- catches typos in test code.

> Deep dive: [references/pytest-patterns.md](references/pytest-patterns.md)

---

## 10. Performance: Profiling and Drop-Down

### cProfile

```bash
python -m cProfile -o profile.out -s cumulative myscript.py
python -c "import pstats; pstats.Stats('profile.out').sort_stats('cum').print_stats(20)"
```

### line_profiler (kernprof)

```bash
pip install line_profiler
```

```python
@profile
def hot_function():
    ...
```

```bash
kernprof -l -v myscript.py
```

Per-line timings. Indispensable for finding the actual hot line inside a hot function.

### memory_profiler

```bash
pip install memory_profiler
mprof run myscript.py
mprof plot
```

### When to Drop to Rust or C

Python is fast enough for most code. When it isn't:

1. Profile and confirm the bottleneck is pure Python.
2. Try NumPy/Pandas vectorisation first.
3. Try `numba` or `cython` for tight numeric loops.
4. Drop to Rust via `pyo3` for sustained CPU work.

Building a Rust extension is a one-time cost. The runtime savings can be 10-100x on the hot path.

> Deep dive: [references/gil-and-perf.md](references/gil-and-perf.md)

---

## 11. Wrong vs Right -- Quick Reference

| Anti-Pattern | Why It's Wrong | Correct Pattern |
|--------------|----------------|-----------------|
| `def f(x):` (no type hint) | mypy can't help | `def f(x: int) -> str:` |
| `Any` everywhere | Defeats type checking | `Protocol`, generics, `cast` |
| `time.sleep` in async code | Blocks event loop | `await asyncio.sleep` |
| Mutable default arg `def f(x=[])` | Shared across calls | `def f(x: list[int] | None = None)` then `x = x or []` |
| `except:` (bare) | Catches BaseException incl. KeyboardInterrupt | `except Exception:` |
| `raise SomeError(str(e))` | Loses traceback | `raise SomeError(...) from e` |
| `dict[str, Any]` for config | No validation | `pydantic.BaseModel` or `TypedDict` |
| `from module import *` | Pollutes namespace | Named imports |
| Catching to log and ignore | Hides bugs | Re-raise or convert to specific result |
| Calling pip from inside script | Mutates global env | Use venv + pyproject.toml |
| Async + sync mixing without `to_thread` | Blocks loop | `await asyncio.to_thread(...)` |
| Mocking what you're testing | Test passes for wrong reasons | Mock dependencies, test the unit |

---

## 12. Iron Law

```
NO PYTHON CODE WITHOUT TYPE HINTS, NO LOOP-BLOCKING IN ASYNC, NO CATCH-AND-IGNORE.

mypy --strict passes.
ruff check passes.
Every blocking call in async code is awaited or to_thread'd.
Every except clause is specific.
Every dataclass that crosses an API boundary is pydantic instead.
Every test passes with -p no:cacheprovider.
```

---

## 13. Compound Skill Chaining

| Chain To | When | What It Adds |
|----------|------|--------------|
| `fireworks-test` | After implementation | Pytest depth, fixtures, mocking patterns |
| `fireworks-performance` | When optimising | cProfile, line_profiler, drop-to-Rust patterns |
| `fireworks-debug` | On crash or hang | pdb, ipdb, asyncio debug mode |
| `fireworks-security` | On HTTP/auth code | Input validation, OWASP Python, secret handling |
| `fireworks-architect` | New service design | FastAPI patterns, layered architecture |

---

## 14. Reference Files Index

| File | Coverage |
|------|----------|
| [references/full-guide.md](references/full-guide.md) | Overview, project layout, modern syntax, stdlib highlights |
| [references/typing-deep-dive.md](references/typing-deep-dive.md) | Protocol, ABC, Generic, TypeVar, ParamSpec, Self, Literal |
| [references/asyncio-pitfalls.md](references/asyncio-pitfalls.md) | TaskGroup, cancellation, blocking, mixing sync/async |
| [references/packaging-modern.md](references/packaging-modern.md) | pyproject.toml, uv, src layout, locking |
| [references/pytest-patterns.md](references/pytest-patterns.md) | Fixtures, parametrize, async tests, monkeypatch, mocks |
| [references/gil-and-perf.md](references/gil-and-perf.md) | GIL impact, profiling, multiprocessing, dropping to Rust |

---

## 15. Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
