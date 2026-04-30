---
name: fireworks-go
version: 1.0.0
author: mneme
description: Use when writing Go services, designing concurrent systems with goroutines and channels, propagating context.Context, building HTTP APIs with gin/fiber/chi, working with errgroup, debugging deadlocks, designing interfaces, structuring cmd/internal/pkg layouts, writing table-driven tests, or profiling with pprof. Covers Go 1.22+, idiomatic error handling, structural typing, and the race detector.
triggers:
  - go
  - golang
  - goroutine
  - chan
  - interface
  - gorm
  - gin
  - fiber
  - cobra
  - errgroup
  - context
  - deadlock
  - mutex
  - sync.Pool
  - slice
  - map
  - defer
  - iota
tags:
  - go
  - golang
  - concurrency
  - error-handling
  - interface-design
  - goroutines
  - context
---

# FIREWORKS-GO -- Idiomatic Go 1.22+ Superbrain

> The definitive Go skill for production services.
> Pairs with `fireworks-test`, `fireworks-performance`, and `fireworks-debug` for full-stack coverage.

---

## 1. The Go Protocol

Every Go task moves through this pipeline. No skipping.

```
DESIGN --> WRITE --> vet --> test -race --> bench --> SHIP
```

1. **DESIGN** -- Sketch types and interfaces first. Pick error semantics. Decide concurrency boundaries.
2. **WRITE** -- Idiomatic Go: explicit errors, accept interfaces return structs, no clever tricks.
3. **vet** -- `go vet ./...` and `staticcheck ./...` -- zero findings before proceeding.
4. **test -race** -- `go test -race ./...` always. Race conditions are not optional.
5. **bench** -- `go test -bench=. -benchmem` for hot paths. Profile before claiming "fast".
6. **SHIP** -- Only after every gate is green.

### Pre-Flight Checklist

- [ ] Are errors wrapped with `%w` so callers can unwrap?
- [ ] Does every goroutine have a clear lifecycle (started, signalled to stop, joined)?
- [ ] Is `context.Context` threaded through every blocking call?
- [ ] Are channel directions narrowed at API boundaries (`<-chan T`, `chan<- T`)?
- [ ] Are interfaces small and consumer-defined?
- [ ] Is the file under 500 lines? If not, can it be split?

---

## 2. Idiomatic Error Handling

Go errors are values. There are no exceptions. Treat every error as data you must inspect or propagate.

### The Canonical Pattern

```go
val, err := doThing()
if err != nil {
    return fmt.Errorf("doThing in OrderService.Submit: %w", err)
}
// use val
```

Three rules that are not negotiable:

1. **Always check `err != nil` immediately** -- never skip, never `_ =`.
2. **Wrap with `%w`** when you propagate so callers can `errors.Is` / `errors.As` against it.
3. **Add context** at every hop -- the function name and the operation, not the input data (which may be sensitive).

### errors.Is vs errors.As

```go
// Sentinel error -- compare with errors.Is
var ErrOrderNotFound = errors.New("order not found")

func LoadOrder(id string) (*Order, error) {
    row := db.QueryRow("SELECT ... WHERE id = ?", id)
    var o Order
    if err := row.Scan(&o.ID, &o.Total); err != nil {
        if errors.Is(err, sql.ErrNoRows) {
            return nil, ErrOrderNotFound
        }
        return nil, fmt.Errorf("scan order %s: %w", id, err)
    }
    return &o, nil
}

// Caller
order, err := LoadOrder(id)
if errors.Is(err, ErrOrderNotFound) {
    http.Error(w, "not found", http.StatusNotFound)
    return
}
```

```go
// Typed error -- extract with errors.As
type ValidationError struct {
    Field string
    Msg   string
}

func (v *ValidationError) Error() string {
    return fmt.Sprintf("%s: %s", v.Field, v.Msg)
}

// Caller
var ve *ValidationError
if errors.As(err, &ve) {
    log.Printf("validation failed on %s", ve.Field)
}
```

### Wrap, Do Not Stringify

```go
// WRONG -- loses the underlying error chain
return fmt.Errorf("submit failed: %s", err.Error())

// RIGHT -- preserves the chain for errors.Is/As
return fmt.Errorf("submit failed: %w", err)
```

### Sentinel vs Typed vs Opaque -- Decision Table

