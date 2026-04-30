# fireworks-python -- Typing Deep Dive

> Protocol, ABC, Generic, TypeVar, ParamSpec, Self, Literal, TypeAlias.
> Everything the type system can do and when to reach for which tool.

---

## 1. The Type Checker

`mypy` is the reference implementation. `pyright` (from Microsoft, also the engine behind Pylance) is stricter and faster. Most projects use one; some use both in CI.

```toml
[tool.mypy]
strict = true
python_version = "3.11"
disallow_any_explicit = true
warn_unused_ignores = true
warn_redundant_casts = true
warn_return_any = true
```

`strict = true` turns on every `disallow_*` flag. That's the baseline.

### Running mypy

```bash
mypy src/
mypy --strict src/       # if not in config
mypy --strict src/ tests/  # include tests
```

Check tests too. Test code is still code; bugs there are the worst kind.

---

## 2. Basic Annotations

```python
# Primitives
x: int = 42
name: str = "anish"
active: bool = True
ratio: float = 0.5
raw: bytes = b"..."

# Containers
nums: list[int] = [1, 2, 3]
pairs: tuple[str, int] = ("a", 1)
heterogeneous: tuple[int, ...] = (1, 2, 3)  # any length of ints
m: dict[str, int] = {"a": 1}
s: set[str] = {"a", "b"}

# Optional (| None)
maybe: int | None = None

# Union
val: int | str = "hello"

# Callable
fn: Callable[[int, str], bool] = check
no_args: Callable[[], None] = shutdown
```

---

## 3. Protocol (Structural Typing)

The single most important typing feature for clean architecture.

### Basic Protocol

```python
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str: ...

class Label:
    def __init__(self, text: str) -> None:
        self.text = text
    def render(self) -> str:
        return self.text

class Button:
    def render(self) -> str:
        return "[button]"

def display(items: list[Renderable]) -> None:
    for item in items:
        print(item.render())

display([Label("Hi"), Button()])  # OK -- both have render()
```

Neither `Label` nor `Button` inherits from `Renderable`. They satisfy it structurally.

### Protocols with Properties

```python
class HasID(Protocol):
    @property
    def id(self) -> str: ...
```

```python
class HasID(Protocol):
    id: str  # read-only if no setter defined
```

### Generic Protocols

```python
from typing import Protocol, TypeVar

T = TypeVar("T")

class Comparable(Protocol[T]):
    def compare(self, other: T) -> int: ...
```

### runtime_checkable

```python
from typing import Protocol, runtime_checkable

@runtime_checkable
class HasName(Protocol):
    name: str

if isinstance(obj, HasName):  # checks attribute presence, NOT types
    print(obj.name)
```

Runtime checks only verify attributes are present. They cannot check method signatures match. Use sparingly.

### When to Use Protocol

- Dependency injection with interchangeable implementations.
- Third-party types you cannot modify but want to accept structurally.
- Defining what a function actually uses from its parameters (the Go-style "accept the smallest interface").

### When NOT to Use Protocol

- When you want to force inheritance for registry/discovery. Use ABC.
- When the shape is pure data (no methods). Use TypedDict or dataclass.

---

## 4. ABC (Nominal Typing)

```python
from abc import ABC, abstractmethod

class Notifier(ABC):
    @abstractmethod
    def send(self, message: str) -> None:
        ...

    def send_many(self, messages: list[str]) -> None:
        for m in messages:
            self.send(m)

class EmailNotifier(Notifier):
    def send(self, message: str) -> None:
        smtp.send(message)
```

`ABC` enforces that `EmailNotifier` implements `send`. You cannot instantiate `Notifier` directly; mypy and the runtime both complain.

### When to Use ABC

- Plugin systems where you enumerate subclasses: `Notifier.__subclasses__()`.
- Abstract base classes with shared implementations (like `send_many` above).
- When you want nominal typing -- callers explicitly subclass to signal intent.

---

## 5. TypedDict

For JSON payloads and configuration dicts.

```python
from typing import TypedDict, NotRequired

class UserData(TypedDict):
    id: str
    name: str
    email: str
    phone: NotRequired[str]  # optional field

u: UserData = {"id": "1", "name": "Alice", "email": "a@example.com"}
```

### TypedDict Inheritance

```python
class BaseEvent(TypedDict):
    event_id: str
    timestamp: int

class OrderPlaced(BaseEvent):
    order_id: str
    total_cents: int
```

`OrderPlaced` has all three fields of `BaseEvent` plus its own.

### total=False

```python
class Config(TypedDict, total=False):
    host: str
    port: int
    timeout: float

# All fields become optional
c: Config = {"host": "localhost"}
```

Prefer `NotRequired` (3.11+) for per-field control.

### TypedDict vs dataclass

