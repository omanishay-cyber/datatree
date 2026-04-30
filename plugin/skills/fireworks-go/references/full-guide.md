# fireworks-go -- Full Guide

> The deep reference for Go service development.
> Loaded on demand from `SKILL.md` when you need more depth.

---

## 1. Project Layout (Detailed)

The `cmd / internal / pkg` convention is not just style. It is enforced by the Go toolchain (`internal/`) and by community tooling (linters, code generators).

### cmd/

```
cmd/
  myservice/          # one binary
    main.go
  myservice-cli/      # another binary
    main.go
```

`main.go` should be tiny. Its job: parse flags, build dependencies, call `Run`. Example:

```go
// cmd/myservice/main.go
package main

import (
    "context"
    "log/slog"
    "os"
    "os/signal"
    "syscall"

    "myservice/internal/config"
    "myservice/internal/server"
)

func main() {
    cfg, err := config.Load()
    if err != nil {
        slog.Error("load config", "err", err)
        os.Exit(1)
    }

    ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
    defer cancel()

    if err := server.Run(ctx, cfg); err != nil {
        slog.Error("server", "err", err)
        os.Exit(1)
    }
}
```

Why this matters: `main` is hard to test. By keeping it tiny, every interesting code path lives in `internal/server` where it can be unit-tested.

### internal/

The `internal` directory is special: any package under `internal/` can only be imported by code rooted at the parent directory. This is enforced by the toolchain.

Use this aggressively. Default everything to `internal/`. Only promote to `pkg/` if you genuinely need external consumers.

```
internal/
  server/
    server.go         # HTTP server bootstrap
    routes.go         # Route table
    middleware.go
  orders/
    service.go        # Business logic
    repository.go     # DB access
    types.go          # Domain types
  payments/
    ...
```

### pkg/

Only put things here that you have already decided to make stable for outside use. If you're not sure, use `internal/`.

---

## 2. Modules and Versioning

```bash
go mod init github.com/example/myservice
go get github.com/spf13/cobra@v1.8.0
go mod tidy
```

### Versioning Rules

- v0.x.y -- breaking changes allowed.
- v1.x.y -- semver, no breaking changes within v1.
- v2+ -- module path must change: `github.com/example/myservice/v2`. The toolchain enforces this.

### Replace Directives

For local development across modules:

```
// go.mod
require github.com/example/lib v1.0.0
replace github.com/example/lib => ../lib
```

Remove the `replace` before publishing.

---

## 3. Generics (Go 1.18+)

Go has parametric polymorphism. Use it sparingly: most code is fine without it.

### Basic Type Parameter

```go
func Map[T, U any](in []T, f func(T) U) []U {
    out := make([]U, len(in))
    for i, v := range in {
        out[i] = f(v)
    }
    return out
}
```

### Constraints

```go
type Numeric interface {
    ~int | ~int64 | ~float64
}

func Sum[T Numeric](xs []T) T {
    var total T
    for _, x := range xs {
        total += x
    }
    return total
}
```

The `~int` syntax means "any type whose underlying type is int", so a `type Cents int` still matches.

### When NOT to Use Generics

- A single concrete type is enough -- don't generalise prematurely.
- Interfaces already express the abstraction better.
- The function body branches on type -- you probably want `interface{}` and a type switch.

---

## 4. Standard Library Highlights

### log/slog (Go 1.21+)

The official structured logger. Use it everywhere.

```go
import "log/slog"

logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{
    Level: slog.LevelInfo,
}))
slog.SetDefault(logger)

slog.Info("order submitted", "order_id", o.ID, "total_cents", o.TotalCents)
slog.Error("submit failed", "order_id", o.ID, "err", err)
```

### encoding/json

```go
type Order struct {
    ID         string    `json:"id"`
    TotalCents int64     `json:"total_cents"`
    CreatedAt  time.Time `json:"created_at"`
    Note       string    `json:"note,omitempty"`  // omit if zero value
    Internal   string    `json:"-"`               // never serialise
}
```