| You need | Use |
|----------|-----|
| A single named failure mode the caller may branch on | Sentinel: `var ErrFoo = errors.New("foo")` |
| Failure with structured data (field name, code, retry-after) | Typed: `type FooError struct { ... }` |
| Internal failure the caller should not branch on | Opaque: `errors.New` or `fmt.Errorf` without sentinels |

> Deep dive: [references/error-handling.md](references/error-handling.md)

---

## 3. Goroutines and Channels

Goroutines are cheap. Leaks are not. Every goroutine must have a defined exit path before you launch it.

### The Three Goroutine Rules

1. **Every goroutine is owned** -- some piece of code is responsible for telling it to stop and waiting for it.
2. **Every goroutine has a stop signal** -- a `context.Context`, a done channel, or a closed input channel.
3. **Every goroutine is joined** -- `sync.WaitGroup`, `errgroup.Group`, or a result channel that signals completion.

### Worker Pool Pattern

```go
func ProcessJobs(ctx context.Context, jobs []Job, workers int) error {
    g, ctx := errgroup.WithContext(ctx)
    in := make(chan Job)

    // Producer
    g.Go(func() error {
        defer close(in)
        for _, j := range jobs {
            select {
            case in <- j:
            case <-ctx.Done():
                return ctx.Err()
            }
        }
        return nil
    })

    // Workers
    for i := 0; i < workers; i++ {
        g.Go(func() error {
            for j := range in {
                if err := process(ctx, j); err != nil {
                    return fmt.Errorf("job %s: %w", j.ID, err)
                }
            }
            return nil
        })
    }

    return g.Wait()
}
```

If any worker returns an error, `errgroup` cancels the shared context, the producer's select hits `ctx.Done()`, and every other worker drains its current job and exits. Clean shutdown for free.

### Channel Direction at API Boundaries

```go
// WRONG -- caller can both send and receive, semantics unclear
func StartProducer() chan Event { ... }

// RIGHT -- caller can only receive
func StartProducer(ctx context.Context) <-chan Event { ... }

// RIGHT -- function only sends
func Drain(ctx context.Context, in <-chan Event, out chan<- Result) { ... }
```

### Common Channel Patterns

```go
// Fan-out: one producer, N consumers
// Fan-in: N producers, one consumer (use sync.WaitGroup + a single output channel)
// Pipeline: chain of stages, each takes <-chan In and returns <-chan Out

// Buffered vs unbuffered:
// - Unbuffered: synchronous handoff, sender blocks until receiver is ready
// - Buffered: async up to capacity, used to absorb bursts or break tight coupling
// Pick unbuffered by default. Add a buffer only when profiling shows you need it.
```

> Deep dive: [references/concurrency.md](references/concurrency.md)

---

## 4. context.Context Propagation

Every function that does I/O, blocks, or spawns a goroutine takes `ctx context.Context` as its first parameter. No exceptions.

### The Context Discipline

```go
// RIGHT
func (s *OrderService) Submit(ctx context.Context, o *Order) error { ... }

// WRONG -- you cannot cancel this, you cannot deadline it, you cannot trace it
func (s *OrderService) Submit(o *Order) error { ... }
```

### Honour the Context

```go
func fetch(ctx context.Context, url string) ([]byte, error) {
    req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
    if err != nil {
        return nil, fmt.Errorf("new request: %w", err)
    }
    resp, err := http.DefaultClient.Do(req)
    if err != nil {
        return nil, fmt.Errorf("do request: %w", err)
    }
    defer resp.Body.Close()
    return io.ReadAll(resp.Body)
}
```

`http.NewRequestWithContext` propagates cancellation to the network layer. If `ctx` is cancelled, the in-flight TCP connection is closed.

### Never Store ctx in a Struct

```go
// WRONG -- ties the struct's lifetime to a single request's context
type OrderService struct {
    ctx context.Context
    db  *sql.DB
}

// RIGHT -- pass ctx into every method
type OrderService struct {
    db *sql.DB
}
func (s *OrderService) Submit(ctx context.Context, o *Order) error { ... }
```

### context.Value Is Last Resort

`context.Value` is for request-scoped data that crosses API boundaries (request ID, auth principal, tracing span). It is not for passing optional parameters. Use it sparingly and always with a typed key.

```go
type ctxKey int
const requestIDKey ctxKey = 1

func WithRequestID(ctx context.Context, id string) context.Context {
    return context.WithValue(ctx, requestIDKey, id)
}
func RequestID(ctx context.Context) string {
    if v, ok := ctx.Value(requestIDKey).(string); ok {
        return v
    }
    return ""
}
```

