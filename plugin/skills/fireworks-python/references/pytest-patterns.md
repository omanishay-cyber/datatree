# fireworks-python -- Pytest Patterns

> Fixtures, parametrize, monkeypatch, async tests, mocks.
> Everything you need to write production-grade Python tests.

---

## 1. Test Discovery

Pytest automatically discovers tests by convention:

- Files matching `test_*.py` or `*_test.py`.
- Functions starting with `test_`.
- Classes starting with `Test` (no `__init__`), methods starting with `test_`.

```
tests/
  conftest.py           # shared fixtures
  test_orders.py        # test functions
  test_api.py
  integration/
    conftest.py
    test_full_flow.py
```

### Configuration

```toml
[tool.pytest.ini_options]
testpaths = ["tests"]
python_files = ["test_*.py"]
python_classes = ["Test*"]
python_functions = ["test_*"]
asyncio_mode = "auto"
addopts = [
    "-v",                    # verbose
    "--tb=short",            # short tracebacks
    "--strict-markers",      # error on unknown markers
    "--strict-config",       # error on unknown config
]
markers = [
    "slow: marks tests as slow (deselect with -m 'not slow')",
    "integration: requires external services",
]
```

### Running Tests

```bash
pytest                        # all tests
pytest tests/test_orders.py   # one file
pytest tests/test_orders.py::test_submit  # one test
pytest -k "submit"            # by name pattern
pytest -m "not slow"          # by marker
pytest -x                     # stop on first failure
pytest --lf                   # last failed only
pytest --ff                   # failed first, then rest
pytest -n auto                # parallel (pytest-xdist)
```

---

## 2. Fixtures: The Core Mechanism

Fixtures are pytest's dependency injection. You declare a fixture; pytest injects it into tests that name it as a parameter.

```python
import pytest

@pytest.fixture
def order() -> Order:
    return Order(id="abc", total_cents=1000)

def test_submit_accepts_order(order: Order) -> None:
    assert submit(order) is True
```

### Scopes

```python
@pytest.fixture(scope="function")   # default -- new instance per test
@pytest.fixture(scope="class")      # one per test class
@pytest.fixture(scope="module")     # one per file
@pytest.fixture(scope="session")    # one per test run
```

```python
@pytest.fixture(scope="session")
def db_engine():
    engine = create_engine("sqlite:///:memory:")
    Base.metadata.create_all(engine)
    yield engine
    engine.dispose()

@pytest.fixture
def session(db_engine):
    with Session(db_engine) as s:
        yield s
        s.rollback()
```

Use `session` scope for expensive setup (Docker containers, DB engines). Use `function` scope (default) for anything with state that leaks between tests.

### Setup and Teardown with yield

```python
@pytest.fixture
def temp_dir():
    path = Path(tempfile.mkdtemp())
    yield path
    shutil.rmtree(path)
```

Code before `yield` is setup. Code after is teardown. Always use `yield` (not `return`) when you need cleanup.

Teardown runs even if the test fails. Pytest wraps this in a try/finally internally.

### Fixture Composition

Fixtures can depend on other fixtures:

```python
@pytest.fixture
def db_engine():
    return create_engine("sqlite:///:memory:")

@pytest.fixture
def order_repo(db_engine):
    return OrderRepository(db_engine)

@pytest.fixture
def order_service(order_repo, fake_notifier):
    return OrderService(order_repo, fake_notifier)

def test_something(order_service):
    order_service.submit(...)
```

Pytest wires them in the right order.

### conftest.py: Shared Fixtures

Fixtures defined in `conftest.py` are available to all tests in the same directory and subdirectories without import.

```python
# tests/conftest.py
import pytest

@pytest.fixture
def order() -> Order:
    return Order(id="test", total_cents=100)
```

```python
# tests/test_any.py -- no import needed
def test_submit(order: Order) -> None:
    ...
```

Nested conftest.py files compose: deeper directories inherit from outer ones and can override.

---

## 3. Parametrize: Data-Driven Tests