Decode with strict mode in HTTP handlers:

```go
dec := json.NewDecoder(r.Body)
dec.DisallowUnknownFields()
var req CreateOrderRequest
if err := dec.Decode(&req); err != nil {
    http.Error(w, "bad request", http.StatusBadRequest)
    return
}
```

### net/http

```go
mux := http.NewServeMux()
mux.HandleFunc("POST /orders", h.createOrder)         // Go 1.22 method-aware patterns
mux.HandleFunc("GET /orders/{id}", h.getOrder)        // path parameters

srv := &http.Server{
    Addr:              ":8080",
    Handler:           mux,
    ReadHeaderTimeout: 5 * time.Second,
    ReadTimeout:       30 * time.Second,
    WriteTimeout:      30 * time.Second,
    IdleTimeout:       120 * time.Second,
}
```

Always set timeouts. The default values are unbounded and a slow client will exhaust your goroutine pool.

### embed

```go
import "embed"

//go:embed templates/*.html
var templatesFS embed.FS

//go:embed migrations/*.sql
var migrationsFS embed.FS
```

`embed.FS` produces a value that satisfies `fs.FS`. Pass it to template engines, http.FileServer, or your migration runner. No filesystem access needed at runtime.

### Build Tags

```go
//go:build linux

package server
```

Common tags:
- `//go:build linux` / `darwin` / `windows`
- `//go:build integration` -- gate slow tests behind `go test -tags integration`
- `//go:build go1.22` -- gate features by Go version

---

## 5. Generics in Practice

### Ordered Types

```go
import "cmp"

func Max[T cmp.Ordered](a, b T) T {
    if a > b {
        return a
    }
    return b
}
```

### Set Type

```go
type Set[T comparable] map[T]struct{}

func NewSet[T comparable](items ...T) Set[T] {
    s := make(Set[T], len(items))
    for _, item := range items {
        s[item] = struct{}{}
    }
    return s
}

func (s Set[T]) Add(item T)       { s[item] = struct{}{} }
func (s Set[T]) Contains(item T) bool {
    _, ok := s[item]
    return ok
}
```

---

## 6. HTTP Frameworks: gin, fiber, chi

| Framework | Style | Pick When |
|-----------|-------|-----------|
| stdlib `net/http` (1.22+) | Standard, no deps | New service, no special needs |
| `chi` | Router on top of net/http | You want middleware composition + path params + standard handler signature |
| `gin` | Custom context, fast | You need built-in JSON binding, validation, and a large ecosystem |
| `fiber` | fasthttp-based | You need raw throughput and don't need stdlib compatibility |

For most services, prefer stdlib (1.22+) or `chi`. They keep the standard `http.Handler` signature, which means every middleware in the ecosystem composes.

### chi Example

```go
import "github.com/go-chi/chi/v5"
import "github.com/go-chi/chi/v5/middleware"

r := chi.NewRouter()
r.Use(middleware.RequestID)
r.Use(middleware.Logger)
r.Use(middleware.Recoverer)
r.Use(middleware.Timeout(30 * time.Second))

r.Route("/orders", func(r chi.Router) {
    r.Post("/", h.createOrder)
    r.Get("/{id}", h.getOrder)
})
```

### gin Example

```go
import "github.com/gin-gonic/gin"

r := gin.New()
r.Use(gin.Recovery())

r.POST("/orders", func(c *gin.Context) {
    var req CreateOrderRequest
    if err := c.ShouldBindJSON(&req); err != nil {
        c.JSON(400, gin.H{"error": err.Error()})
        return
    }
    o, err := svc.Create(c.Request.Context(), req)
    if err != nil {
        c.JSON(500, gin.H{"error": err.Error()})
        return
    }
    c.JSON(201, o)
})
```

---

## 7. Database Patterns

### database/sql