| Property | TypedDict | dataclass |
|----------|-----------|-----------|
| Runtime type | `dict` | custom class |
| Methods | No | Yes |
| Validation | No | No (use pydantic) |
| Inheritance | Yes | Yes |
| Mutation | Mutable | `frozen=True` option |
| JSON-friendly | Yes | Via `asdict` |
| Memory | Higher (dict overhead) | Lower with `slots=True` |

TypedDict is a dict at runtime. dataclass is an object. Choose based on what you need: TypedDict for interop (JSON in/out), dataclass for in-code records.

---

## 6. Generics

### TypeVar

```python
from typing import TypeVar

T = TypeVar("T")

def first_or_default(items: list[T], default: T) -> T:
    return items[0] if items else default

x: int = first_or_default([1, 2, 3], 0)
y: str = first_or_default(["a", "b"], "")
```

### Bounded TypeVar

```python
from typing import TypeVar
from dataclasses import dataclass

@dataclass
class Named:
    name: str

NamedT = TypeVar("NamedT", bound=Named)

def name_of(obj: NamedT) -> str:
    return obj.name
```

`NamedT` accepts anything that is-a Named. The return type preserves the specific subclass.

### Constrained TypeVar

```python
StrOrBytes = TypeVar("StrOrBytes", str, bytes)  # ONLY str or bytes

def concat(a: StrOrBytes, b: StrOrBytes) -> StrOrBytes:
    return a + b

concat("a", "b")    # OK, returns str
concat(b"a", b"b")  # OK, returns bytes
concat("a", b"b")   # TYPE ERROR
```

Bounded TypeVars are a supertype constraint; constrained TypeVars are an exact-match constraint.

### Generic Classes (3.12+)

```python
# Python 3.12+ syntax
class Stack[T]:
    def __init__(self) -> None:
        self._items: list[T] = []

    def push(self, x: T) -> None:
        self._items.append(x)

    def pop(self) -> T:
        return self._items.pop()

s: Stack[int] = Stack()
s.push(1)
```

Pre-3.12:

```python
from typing import Generic, TypeVar

T = TypeVar("T")

class Stack(Generic[T]):
    def __init__(self) -> None:
        self._items: list[T] = []
```

### Variadic Generics (TypeVarTuple, 3.11+)

```python
from typing import TypeVarTuple

Ts = TypeVarTuple("Ts")

def stack(*arrays: *Ts) -> tuple[*Ts]:
    return tuple(arrays)

x: tuple[int, str, float] = stack(1, "a", 3.14)
```

Useful for numerical libraries where shapes matter.

---

## 7. ParamSpec (Decorator Typing)

The reason decorators used to lose type information.

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
def fetch(url: str, timeout: float = 30.0) -> bytes:
    ...

# Type checkers know fetch still takes (url: str, timeout: float) -> bytes
data: bytes = fetch("https://example", timeout=10.0)
```

### ParamSpec.args and ParamSpec.kwargs

```python
def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
    ...
```

These are special forms. `args` captures positional arguments; `kwargs` captures keyword arguments. Both must appear together in the wrapper.

### Concatenate (Adding Parameters)

```python
from typing import Concatenate, ParamSpec, TypeVar, Callable

P = ParamSpec("P")
R = TypeVar("R")

def with_context(func: Callable[Concatenate[Context, P], R]) -> Callable[P, R]:
    ctx = build_context()
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
        return func(ctx, *args, **kwargs)
    return wrapper

@with_context
def handler(ctx: Context, user_id: str) -> Response:
    ...

# Callable becomes (user_id: str) -> Response
```

---

## 8. Literal

Useful for string enums without the overhead of `Enum`.

```python
from typing import Literal

Direction = Literal["north", "south", "east", "west"]

def move(d: Direction) -> None: ...

move("north")   # OK
move("up")      # TYPE ERROR
```

### Literal for Flags

```python
def open_file(path: str, mode: Literal["r", "w", "a", "rb", "wb"]) -> File: ...
```

### Literal + overload

```python
from typing import Literal, overload

@overload
def parse(data: str, format: Literal["json"]) -> dict: ...
@overload
def parse(data: str, format: Literal["yaml"]) -> list: ...

def parse(data: str, format: str) -> dict | list:
    ...
```

---

## 9. Self (3.11+)

For fluent APIs.

```python
from typing import Self

class QueryBuilder:
    def __init__(self) -> None:
        self._table: str | None = None
        self._filters: list[str] = []

    def from_(self, table: str) -> Self:
        self._table = table
        return self

    def where(self, cond: str) -> Self:
        self._filters.append(cond)
        return self

q: QueryBuilder = QueryBuilder().from_("orders").where("status = 'new'")
```

Subclasses automatically get the right return type. Before `Self`, you needed TypeVars bound to the class.

---

## 10. Final

```python
from typing import Final

API_URL: Final = "https://api.example.com"
```

The type checker forbids reassignment. Useful for module-level constants.

```python
class Config:
    max_size: Final[int] = 1000
