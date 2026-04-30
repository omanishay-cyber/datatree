# fireworks-go -- Testing Reference

> Table-driven tests, subtests, fuzzing, benchmarks, golden files,
> httptest, mocks. The full Go testing toolkit.

---

## 1. Test File Layout

```
package_dir/
  service.go
  service_test.go     # Same package -- white-box testing
  service_export_test.go  # Optional -- export internals for testing
  testdata/           # Fixtures (golden files, sample inputs)
    happy.json
    error.golden
```

Three rules:

- Test files end in `_test.go`.
- `_test.go` files are excluded from the regular build.
- `testdata/` is a magic directory name -- the toolchain ignores it for vet, build, etc.

### White-Box vs Black-Box

```go
// service.go
package orders

func processInternal(...) {...}

// service_test.go -- same package, can access processInternal
package orders

func TestProcessInternal(t *testing.T) { ... }

// service_external_test.go -- external package, only public API
package orders_test

import "github.com/example/orders"

func TestPublicAPI(t *testing.T) {
    orders.Submit(...)
}
```

Use `_test` suffix when you want to test only the public API. This catches accidental coupling to internals.

---

## 2. Table-Driven Tests

The fundamental Go test idiom.

```go
func TestParseOrderID(t *testing.T) {
    tests := []struct {
        name    string
        input   string
        want    string
        wantErr error
    }{
        {"valid prefixed", "ord_abc123", "abc123", nil},
        {"empty input", "", "", ErrEmpty},
        {"missing prefix", "abc123", "", ErrBadFormat},
        {"too long", strings.Repeat("a", 65), "", ErrTooLong},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got, err := ParseOrderID(tt.input)
            if !errors.Is(err, tt.wantErr) {
                t.Fatalf("err = %v, want %v", err, tt.wantErr)
            }
            if got != tt.want {
                t.Errorf("got %q, want %q", got, tt.want)
            }
        })
    }
}
```

Why this is the standard:

1. Adding a case is one line.
2. Each case runs as a subtest -- failures show which case failed.
3. You can run a single case: `go test -run TestParseOrderID/valid_prefixed`.
4. The structure is uniform across the codebase, so reviewers know what to look for.

### Naming the Cases

Use space-separated lowercase. Subtest names are slugified by the toolchain (spaces become underscores), so the CLI form is `TestX/some_case_name`.

### t.Run Quirks

- Each `t.Run` runs sequentially by default. Add `t.Parallel()` inside the subtest body to run cases concurrently.
- A `t.Fatalf` inside a subtest stops only that subtest. Other cases still run.

---

## 3. The Three Required Cases

Every public function gets at least three tests:

1. **Happy path** -- the common, valid input.
2. **Edge case** -- empty input, max-size input, boundary values.
3. **Error case** -- invalid input, dependency failure.

If you have only one of these, your test suite is incomplete.

```go
func TestDivide(t *testing.T) {
    tests := []struct {
        name        string
        numerator   int
        denominator int
        want        int
        wantErr     error
    }{
        // Happy
        {"positive divides cleanly", 10, 2, 5, nil},
        // Edge
        {"divides into zero", 0, 5, 0, nil},
        {"negative numerator", -10, 2, -5, nil},
        // Error
        {"divide by zero", 10, 0, 0, ErrDivByZero},
    }
    ...
}
```

---

## 4. testify -- Use Sparingly

`stretchr/testify` is popular. The Go community is split. Position from this skill:

- Use the standard library by default. `t.Errorf` and `errors.Is` cover most cases.
- Reach for `testify/require` only when you have genuinely complex equality (deep struct compare, timing tolerance, slice unordered compare).
- Never use `testify/suite`. It mimics xUnit and is non-idiomatic in Go.

### When testify earns its keep

```go
import "github.com/stretchr/testify/require"

require.ElementsMatch(t, expected, actual)  // unordered slice equality
require.WithinDuration(t, t1, t2, time.Second)  // time tolerance
require.Equal(t, expectedStruct, actualStruct)  // deep struct equality
```

For everything else, the stdlib is fine.

---

## 5. Test Helpers

```go
func setupRepo(t *testing.T) *OrderRepo {
    t.Helper()
    db, err := sql.Open("sqlite3", ":memory:")
    if err != nil {
        t.Fatalf("open db: %v", err)
    }
    t.Cleanup(func() { db.Close() })

    if _, err := db.Exec(schema); err != nil {
        t.Fatalf("apply schema: %v", err)
    }
    return NewOrderRepo(db)
}

func TestRepoSave(t *testing.T) {
    repo := setupRepo(t)
    ...
}
```