```go
import (
    "database/sql"
    _ "github.com/lib/pq"  // postgres driver
)

db, err := sql.Open("postgres", connStr)
if err != nil { ... }
db.SetMaxOpenConns(25)
db.SetMaxIdleConns(5)
db.SetConnMaxLifetime(5 * time.Minute)
```

Always set the connection pool limits. The defaults are unbounded.

### Always Use QueryContext

```go
func (r *OrderRepo) FindByCustomer(ctx context.Context, customerID string) ([]*Order, error) {
    rows, err := r.db.QueryContext(ctx,
        `SELECT id, total_cents, created_at FROM orders WHERE customer_id = $1`,
        customerID,
    )
    if err != nil {
        return nil, fmt.Errorf("query orders: %w", err)
    }
    defer rows.Close()

    var orders []*Order
    for rows.Next() {
        var o Order
        if err := rows.Scan(&o.ID, &o.TotalCents, &o.CreatedAt); err != nil {
            return nil, fmt.Errorf("scan order row: %w", err)
        }
        orders = append(orders, &o)
    }
    return orders, rows.Err()
}
```

Three rules:

1. Always pass `ctx` -- so cancellation closes the underlying network read.
2. Always `defer rows.Close()` -- otherwise you leak a connection.
3. Always check `rows.Err()` after the loop -- iteration errors are not returned by `rows.Next`.

### sqlc and gorm

- `sqlc` -- generates type-safe Go from SQL. Best for teams that want SQL-first.
- `gorm` -- ORM. Saves boilerplate but obscures generated SQL. Best for prototypes; risky for high-perf services.

If you reach for an ORM, prefer `sqlc` for greenfield work. It is a code generator, not a runtime, so the perf cost is zero.

---

## 8. Configuration

### Pattern: Struct + Env + Defaults

```go
type Config struct {
    HTTPAddr    string        `env:"HTTP_ADDR" default:":8080"`
    DBURL       string        `env:"DB_URL" required:"true"`
    LogLevel    string        `env:"LOG_LEVEL" default:"info"`
    Timeout     time.Duration `env:"TIMEOUT" default:"30s"`
}
```

Use one of: `caarlos0/env`, `kelseyhightower/envconfig`, or hand-roll. Hand-rolled is fine for small services and removes a dependency.

### Never Read Env Vars Inside a Function

```go
// WRONG -- now this function is untestable and depends on global state
func StartServer() {
    addr := os.Getenv("HTTP_ADDR")
    ...
}

// RIGHT -- config flows in
func StartServer(cfg Config) error { ... }
```

---

## 9. Defer Idioms

### defer Runs in LIFO Order

```go
defer fmt.Println("1")
defer fmt.Println("2")
defer fmt.Println("3")
// Output: 3, 2, 1
```

### defer Captures the Function Reference + Arguments at the defer Statement

```go
x := 1
defer fmt.Println(x) // Prints "1" -- argument captured here
x = 2
// Returns
// Output: 1
```

### Common Defer Patterns

```go
// Always close
f, err := os.Open(path)
if err != nil { return err }
defer f.Close()

// Time a function
defer func(start time.Time) {
    log.Printf("Submit took %s", time.Since(start))
}(time.Now())

// Recover from panic at goroutine boundary (only place panics legitimately cross)
go func() {
    defer func() {
        if r := recover(); r != nil {
            log.Printf("worker panic: %v", r)
        }
    }()
    work()
}()
```

---

## 10. iota and Enum-Like Types

```go
type Status int

const (
    StatusPending Status = iota
    StatusProcessing
    StatusComplete
    StatusFailed
)

func (s Status) String() string {
    return [...]string{"pending", "processing", "complete", "failed"}[s]
}
```

For tighter typesafety and JSON support, use `stringer` or hand-write `MarshalJSON`:

```go
func (s Status) MarshalJSON() ([]byte, error) {
    return json.Marshal(s.String())
}
```

