# fireworks-python -- Full Guide

> The deep reference for Python 3.11+ development.
> Loaded on demand from `SKILL.md`.

---

## 1. Project Layout (src/ Layout)

The `src/` layout is the community consensus for new projects. It prevents a class of "works when I run from the repo root, fails when installed" bugs.

```
mypackage/
  pyproject.toml
  src/
    mypackage/
      __init__.py
      core.py
      cli.py
      _internal.py       # leading underscore = private module
  tests/
    conftest.py
    test_core.py
  docs/
  README.md
  LICENSE
```

The key property: `import mypackage` only works when the package is installed (editable or real). You cannot accidentally import it from the current working directory. This forces your test suite and your production runtime to use the same import path.

### Naming

- Package: `lower_underscore` in imports, `lower-dash` in pip/PyPI. Prefer matching them: `my_package` everywhere.
- Module: `lower_underscore`.
- Class: `CapitalCase`.
- Function/variable: `lower_underscore`.
- Constant: `UPPER_UNDERSCORE`.

---

## 2. Modern Syntax You Should Use

### Structural Pattern Matching (3.10+)

```python
match event:
    case OrderPlaced(id=order_id, total=total) if total > 10000:
        flag_for_review(order_id)
    case OrderPlaced(id=order_id):
        process(order_id)
    case OrderCancelled(id=order_id, reason=reason):
        log_cancel(order_id, reason)
    case _:
        raise ValueError(f"unknown event: {event!r}")
```

Not a switch. It's destructuring + guards. Use it for AST walks, event dispatching, and anywhere you currently have a chain of `isinstance` + attribute access.

### Assignment Expressions (3.8+)

```python
# Old
data = fetch()
if data is not None:
    process(data)

# New
if (data := fetch()) is not None:
    process(data)
```

Don't overuse. Easy to make code hard to read.

### f-strings with `=` (3.8+) for Debug

```python
x = 42
print(f"{x=}")  # prints "x=42"
```

### Positional-only and Keyword-only Parameters

```python
def f(a, b, /, c, d, *, e, f):
    """a, b must be positional. e, f must be keyword. c, d can be either."""
```

Keyword-only is the useful one. Use it for boolean flags and rarely-set options:

```python
def open_file(path: str, *, read_only: bool = True) -> File:
    ...

# Readable call
open_file("/etc/passwd", read_only=True)
# Forbidden at call site
open_file("/etc/passwd", True)  # TypeError
```

### type Statement (3.12+)

```python
type UserID = str
type OrderMap = dict[UserID, list[Order]]
```

Better than `UserID = str` because mypy treats `UserID` as a distinct alias, not a reassignment.

---

## 3. Exception Handling

### Chain Exceptions

```python
try:
    value = int(raw)
except ValueError as e:
    raise InvalidOrderError(f"bad total: {raw}") from e
```

The `from e` preserves the original traceback. Without it, the caller sees only `InvalidOrderError` and has to dig into logs to find the real cause.

To suppress the original deliberately:

```python
raise InvalidOrderError("bad total") from None
```

Use `from None` only when the original exception is an implementation detail that would confuse the caller.

### Specific, Not Bare

```python
# WRONG
try:
    do_thing()
except:
    pass

# WRONG (almost)
try:
    do_thing()
except Exception:
    pass

# RIGHT
try:
    do_thing()
except (ValueError, TypeError) as e:
    handle(e)
```

Bare `except:` catches `SystemExit` and `KeyboardInterrupt`. `except Exception:` does not, but it's still overly broad -- name the exceptions you expect.

### Context Managers for Cleanup

```python
# Old
f = open("/etc/foo", "r")
try:
    data = f.read()
finally:
    f.close()

# New
with open("/etc/foo", "r") as f:
    data = f.read()
```

Most resource-holding types now support context managers: files, locks, DB transactions, HTTP sessions. Use them.

### Custom Context Managers

```python
from contextlib import contextmanager
from typing import Iterator

@contextmanager
def timer(label: str) -> Iterator[None]:
    start = time.perf_counter()
    try:
        yield
    finally:
        print(f"{label}: {time.perf_counter() - start:.3f}s")

with timer("submit"):
    submit_order(o)
```

For async context managers, use `@asynccontextmanager`:

```python
from contextlib import asynccontextmanager

@asynccontextmanager
async def db_session() -> AsyncIterator[Session]:
    s = await pool.acquire()
    try:
        yield s
    finally:
        await pool.release(s)
```

---

## 4. Standard Library Highlights

### pathlib

Stop using `os.path`. Use `pathlib.Path`.

```python
from pathlib import Path

cfg = Path.home() / ".config" / "myapp" / "config.toml"
if cfg.exists():
    data = cfg.read_text()
for f in cfg.parent.glob("*.toml"):
    print(f)
```