```

---

## 11. Union and Discriminated Unions

```python
from dataclasses import dataclass
from typing import Literal

@dataclass
class Success:
    kind: Literal["success"]
    value: int

@dataclass
class Failure:
    kind: Literal["failure"]
    error: str

Result = Success | Failure

def handle(r: Result) -> None:
    match r.kind:
        case "success":
            # mypy knows r is Success here
            print(r.value)
        case "failure":
            # mypy knows r is Failure here
            print(r.error)
```

The `kind: Literal[...]` field is the discriminator. mypy narrows the type automatically.

---

## 12. NewType

Brand a primitive type to prevent mixups.

```python
from typing import NewType

UserID = NewType("UserID", str)
OrderID = NewType("OrderID", str)

def get_order(order_id: OrderID) -> Order: ...

uid = UserID("abc")
oid = OrderID("xyz")

get_order(oid)  # OK
get_order(uid)  # TYPE ERROR -- UserID is not OrderID
get_order("xyz")  # TYPE ERROR -- str is not OrderID
```

Runtime cost: zero. `UserID("abc")` is just `"abc"`.

---

## 13. TypeAlias (3.10+) and `type` Statement (3.12+)

```python
from typing import TypeAlias

# 3.10+
UserID: TypeAlias = str

# 3.12+
type UserID = str
```

The `type` statement is cleaner and is the future. `TypeAlias` is a placeholder to signal that this is an alias, not a reassignment.

For parameterized aliases:

```python
# 3.12+
type Stack[T] = list[T]

s: Stack[int] = [1, 2, 3]
```

---

## 14. cast and type: ignore

```python
from typing import cast

raw: object = json.loads(data)
orders = cast(list[Order], raw)
```

`cast` tells mypy "trust me" with no runtime cost. Use when you've genuinely validated the shape and the type system can't express the check.

```python
result = some_ambiguous_call()  # type: ignore[return-value]
```

`# type: ignore` is an escape hatch. Always specify the error code (e.g., `return-value`) so you don't silence unrelated errors by accident.

Rule of thumb: if you need `cast` or `type: ignore` more than once per hundred lines, your types are wrong.

---

## 15. Type-Only Imports

Imports only needed for type hints go under `TYPE_CHECKING`:

```python
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from mypackage.heavy import HeavyObject
    from mypackage.orders import Order

def process(o: "Order", h: "HeavyObject") -> None:
    ...
```

The quoted type names are resolved by mypy but not at runtime. Benefits:

- Avoids circular imports.
- Avoids loading heavy modules at startup.
- Keeps imports honest -- you can see what actually runs vs what's only for types.

3.10+ can use `from __future__ import annotations` at the top of the file to get implicit string annotations everywhere:

```python
from __future__ import annotations

def process(o: Order, h: HeavyObject) -> None:
    ...
```

All annotations become strings at runtime. mypy is unaffected. This is broadly the recommended pattern.

---

## 16. Type Narrowing

mypy tracks type narrowing through control flow.

```python
def process(x: int | None) -> int:
    if x is None:
        return 0
    return x * 2  # x is int here

def describe(x: object) -> str:
    if isinstance(x, str):
        return x.upper()  # x is str here
    if isinstance(x, list):
        return f"list of {len(x)}"  # x is list here
    return repr(x)
```

### assert_never for Exhaustiveness

```python
from typing import assert_never, Literal

Status = Literal["pending", "done", "failed"]

def describe(s: Status) -> str:
    match s:
        case "pending":
            return "in progress"
        case "done":
            return "complete"
        case "failed":
            return "failed"
        case _:
            assert_never(s)  # compile-time error if Status has more values
```

If you add `"cancelled"` to `Status`, mypy will error on `assert_never(s)` because it can prove `s` might be `"cancelled"`.

---

## 17. Common Mypy Errors and Fixes

| Error | Meaning | Fix |
|-------|---------|-----|
| `Item "None" of "X | None" has no attribute "y"` | Accessing attr on possibly-None | `if x: x.y` or `assert x is not None` |
| `Argument 1 has incompatible type` | Type mismatch | Check the types; add cast if deliberately narrowing |
| `Call to untyped function` | Calling something without annotations | Annotate the function or cast the call |
| `Returning Any from function declared to return T` | Reflection leaked | Narrow via `cast` or type guard |
| `Missing return statement` | Missing return on some path | Add `raise` or explicit return |
| `Cannot assign to final name` | Reassigning a Final | Remove the reassignment |

---

## 18. Cross-References

- [asyncio-pitfalls.md](asyncio-pitfalls.md) -- typing of async functions
- [pytest-patterns.md](pytest-patterns.md) -- typing fixtures and mocks
- [full-guide.md](full-guide.md) -- overall Python discipline
- [gil-and-perf.md](gil-and-perf.md) -- typing around multiprocessing