```python
@pytest.mark.parametrize("input,expected", [
    ("ord_abc", "abc"),
    ("ord_xyz", "xyz"),
    ("", None),
    ("no_prefix", None),
])
def test_parse_id(input: str, expected: str | None) -> None:
    assert parse_order_id(input) == expected
```

Each tuple becomes a separate test case. Output:

```
test_parse_id[ord_abc-abc] PASSED
test_parse_id[ord_xyz-xyz] PASSED
test_parse_id[-None] PASSED
test_parse_id[no_prefix-None] PASSED
```

### Named Cases with pytest.param

```python
@pytest.mark.parametrize("input,expected", [
    pytest.param("ord_abc", "abc", id="valid prefix"),
    pytest.param("", None, id="empty"),
    pytest.param("no_prefix", None, id="missing prefix", marks=pytest.mark.xfail(reason="known bug")),
])
def test_parse_id(input: str, expected: str | None) -> None:
    ...
```

Named cases are self-documenting. Failure reports read "valid prefix FAILED" instead of "ord_abc-abc FAILED".

### Multiple parametrize

```python
@pytest.mark.parametrize("role", ["admin", "user"])
@pytest.mark.parametrize("action", ["read", "write"])
def test_permissions(role: str, action: str) -> None:
    # runs 4 times: admin-read, admin-write, user-read, user-write
    ...
```

Stacked parametrize decorators produce the Cartesian product.

### Parametrize Fixtures

```python
@pytest.fixture(params=["sqlite", "postgres"])
def db(request):
    if request.param == "sqlite":
        return sqlite_setup()
    return postgres_setup()

def test_db_operation(db):
    # runs twice, once per backend
    ...
```

---

## 4. Async Tests (pytest-asyncio)

### Setup

```toml
[tool.pytest.ini_options]
asyncio_mode = "auto"  # all async functions are treated as async tests
```

Without `auto`, you must mark each test:

```python
@pytest.mark.asyncio
async def test_fetch() -> None:
    ...
```

### Async Fixtures

```python
@pytest.fixture
async def http_client() -> AsyncIterator[httpx.AsyncClient]:
    async with httpx.AsyncClient() as client:
        yield client

@pytest.mark.asyncio
async def test_get(http_client: httpx.AsyncClient) -> None:
    r = await http_client.get("https://example.com")
    assert r.status_code == 200
```

### Async Fixtures with Setup/Teardown

```python
@pytest.fixture
async def db_session() -> AsyncIterator[AsyncSession]:
    async with SessionLocal() as session:
        yield session
        await session.rollback()
```

### AsyncMock

```python
from unittest.mock import AsyncMock, MagicMock

async def test_with_async_mock():
    mock = AsyncMock()
    mock.fetch.return_value = {"status": "ok"}

    result = await mock.fetch("https://example.com")

    assert result == {"status": "ok"}
    mock.fetch.assert_awaited_once_with("https://example.com")
```

`AsyncMock` returns coroutines from mocked methods. The assertions (`assert_awaited_*`) check the coroutine was actually awaited.

---

## 5. monkeypatch

Pytest's cleaner alternative to `mock.patch` for simple substitutions.

### Environment Variables

```python
def test_with_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("API_URL", "https://test.example")
    monkeypatch.delenv("OPTIONAL_VAR", raising=False)

    config = load_config()
    assert config.api_url == "https://test.example"
```

### Attribute Patching

```python
def test_with_fake_time(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        "mymodule.datetime",
        lambda: datetime(2026, 1, 1, tzinfo=UTC),
    )
    # anything inside mymodule that calls datetime() gets the fake
```

### Module Attribute

```python
def test_with_fake_requests(monkeypatch: pytest.MonkeyPatch) -> None:
    def fake_get(url, **kwargs):
        return Mock(status_code=200, json=lambda: {"ok": True})

    monkeypatch.setattr("requests.get", fake_get)

    result = fetch_user(1)
    assert result["ok"] is True
```

### Working Directory