Two patterns to memorise:

- `t.Helper()` -- when this helper fails, the failure is reported at the caller's line, not inside the helper. Always call it on the first line.
- `t.Cleanup` -- registers a function that runs when the test ends. Cleaner than defer because it composes through helpers.

---

## 6. Subtest Parallelism

```go
func TestStuff(t *testing.T) {
    tests := []struct{ ... }{...}
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            t.Parallel()
            // assertions
        })
    }
}
```

`t.Parallel()` does two things:

1. Defers this subtest until its siblings have started.
2. Runs subtests concurrently within the same package.

Catches data races between tests that share state -- which they shouldn't.

---

## 7. Fuzzing (Go 1.18+)

```go
func FuzzParseOrderID(f *testing.F) {
    seeds := []string{"ord_abc", "abc", "", "ord_a"}
    for _, s := range seeds {
        f.Add(s)
    }
    f.Fuzz(func(t *testing.T, input string) {
        got, err := ParseOrderID(input)
        if err != nil {
            return // expected for many inputs
        }
        if !strings.HasPrefix(input, "ord_") {
            t.Errorf("accepted %q without prefix", input)
        }
        if got != input[4:] {
            t.Errorf("ParseOrderID(%q) = %q, want %q", input, got, input[4:])
        }
    })
}
```

```bash
go test -fuzz FuzzParseOrderID
```

Fuzz runs forever (or until you Ctrl-C) generating random inputs. When it finds a failing case, it saves it to `testdata/fuzz/FuzzParseOrderID/` so subsequent regular `go test` runs include it.

Fuzzing is high-leverage for parsers, validators, and anything that takes untrusted input. It is not the right tool for business logic with structured inputs.

---

## 8. Benchmarks

```go
func BenchmarkParseOrderID(b *testing.B) {
    input := "ord_abc123def456"
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        _, _ = ParseOrderID(input)
    }
}
```

```bash
go test -bench=. -benchmem
```

Output:

```
BenchmarkParseOrderID-8    50000000    24.3 ns/op    8 B/op    1 allocs/op
```

Read this as: 50M iterations, 24.3 nanoseconds per call, 8 bytes allocated per call, 1 allocation per call.

### Benchmark Rules

- Set up outside the timer loop. Call `b.ResetTimer()` after setup.
- Use `b.ReportAllocs()` for any benchmark that touches the heap.
- Use realistic inputs. Benchmarks of `f(0)` are not useful.
- Compare benchmarks against a baseline. Use `benchstat` to detect significant regressions.

```bash
go test -bench=. -count=10 > old.txt
# make change
go test -bench=. -count=10 > new.txt
benchstat old.txt new.txt
```

`benchstat` reports the geometric mean and statistical significance of the difference.

---

## 9. Golden Files

For tests where the expected output is large or structured, store it in `testdata/`.

```go
func TestRender(t *testing.T) {
    got, err := Render(input)
    if err != nil {
        t.Fatalf("render: %v", err)
    }

    goldenPath := "testdata/render.golden"
    if *update {
        if err := os.WriteFile(goldenPath, got, 0644); err != nil {
            t.Fatalf("update golden: %v", err)
        }
    }

    want, err := os.ReadFile(goldenPath)
    if err != nil {
        t.Fatalf("read golden: %v", err)
    }
    if !bytes.Equal(got, want) {
        t.Errorf("render mismatch:\ngot:\n%s\nwant:\n%s", got, want)
    }
}

var update = flag.Bool("update", false, "update golden files")
```

Run `go test -update` to regenerate. Always review the diff before committing.

---

## 10. httptest

For testing HTTP servers without binding a real port.

```go
func TestGetOrder(t *testing.T) {
    h := NewHandler(setupRepo(t))
    srv := httptest.NewServer(h)
    defer srv.Close()

    resp, err := http.Get(srv.URL + "/orders/abc123")
    if err != nil {
        t.Fatalf("GET: %v", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != 200 {
        t.Fatalf("status = %d, want 200", resp.StatusCode)
    }
    var got Order
    if err := json.NewDecoder(resp.Body).Decode(&got); err != nil {
        t.Fatalf("decode: %v", err)
    }
    // assert on got
}
```

For unit testing a single handler without a server:

```go
req := httptest.NewRequest("GET", "/orders/abc123", nil)
rec := httptest.NewRecorder()
h.ServeHTTP(rec, req)
if rec.Code != 200 {
    t.Fatalf("status = %d", rec.Code)
}
```

`httptest.NewRecorder` is faster than spinning up a server. Use it when you don't need full HTTP semantics.

---

## 11. Mocking

Go's interface satisfaction means you mock by implementing an interface.

```go
type Notifier interface {
    Send(ctx context.Context, msg string) error
}

type fakeNotifier struct {
    sent []string
    err  error
}

func (f *fakeNotifier) Send(ctx context.Context, msg string) error {
    if f.err != nil {
        return f.err
    }
    f.sent = append(f.sent, msg)
    return nil
}

func TestSubmitNotifies(t *testing.T) {
    notifier := &fakeNotifier{}
    svc := NewOrderService(repo, notifier)

    if err := svc.Submit(ctx, order); err != nil {
        t.Fatalf("submit: %v", err)
    }
    if len(notifier.sent) != 1 {
        t.Errorf("notifier sent %d, want 1", len(notifier.sent))
    }
}
```

### Mock Generators

If your interfaces are large or you have many of them:

- `mockgen` (`go.uber.org/mock`) -- generates mocks from interfaces with assertion helpers.
- `counterfeiter` -- generates mocks with hooks like `WasCalled`, `CallCount`.

For small interfaces, hand-rolled fakes are usually clearer and faster.

---

## 12. Test Coverage

```bash
go test -cover ./...
go test -coverprofile=cover.out ./...
go tool cover -html=cover.out
```

Coverage targets are not a goal. They are a signal. Aim for:

- 80% on packages where bugs are expensive (auth, billing, data store).
- 60% on packages where bugs are easy to detect at runtime.
- Don't aim for 100% -- the last 10% is usually unreachable error paths whose tests would be more brittle than valuable.

---

## 13. Integration Tests

Mark slow tests with a build tag:

```go
//go:build integration

package store_test

func TestRealPostgres(t *testing.T) {
    // requires PostgreSQL running
}
```

Run with:

```bash
go test ./...                       # unit tests only
go test -tags integration ./...     # all tests
```

In CI, run unit tests first (fast), integration tests separately (slow).

---

## 14. testcontainers-go

For integration tests against real services without managing them yourself:

```go
import "github.com/testcontainers/testcontainers-go"

func TestRealPostgres(t *testing.T) {
    ctx := context.Background()
    pg, err := postgres.RunContainer(ctx,
        postgres.WithDatabase("test"),
        postgres.WithUsername("user"),
        postgres.WithPassword("pass"),
    )
    if err != nil { t.Fatal(err) }
    t.Cleanup(func() { pg.Terminate(ctx) })

    connStr, err := pg.ConnectionString(ctx, "sslmode=disable")
    if err != nil { t.Fatal(err) }
    db, err := sql.Open("postgres", connStr)
    if err != nil { t.Fatal(err) }
    // run tests against db
}
```

Spins up a real Postgres in Docker, tears it down at end. Slow but realistic.

---

## 15. goleak (Goroutine Leak Detection)

```go
import "go.uber.org/goleak"

func TestMain(m *testing.M) {
    goleak.VerifyTestMain(m)
}
```

Snapshots goroutines at test start and asserts they're all gone at end. Catches leaks at PR review time, not in production.

---

## 16. Race Detector

```bash
go test -race ./...
```

Enable in CI. Always. The runtime cost during tests is acceptable; the cost of a production race is not.

---

## 17. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `time.Sleep` in tests for "wait for thing" | Use channels or polling with timeout |
| Test depends on previous test's state | Each test isolates: setup, run, cleanup |
| Test inspects internal struct fields | Test public behaviour, not internal layout |
| Single mega-test with 30 assertions | Split into table-driven cases |
| Mocking what you're testing | Test the real thing; mock its dependencies |
| Skipping `t.Parallel()` reflexively | Default to parallel; serialise only when needed |
| Comparing errors with `==` instead of `errors.Is` | Always `errors.Is` for chain-safety |
| Shared mutable state across cases | Each table case must be independent |

---

## 18. Cross-References

- [concurrency.md](concurrency.md) -- testing concurrent code, sync primitives
- [error-handling.md](error-handling.md) -- testing error conditions
- [profiling.md](profiling.md) -- benchmark interpretation
- [full-guide.md](full-guide.md) -- project layout