---

## 5. Interface Design

Go is structurally typed. A type satisfies an interface by implementing its methods, no `implements` keyword needed. This drives a specific design discipline.

### Accept Interfaces, Return Structs

```go
// RIGHT -- accept the smallest interface the function actually uses
func ProcessReader(r io.Reader) error { ... }

// RIGHT -- return a concrete type so callers can use all its methods
func NewOrderService(db *sql.DB) *OrderService { ... }

// WRONG -- forces the caller to upcast
func NewOrderService(db *sql.DB) OrderServiceInterface { ... }
```

### Define Interfaces Where They Are Used

The consumer defines the interface, not the producer. This is the inverse of how most OO languages work and it is one of the most important rules in Go.

```go
// Package "shipping" -- the consumer
package shipping

type OrderLoader interface {
    Load(ctx context.Context, id string) (*Order, error)
}

func (s *Shipper) Ship(ctx context.Context, loader OrderLoader, id string) error {
    o, err := loader.Load(ctx, id)
    ...
}

// Package "orders" -- the producer
package orders

type Service struct { ... }
func (s *Service) Load(ctx context.Context, id string) (*Order, error) { ... }
// No "OrderLoader" interface declared here. The consumer defines what it needs.
```

### Keep Interfaces Small

The standard library's most reused interfaces have one or two methods: `io.Reader`, `io.Writer`, `io.Closer`, `fmt.Stringer`, `error`. Aim for the same.

```go
// AVOID
type OrderRepository interface {
    Save(o *Order) error
    Load(id string) (*Order, error)
    Delete(id string) error
    Search(q string) ([]*Order, error)
    Count() (int, error)
    UpdateStatus(id string, status Status) error
}

// PREFER -- split by use case
type OrderSaver interface { Save(ctx context.Context, o *Order) error }
type OrderLoader interface { Load(ctx context.Context, id string) (*Order, error) }
// Compose where needed
type OrderRepository interface { OrderSaver; OrderLoader }
```

> Deep dive: [references/full-guide.md](references/full-guide.md)

---

## 6. Deadlocks and the Race Detector

### Detect Races

```bash
go test -race ./...
go run -race ./cmd/server
```

The race detector is an instrumented runtime. It catches concurrent reads and writes to the same memory without synchronization. Run it on every CI build.

### Common Deadlock Patterns

| Pattern | Diagnosis |
|---------|-----------|
| Goroutine sends on a channel nobody reads | `pprof` shows blocked goroutine on `chansend` |
| Two goroutines lock mutexes in opposite orders | Add a single lock-order convention; use `sync.Mutex` + `defer m.Unlock()` |
| `sync.WaitGroup.Wait` before all `Add` calls complete | Always call `Add` on the goroutine's parent before launching |
| Buffered channel fills, sender blocks, receiver never starts | Diagnose with `go tool trace`; usually a missing goroutine |

### `go vet` and `staticcheck`

Always run both. `go vet` is bundled; `staticcheck` is the de-facto extra linter.

```bash
go vet ./...
staticcheck ./...
```

---

## 7. Project Layout

The community-standard layout is specific and load-bearing. Follow it.

```
myservice/
  cmd/
    myservice/
      main.go              # Tiny: parse flags, build dependencies, call Run
  internal/                # Code only this module can import
    server/
      server.go
      server_test.go
    orders/
      service.go
      service_test.go
      repository.go
  pkg/                     # Code other modules MAY import (use sparingly)
    clientsdk/
      client.go
  api/                     # OpenAPI, protobuf, schema
    openapi.yaml
  configs/                 # Default configs (not secrets)
    default.yaml
  scripts/                 # Build / deploy helpers
  test/                    # Integration / e2e fixtures
  go.mod
  go.sum
  Makefile
```

Rules:

- `cmd/<binary>/main.go` is at most ~50 lines. All real work is in `internal/`.
- `internal/` is enforced by the toolchain -- nothing outside the module can import it.
- `pkg/` exists only when you genuinely intend external consumers; otherwise prefer `internal/`.
- One package per directory. Package name matches directory name.

---

## 8. Table-Driven Tests

This is the Go testing idiom. Master it.