```python
def test_reads_cwd(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.chdir(tmp_path)
    # code that uses cwd sees tmp_path
```

monkeypatch auto-reverts at test end. Easier than `try/finally`.

---

## 6. Built-in Fixtures

### tmp_path and tmp_path_factory

```python
def test_writes_file(tmp_path: Path) -> None:
    file = tmp_path / "output.txt"
    write_report(file)
    assert file.read_text().startswith("Report")
```

`tmp_path` is a unique directory per test, cleaned up at end. `tmp_path_factory` is session-scoped.

### capsys and caplog

```python
def test_prints_message(capsys: pytest.CaptureFixture[str]) -> None:
    greet("Alice")
    captured = capsys.readouterr()
    assert captured.out == "Hello, Alice\n"

def test_logs_warning(caplog: pytest.LogCaptureFixture) -> None:
    with caplog.at_level(logging.WARNING):
        risky_operation()
    assert "retrying" in caplog.text
```

### request

Meta-fixture giving the current test's context:

```python
@pytest.fixture
def configured_client(request: pytest.FixtureRequest):
    marker = request.node.get_closest_marker("auth")
    token = marker.args[0] if marker else "default"
    return Client(token=token)

@pytest.mark.auth("admin-token")
def test_admin_action(configured_client):
    ...
```

---

## 7. Mocking Patterns

### Mock vs MagicMock vs AsyncMock

```python
from unittest.mock import Mock, MagicMock, AsyncMock

Mock()         # basic mock, calls return Mock objects
MagicMock()    # adds magic methods (__len__, __iter__, etc.)
AsyncMock()    # async-friendly; calls return coroutines
```

Use `MagicMock` by default; `Mock` is too minimal for most cases.

### patch vs patch.object

```python
# patch: string path to the target
with patch("mymodule.requests.get") as mock_get:
    mock_get.return_value.json.return_value = {"ok": True}
    result = fetch_user(1)

# patch.object: object + attr name
with patch.object(myservice, "fetch") as mock_fetch:
    mock_fetch.return_value = {"ok": True}
    result = use_service()
```

Prefer `patch.object` when you have a reference to the object. It's less brittle than string paths.

### Patch at the Import Site

```python
# mymodule.py
import requests

def fetch(url: str) -> dict:
    return requests.get(url).json()
```

```python
# test_mymodule.py
def test_fetch():
    with patch("mymodule.requests.get") as mock_get:  # patch in mymodule, not in requests
        ...
```

Rule: patch where the symbol is used, not where it's defined. This is one of the most common mocking mistakes.

### spec for Type Safety

```python
from unittest.mock import Mock

class Notifier:
    def send(self, msg: str) -> None: ...
    def broadcast(self, msg: str) -> None: ...

def test_submit() -> None:
    notifier = Mock(spec=Notifier)
    submit(order, notifier)

    notifier.send.assert_called_once_with("submitted")
    # notifier.nonexistent_method() would raise AttributeError
```

`spec=Notifier` limits the mock to the class's interface. Typos in tests fail at mock creation instead of silently succeeding.

### Builder Pattern for Mocks

```python
def make_fake_order(id: str = "abc", total: int = 100) -> Mock:
    order = Mock(spec=Order)
    order.id = id
    order.total_cents = total
    return order

def test_submit() -> None:
    order = make_fake_order(total=5000)
    ...
```

Reduces test-setup boilerplate for complex objects.

---

## 8. Testing Exceptions

```python
import pytest

def test_raises_on_bad_id() -> None:
    with pytest.raises(ValueError, match="invalid id"):
        parse_order_id("")
```

The `match` is a regex against `str(exc)`. Use it to assert the error message, not just the type.

### Capturing the Exception

```python
def test_exception_details() -> None:
    with pytest.raises(ValidationError) as exc_info:
        Order.model_validate({"total_cents": -1})

    assert exc_info.value.errors()[0]["loc"] == ("total_cents",)
```

---

## 9. Custom Markers