Generate `String()` automatically:

```bash
go install golang.org/x/tools/cmd/stringer@latest
//go:generate stringer -type=Status
go generate ./...
```

---

## 11. sync Primitives

| Primitive | Use For |
|-----------|---------|
| `sync.Mutex` | Mutual exclusion around shared state |
| `sync.RWMutex` | Many readers, few writers (only worth it if you've measured) |
| `sync.WaitGroup` | Wait for N goroutines to finish |
| `sync.Once` | Run init exactly once |
| `sync.Map` | Concurrent map for high write contention or many goroutines |
| `sync.Pool` | Reuse short-lived allocations to reduce GC pressure |
| `sync/atomic` | Lock-free counters, flags |

### sync.Mutex

```go
type Counter struct {
    mu sync.Mutex
    n  int
}

func (c *Counter) Inc() {
    c.mu.Lock()
    defer c.mu.Unlock()
    c.n++
}
```

Always pair `Lock` with a `defer Unlock` on the next line. No exceptions, even for "fast" critical sections.

### sync.RWMutex

```go
type Cache struct {
    mu sync.RWMutex
    m  map[string]string
}

func (c *Cache) Get(k string) (string, bool) {
    c.mu.RLock()
    defer c.mu.RUnlock()
    v, ok := c.m[k]
    return v, ok
}

func (c *Cache) Set(k, v string) {
    c.mu.Lock()
    defer c.mu.Unlock()
    c.m[k] = v
}
```

Only worth using if reads dominate writes by 10x or more. Otherwise the lock overhead exceeds the benefit.

### sync.Pool

```go
var bufPool = sync.Pool{
    New: func() interface{} {
        return new(bytes.Buffer)
    },
}

func handler(w http.ResponseWriter, r *http.Request) {
    buf := bufPool.Get().(*bytes.Buffer)
    defer func() {
        buf.Reset()
        bufPool.Put(buf)
    }()
    // use buf
}
```

`sync.Pool` is for short-lived allocations on hot paths. Items in the pool may be GC'd between uses, so do not store anything that requires explicit cleanup.

### sync/atomic

```go
type Counter struct {
    n atomic.Int64
}

func (c *Counter) Inc()         { c.n.Add(1) }
func (c *Counter) Value() int64 { return c.n.Load() }
```

Cheaper than a mutex. Use for counters, flags, and pointer swaps.

---

## 12. Cobra (CLI)

```go
import "github.com/spf13/cobra"

func main() {
    root := &cobra.Command{
        Use:   "myservice",
        Short: "Service for processing orders",
    }

    root.AddCommand(serveCommand())
    root.AddCommand(migrateCommand())

    if err := root.Execute(); err != nil {
        os.Exit(1)
    }
}

func serveCommand() *cobra.Command {
    var addr string
    cmd := &cobra.Command{
        Use:   "serve",
        Short: "Start the HTTP server",
        RunE: func(cmd *cobra.Command, args []string) error {
            return server.Run(cmd.Context(), addr)
        },
    }
    cmd.Flags().StringVar(&addr, "addr", ":8080", "HTTP listen address")
    return cmd
}
```

`RunE` (not `Run`) so errors propagate. `cmd.Context()` is cancelled on SIGINT.

---

## 13. Cross-References

| Resource | Where | Purpose |
|----------|-------|---------|
| Effective Go | https://go.dev/doc/effective_go | The original style guide |
| Go Code Review Comments | https://github.com/golang/go/wiki/CodeReviewComments | Community review checklist |
| Concurrency Reference | [concurrency.md](concurrency.md) | Goroutines, channels, errgroup |
| Error Handling Reference | [error-handling.md](error-handling.md) | Wrap chains, sentinel/typed |
| Testing Reference | [testing-go.md](testing-go.md) | Table-driven, fuzzing, benchmarks |
| Profiling Reference | [profiling.md](profiling.md) | pprof, trace, allocation reduction |