### dataclasses

Already covered in SKILL.md. The one subtlety:

```python
@dataclass
class Order:
    items: list[Item] = field(default_factory=list)  # NOT []
```

`field(default_factory=list)` creates a new list per instance. Using `items: list[Item] = []` would share one list across all instances -- classic Python gotcha.

### enum

```python
from enum import Enum, auto

class Status(Enum):
    PENDING = auto()
    PROCESSING = auto()
    COMPLETE = auto()
    FAILED = auto()

# Access by name or value
Status.PENDING.name   # "PENDING"
Status.PENDING.value  # 1
Status["PENDING"]     # Status.PENDING

# StrEnum for JSON-friendly enums (3.11+)
from enum import StrEnum

class Tier(StrEnum):
    FREE = "free"
    PRO = "pro"
    ENTERPRISE = "enterprise"

Tier.PRO == "pro"  # True
```

### functools

```python
from functools import lru_cache, cached_property

@lru_cache(maxsize=1024)
def expensive(x: int) -> int:
    ...

class User:
    @cached_property
    def profile(self) -> Profile:
        return fetch_profile(self.id)  # computed once per instance
```

`cached_property` vs `lru_cache`: `cached_property` caches per instance and has no max size; `lru_cache` caches globally with an LRU policy.

### itertools

```python
from itertools import batched, groupby, chain

# Python 3.12+
for batch in batched(items, 100):
    process_batch(batch)

# Group consecutive equal keys
for key, group in groupby(sorted_items, key=lambda x: x.customer_id):
    process_customer(key, list(group))

# Flatten
flat = list(chain.from_iterable(list_of_lists))
```

### collections

```python
from collections import Counter, defaultdict, deque

# Count occurrences
c = Counter(words)
c.most_common(10)

# Auto-init
d = defaultdict(list)
for k, v in pairs:
    d[k].append(v)

# Fast pops from both ends
q = deque(maxlen=100)
q.append(x)
q.popleft()
```

### subprocess

```python
import subprocess

# Modern call
result = subprocess.run(
    ["git", "log", "-1", "--format=%H"],
    capture_output=True,
    text=True,
    check=True,
)
commit = result.stdout.strip()
```

Always prefer the list form over a shell string. List form does not invoke the shell and is not vulnerable to shell injection.

### logging

```python
import logging

logger = logging.getLogger(__name__)

def submit(order: Order) -> None:
    logger.info("submit order", extra={"order_id": order.id, "total": order.total_cents})
    try:
        ...
    except Exception:
        logger.exception("submit failed", extra={"order_id": order.id})
        raise
```

`logger.exception` captures the current traceback automatically.

For structured logs (JSON output), use `structlog` or configure a `JSONFormatter`. Modern deployments want structured logs for ingestion.

---

## 5. HTTP Frameworks

### FastAPI (modern default)

```python
from fastapi import FastAPI, HTTPException, Depends
from pydantic import BaseModel

app = FastAPI()

class CreateOrder(BaseModel):
    customer_id: str
    total_cents: int

class OrderResponse(BaseModel):
    id: str
    customer_id: str
    total_cents: int

@app.post("/orders", response_model=OrderResponse, status_code=201)
async def create_order(
    req: CreateOrder,
    svc: OrderService = Depends(get_order_service),
) -> OrderResponse:
    try:
        o = await svc.create(req.customer_id, req.total_cents)
    except InsufficientFunds:
        raise HTTPException(status_code=402, detail="insufficient funds")
    return OrderResponse(id=o.id, customer_id=o.customer_id, total_cents=o.total_cents)
```

FastAPI does validation, serialisation, OpenAPI, and dependency injection. Default for new services.

### Flask (light and familiar)

```python
from flask import Flask, request, jsonify

app = Flask(__name__)

@app.post("/orders")
def create_order():
    data = request.get_json()
    # manual validation
    if "customer_id" not in data:
        return jsonify({"error": "customer_id required"}), 400
    o = svc.create(data["customer_id"], data["total_cents"])
    return jsonify({"id": o.id}), 201
```

Flask is older, lower-level, and still widely used. Use it when you want minimal magic.

### Django (monolith with ORM)

```python
# views.py
from django.http import JsonResponse
from django.views.decorators.http import require_POST

@require_POST
def create_order(request):
    ...
```

Django is the right choice for admin-heavy applications with heavy ORM use. Its auth, admin, and form systems save enormous time for the right use case.

---

## 6. SQLAlchemy 2.x (Typed)

SQLAlchemy 2.x has a typed API with `Mapped` annotations.