```python
# pyproject.toml
[tool.pytest.ini_options]
markers = [
    "slow: marks tests that take >1 second",
    "integration: requires external services",
    "flaky: retries on failure (unreliable infrastructure)",
]
```

```python
@pytest.mark.slow
def test_big_import() -> None:
    ...

@pytest.mark.integration
def test_real_db() -> None:
    ...
```

```bash
pytest -m "not slow"       # skip slow tests
pytest -m "integration"    # only integration
```

---

## 10. Coverage

```toml
[tool.coverage.run]
source = ["src/mypackage"]
branch = true
omit = [
    "*/tests/*",
    "*/__main__.py",
]

[tool.coverage.report]
fail_under = 80
show_missing = true
skip_covered = false
exclude_lines = [
    "pragma: no cover",
    "raise NotImplementedError",
    "if TYPE_CHECKING:",
    "if __name__ == .__main__.:",
]
```

```bash
pytest --cov=mypackage --cov-report=html --cov-report=term-missing
```

Branch coverage (vs line coverage) tracks whether both sides of every `if` were taken. Always enable it.

---

## 11. Property-Based Testing (hypothesis)

For invariants, not examples. The library generates inputs; you describe the property.

```python
from hypothesis import given, strategies as st

@given(
    items=st.lists(st.integers()),
)
def test_sort_is_idempotent(items: list[int]) -> None:
    assert sorted(sorted(items)) == sorted(items)

@given(
    name=st.text(min_size=1, max_size=64).filter(str.isprintable),
)
def test_greet_accepts_valid_names(name: str) -> None:
    result = greet(name)
    assert isinstance(result, str)
    assert name in result
```

When hypothesis finds a failing case, it shrinks to the smallest reproducer and saves it to `.hypothesis/` for future runs.

Use hypothesis for parsers, serializers, validators, invariant-heavy pure functions. Not for business logic with specific scenarios.

---

## 12. Testcontainers for Integration

```python
from testcontainers.postgres import PostgresContainer

@pytest.fixture(scope="session")
def pg_container():
    with PostgresContainer("postgres:16") as pg:
        yield pg

@pytest.fixture
def db_url(pg_container):
    return pg_container.get_connection_url()
```

Spins up a real Postgres in Docker for the test session. Slow but realistic.

---

## 13. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `time.sleep` in tests | Use fake clock or async waiting |
| Test depends on previous test's state | Use fixtures; each test isolates |
| Snapshot tests for dynamic content | Golden-file or targeted assertions |
| Testing implementation (internal attrs) | Test public behaviour |
| Single mega-test with 30 assertions | Split into parametrized cases |
| Mocking what you're testing | Test the real thing; mock its deps |
| `assert True` placeholders | Skip or implement |
| Committing `.pytest_cache` | gitignore it |
| Ignored test files via leading underscore | Delete or fix, don't hide |
| Print statements for debugging | Use `-s` flag or logging + caplog |
| Overly broad `except Exception` in tests | Let exceptions propagate to pytest |
| `time.sleep(1)` to wait for async result | Use `await` or explicit polling |

---

## 14. Test Quality Checklist

Before merging a test file, verify:

- [ ] Every public function has at least 3 tests (happy, edge, error).
- [ ] Tests are independent -- any one can be run alone.
- [ ] Fixtures are properly scoped (session vs function).
- [ ] No time.sleep; async waits use `await` or polling with timeout.
- [ ] Assertions are specific (not `assert result`).
- [ ] Mocks use `spec=` for type safety.
- [ ] Error messages are asserted via `match=`, not just types.
- [ ] Coverage is >= 80% on the lines that were changed.
- [ ] Tests run cleanly under `pytest -p no:cacheprovider -x`.

---

## 15. Cross-References

- [asyncio-pitfalls.md](asyncio-pitfalls.md) -- async test patterns
- [typing-deep-dive.md](typing-deep-dive.md) -- typing fixtures and mocks
- [full-guide.md](full-guide.md) -- general Python discipline
- [packaging-modern.md](packaging-modern.md) -- pytest config in pyproject.toml