```go
func TestParseOrderID(t *testing.T) {
    tests := []struct {
        name    string
        input   string
        want    string
        wantErr error
    }{
        {"valid prefixed", "ord_abc123", "abc123", nil},
        {"empty string", "", "", ErrEmptyID},
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

Rules:

- `t.Run(tt.name, ...)` for every case so failures show which row failed.
- Compare errors with `errors.Is` -- not string equality.
- Use `t.Fatalf` when a failure means the rest of the test cannot proceed.
- Use `t.Errorf` when subsequent assertions still make sense.

### testify -- Use Sparingly

`testify/assert` and `testify/require` are popular but not idiomatic. The standard library + `errors.Is` covers 95% of cases. Reach for `testify` only when you have genuinely complex equality checks (deeply nested structs, time tolerances).

> Deep dive: [references/testing-go.md](references/testing-go.md)

---

## 9. Profiling: pprof and trace

When something feels slow, profile. Do not guess.

### CPU Profile

```go
import _ "net/http/pprof"
// ... in main:
go func() { log.Println(http.ListenAndServe("localhost:6060", nil)) }()
```

```bash
go tool pprof http://localhost:6060/debug/pprof/profile?seconds=30
(pprof) top10
(pprof) list MyHotFunction
(pprof) web
```

### Benchmark + Memory Profile

```go
func BenchmarkParseOrderID(b *testing.B) {
    for i := 0; i < b.N; i++ {
        _, _ = ParseOrderID("ord_abc123")
    }
}
```

```bash
go test -bench=. -benchmem -memprofile=mem.out
go tool pprof mem.out
```

### Execution Trace

```bash
go test -trace=trace.out
go tool trace trace.out
```

The trace UI shows goroutine scheduling, GC, and syscall blocking. Use it when you suspect contention or scheduler-related issues.

> Deep dive: [references/profiling.md](references/profiling.md)

---

## 10. Wrong vs Right -- Quick Reference

| Anti-Pattern | Why It's Wrong | Correct Pattern |
|--------------|----------------|-----------------|
| `if err != nil { return err }` everywhere unwrapped | Loses caller context | Wrap: `fmt.Errorf("op: %w", err)` |
| `panic` for control flow | Crashes the process | Return errors |
| Goroutine without exit path | Leaks until process dies | `ctx.Done()` or close-on-shutdown |
| Storing `ctx` in a struct | Couples lifetimes | Pass into every method |
| Shared mutable map without lock | Race condition | `sync.Mutex` or `sync.Map` |
| `time.Sleep` in tests | Flaky timing | Use channels or fake clocks |
| Returning `interface{}` (or `any`) | Loses type safety | Return concrete type |
| Empty interface as a "generic bag" | No compile-time checks | Use generics (Go 1.18+) |
| `for _, v := range slice { go work(v) }` (pre-1.22) | Loop var capture | Go 1.22 fixes this; still pass `v` explicitly for clarity |
| Manual mutex around a channel | Channels already serialize | Pick one or the other, not both |

---

## 11. Iron Law

```
NO GO CODE WITHOUT A CLEAR ERROR PATH AND A CLEAR GOROUTINE LIFECYCLE.

Every goroutine has a stop signal.
Every error is wrapped with context.
Every context is propagated.
Every interface is small.
Every test is table-driven.
Every race is fixed before merge.
```

---

## 12. Compound Skill Chaining

| Chain To | When | What It Adds |
|----------|------|--------------|
| `fireworks-test` | After implementation | Coverage strategy, integration tests, fuzzing |
| `fireworks-performance` | When optimising | pprof workflow, allocation reduction, GC tuning |
| `fireworks-debug` | On crash or hang | Race detector workflow, delve, stack trace reading |
| `fireworks-security` | On HTTP/auth code | Input validation, TLS, OWASP API Top 10 |
| `fireworks-architect` | New service design | Hexagonal layout, event-driven patterns |

---

## 13. Reference Files Index

| File | Coverage |
|------|----------|
| [references/full-guide.md](references/full-guide.md) | Overview, project layout, generics, build tags, embed, modules |
| [references/concurrency.md](references/concurrency.md) | Goroutines, channels, errgroup, sync primitives, atomic, sync.Pool |
| [references/error-handling.md](references/error-handling.md) | Wrap chains, sentinel vs typed, error message style, panic/recover |
| [references/testing-go.md](references/testing-go.md) | Table-driven, subtests, fuzzing, benchmarks, golden files, httptest |
| [references/profiling.md](references/profiling.md) | pprof workflow, memory profiles, execution traces, benchmark analysis |

---

## 14. Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