```python
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column
from sqlalchemy import String, ForeignKey
from datetime import datetime

class Base(DeclarativeBase):
    pass

class Order(Base):
    __tablename__ = "orders"
    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    customer_id: Mapped[str] = mapped_column(ForeignKey("customers.id"))
    total_cents: Mapped[int]
    created_at: Mapped[datetime] = mapped_column(default=datetime.utcnow)
    notes: Mapped[str | None]
```

Queries use the typed `select`:

```python
from sqlalchemy import select

stmt = select(Order).where(Order.customer_id == customer_id)
result = await session.execute(stmt)
orders = result.scalars().all()
```

Use SQLAlchemy 2.x with asyncpg for async Postgres:

```python
from sqlalchemy.ext.asyncio import create_async_engine, AsyncSession
engine = create_async_engine("postgresql+asyncpg://user:pass@host/db")
```

---

## 7. Configuration

### Single Source of Truth: pyproject.toml

Most tools now read their config from `pyproject.toml`:

- `[tool.mypy]` for mypy
- `[tool.ruff]` for ruff
- `[tool.pytest.ini_options]` for pytest
- `[tool.coverage.*]` for coverage
- `[project]` for build metadata

Having one file is easier to reason about and reduces "where does this setting live" confusion.

### Runtime Config via Pydantic Settings

```python
from pydantic_settings import BaseSettings

class Settings(BaseSettings):
    db_url: str
    log_level: str = "INFO"
    timeout: float = 30.0

    class Config:
        env_file = ".env"
        env_prefix = "MYAPP_"

settings = Settings()
```

`pydantic_settings` reads env vars and `.env` files with validation. Beats hand-rolled config loaders.

---

## 8. Decorators Worth Knowing

```python
from functools import wraps
from typing import Callable, ParamSpec, TypeVar

P = ParamSpec("P")
R = TypeVar("R")

def retry(max_attempts: int = 3, backoff: float = 1.0) -> Callable[[Callable[P, R]], Callable[P, R]]:
    def decorator(func: Callable[P, R]) -> Callable[P, R]:
        @wraps(func)
        def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
            last: Exception | None = None
            for attempt in range(max_attempts):
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    last = e
                    if attempt < max_attempts - 1:
                        time.sleep(backoff * (2 ** attempt))
            assert last is not None
            raise last
        return wrapper
    return decorator

@retry(max_attempts=5, backoff=0.5)
def fetch(url: str) -> bytes: ...
```

Note the `@wraps(func)` -- it copies `__name__`, `__doc__`, and `__wrapped__` from the wrapped function. Without it, the decorator erases the function's identity.

### Async Decorators

```python
def async_retry(max_attempts: int = 3) -> Callable[[Callable[P, Awaitable[R]]], Callable[P, Awaitable[R]]]:
    def decorator(func: Callable[P, Awaitable[R]]) -> Callable[P, Awaitable[R]]:
        @wraps(func)
        async def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
            ...
        return wrapper
    return decorator
```

The types are more verbose but the idea is the same.

---

## 9. Imports and Module Structure

### Import Order (ruff/isort default)

```python
# 1. Standard library
import os
import sys
from pathlib import Path

# 2. Third-party
import httpx
from pydantic import BaseModel

# 3. Local
from mypackage import config
from mypackage.core import OrderService
```

ruff enforces this automatically (`I` rules). Run `ruff check --fix` to auto-sort.

### Avoid `import *`

Except in `__init__.py` for explicit re-exports:

```python
# mypackage/__init__.py
from mypackage.core import OrderService, submit
from mypackage.types import Order, Customer

__all__ = ["OrderService", "submit", "Order", "Customer"]
```

`__all__` controls what `from mypackage import *` imports. It's also what static analysers and docs generators use.

### Circular Imports

Symptom: `ImportError: cannot import name 'X' from 'Y' (most likely due to a circular import)`.

Fix:

1. Move the shared thing to a third module.
2. Use lazy imports inside functions.
3. Use `TYPE_CHECKING` for type-only imports:

```python
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from mypackage.orders import Order

def process(o: "Order") -> None:
    ...
```

The quoted `"Order"` is resolved by mypy but not at runtime.

---

## 10. Cross-References

| Resource | Where | Purpose |
|----------|-------|---------|
| PEPs (Python Enhancement Proposals) | https://peps.python.org/ | Language spec |
| Python Type Hints | [typing-deep-dive.md](typing-deep-dive.md) | Protocol, Generic, etc. |
| Asyncio | [asyncio-pitfalls.md](asyncio-pitfalls.md) | TaskGroup, cancellation |
| Packaging | [packaging-modern.md](packaging-modern.md) | pyproject.toml, uv |
| Testing | [pytest-patterns.md](pytest-patterns.md) | Fixtures, parametrize |
| Performance | [gil-and-perf.md](gil-and-perf.md) | GIL, profiling, multiprocessing |
