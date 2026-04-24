# fireworks-go -- Error Handling Reference

> Idiomatic error handling. Wrap chains, sentinels, typed errors,
> panic/recover. The full protocol.

---

## 1. Errors Are Values

The `error` interface is one method:

```go
type error interface {
    Error() string
}
```

That's it. Anything that implements `Error() string` is an error. There is no class hierarchy, no exception type, no special syntax. Errors are passed by return value, checked, wrapped, and propagated.

This simplicity is the strength. Every code path is visible. There is no hidden control flow.

---

## 2. The Three Error Categories

Choose one consciously for every error you create.

### 2a. Sentinel Error

A package-level `var` that callers compare against using `errors.Is`.

```go
package orders

import "errors"

var (
    ErrNotFound      = errors.New("orders: not found")
    ErrAlreadyClosed = errors.New("orders: already closed")
    ErrLockHeld      = errors.New("orders: lock held by another process")
)

func Load(ctx context.Context, id string) (*Order, error) {
    row := db.QueryRowContext(ctx, "...", id)
    var o Order
    if err := row.Scan(&o.ID, &o.Total); err != nil {
        if errors.Is(err, sql.ErrNoRows) {
            return nil, ErrNotFound
        }
        return nil, fmt.Errorf("load order %s: %w", id, err)
    }
    return &o, nil
}

// Caller
order, err := orders.Load(ctx, id)
switch {
case errors.Is(err, orders.ErrNotFound):
    return http.StatusNotFound
case err != nil:
    return http.StatusInternalServerError
default:
    return http.StatusOK
}
```

Use sentinels when:

- Callers may meaningfully branch on this specific failure.
- The failure has no associated structured data.
- The set of sentinel errors is small and stable.

Standard library examples: `io.EOF`, `sql.ErrNoRows`, `os.ErrNotExist`, `context.Canceled`, `context.DeadlineExceeded`.

### 2b. Typed Error

A struct that implements `error`, possibly with extra fields and an `Unwrap` method. Callers extract with `errors.As`.

```go
type ValidationError struct {
    Field string
    Code  string
    Msg   string
}

func (v *ValidationError) Error() string {
    return fmt.Sprintf("validation failed on %s: %s", v.Field, v.Msg)
}

// Caller
var ve *ValidationError
if errors.As(err, &ve) {
    log.Printf("validation: field=%s code=%s", ve.Field, ve.Code)
    http.Error(w, ve.Msg, http.StatusBadRequest)
    return
}
```

Use typed errors when:

- The failure carries structured data the caller will use programmatically.
- Multiple variants of the same conceptual failure exist (different fields, codes, etc.).
- You need both `errors.Is` (for category) and field access.

For Is/As to work through wrapping, your custom type may need its own `Is` method:

```go
func (v *ValidationError) Is(target error) bool {
    return target == ErrValidation
}
```

### 2c. Opaque Error

A wrapped string with no specific type or sentinel. Callers can only check `err != nil` and propagate.

```go
data, err := os.ReadFile(path)
if err != nil {
    return fmt.Errorf("read config: %w", err)
}
```

Use opaque errors when:

- The caller has no way to recover from this specific failure.
- The error is purely informational for logging/debugging.
- This is most errors. Default to opaque, escalate only when needed.

---

## 3. Wrapping with %w

`fmt.Errorf` with `%w` wraps the underlying error so callers can unwrap it.

```go
// At a low level
data, err := os.ReadFile(path)
if err != nil {
    return fmt.Errorf("read %s: %w", path, err)
}

// At a higher level
cfg, err := loadConfig(path)
if err != nil {
    return fmt.Errorf("init server: %w", err)
}
```

If the caller does:

```go
err := server.Init(...)
if errors.Is(err, os.ErrNotExist) {
    fmt.Println("config file missing")
}
```

The chain unwinds: `init server -> read /etc/foo: <os.PathError> -> open: no such file`. `errors.Is` walks the chain and finds the match.

### %w vs %v vs %s

| Verb | Behaviour |
|------|-----------|
| `%w` | Wrap. The wrapped error is recoverable via Unwrap/Is/As. Only `fmt.Errorf` understands this. |
| `%v` | Default formatting. Loses the chain. |
| `%s` | String. Same as %v for errors. |

Always use `%w` when wrapping an error you received. Use `%v` or `%s` only for non-error context like input values.

### Multiple Wraps

Go 1.20+ supports wrapping multiple errors:

```go
return fmt.Errorf("submit: %w, %w", err1, err2)
```

`errors.Is` and `errors.As` walk all wrapped errors.

`errors.Join` is the cleaner API:

```go
return errors.Join(err1, err2, err3)
```

Use `errors.Join` when you have a slice of errors from a parallel operation -- e.g., the result of an `errgroup` where you want all failures, not just the first.

---

## 4. errors.Is and errors.As

### errors.Is

Walks the wrap chain looking for an error that equals (==) the target. For sentinels, this is what you want.

```go
if errors.Is(err, sql.ErrNoRows) { ... }
if errors.Is(err, context.Canceled) { ... }
```

You can implement custom Is logic by adding an `Is(target error) bool` method.

### errors.As

Walks the wrap chain looking for an error of the target type. Sets the target if found.

```go
var pathErr *os.PathError
if errors.As(err, &pathErr) {
    log.Printf("path error: op=%s path=%s", pathErr.Op, pathErr.Path)
}
```

Note `&pathErr` -- you pass a pointer to the target variable. `errors.As` writes into it.

### Common Mistakes

```go
// WRONG -- == doesn't walk the chain
if err == sql.ErrNoRows { ... }

// RIGHT
if errors.Is(err, sql.ErrNoRows) { ... }
```

```go
// WRONG -- you'll miss anything wrapping a *MyErr
if myErr, ok := err.(*MyErr); ok { ... }

// RIGHT
var myErr *MyErr
if errors.As(err, &myErr) { ... }
```

There are exceptions. Type assertions are fine when you control both ends and you know there's no wrapping. Use them for tight internal code; use `errors.As` at API boundaries.

---

## 5. Error Message Style

The conventions here are surprisingly load-bearing for tooling and log readability.

### Lowercase, No Trailing Punctuation

```go
// RIGHT
return errors.New("invalid order id")
return fmt.Errorf("load order %s: %w", id, err)

// WRONG
return errors.New("Invalid order id.")
```

The reason: errors get composed. `fmt.Errorf("load: %w", err)` should produce `"load: invalid order id"`, not `"load: Invalid order id."`.

### Format: "what failed: why"

```go
// "what failed" first, the wrapped reason last.
return fmt.Errorf("scan order row: %w", err)
return fmt.Errorf("validate order %s: %w", id, err)
```

### Don't Stutter

```go
// BAD -- "user.Save: failed to save user"
package user
func Save(u *User) error {
    return fmt.Errorf("failed to save user: %w", err)
}

// GOOD -- "user.Save: write users table: ..."
package user
func Save(u *User) error {
    return fmt.Errorf("write users table: %w", err)
}
```

The package and function name are visible from the call site. Don't repeat them.

### Don't Embed Sensitive Data

The error message ends up in logs. Do not put passwords, tokens, or PII in the error string.

```go
// BAD
return fmt.Errorf("auth failed for %s with token %s", user, token)

// GOOD
return fmt.Errorf("auth failed for %s: %w", user, ErrInvalidToken)
```

---

## 6. Panic and Recover

`panic` aborts the goroutine. `recover` catches it. They are not exceptions. They are emergency exits.

### When to panic

- Truly unrecoverable conditions during init -- e.g., a regex pattern in a constant fails to compile.
- Programmer errors where continuing would corrupt state -- e.g., an invariant check inside an algorithm.

### When NOT to panic

- Anywhere user input could trigger it. Always return an error.
- Inside library code. Library callers do not expect panics.
- For control flow. Errors are values; use them.

### Recover Idiom

`recover` only works inside a deferred function. It returns the value passed to `panic`, or nil if not panicking.

```go
func safeDoWork(input string) (err error) {
    defer func() {
        if r := recover(); r != nil {
            err = fmt.Errorf("doWork panicked: %v", r)
        }
    }()
    doWork(input)
    return nil
}
```

The named return value `err` is what the deferred recover writes into. This is one of the few places named returns earn their keep.

### HTTP Server Recover Middleware

```go
func Recover(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        defer func() {
            if rec := recover(); rec != nil {
                slog.Error("panic in handler",
                    "method", r.Method,
                    "path", r.URL.Path,
                    "panic", rec,
                    "stack", string(debug.Stack()),
                )
                http.Error(w, "internal server error", http.StatusInternalServerError)
            }
        }()
        next.ServeHTTP(w, r)
    })
}
```

`chi` and `gin` ship recovery middleware. Use the framework's; don't roll your own unless you have specific needs.

### Recover Across Goroutines: NO

```go
// WRONG -- recover only catches panics from the same goroutine
defer func() { recover() }()
go func() {
    panic("boom") // this kills the program
}()
```

Each goroutine needs its own deferred recover. The pattern:

```go
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

## 7. context.Canceled vs context.DeadlineExceeded

When a context is cancelled, propagating an error becomes nuanced. Two cases:

- The work itself failed. Return that error wrapped.
- The work was cancelled before completion. Return `ctx.Err()` (which is `context.Canceled` or `context.DeadlineExceeded`).

```go
func process(ctx context.Context, items []Item) error {
    for _, item := range items {
        select {
        case <-ctx.Done():
            return ctx.Err()
        default:
        }

        if err := handle(ctx, item); err != nil {
            return fmt.Errorf("handle %s: %w", item.ID, err)
        }
    }
    return nil
}

// Caller
err := process(ctx, items)
switch {
case errors.Is(err, context.Canceled):
    // user cancelled, log at debug level
case errors.Is(err, context.DeadlineExceeded):
    // timed out, log at warn level
case err != nil:
    // real failure, log at error level
}
```

---

## 8. errors.Join (Go 1.20+)

When you have multiple errors that all happened, join them:

```go
var errs []error
for _, item := range items {
    if err := process(item); err != nil {
        errs = append(errs, fmt.Errorf("item %s: %w", item.ID, err))
    }
}
if len(errs) > 0 {
    return errors.Join(errs...)
}
```

The joined error's `Error()` returns each message on its own line. `errors.Is` and `errors.As` walk all joined errors.

---

## 9. Don't Ignore Errors

```go
// WRONG
file.Close() // ignored

// WRONG
_ = file.Close() // explicit ignore but no comment

// RIGHT for deferred close on a read-only file
defer file.Close() // best-effort close, error doesn't matter

// RIGHT for write -- you must check
if err := file.Close(); err != nil {
    return fmt.Errorf("close output: %w", err)
}
```

`Close` on a writer can return errors that flush failed. Always check.

For deferred close on writers, use a named return and assign in the defer:

```go
func writeOutput(path string, data []byte) (err error) {
    f, err := os.Create(path)
    if err != nil {
        return fmt.Errorf("create %s: %w", path, err)
    }
    defer func() {
        if cerr := f.Close(); cerr != nil && err == nil {
            err = fmt.Errorf("close %s: %w", path, cerr)
        }
    }()

    if _, err := f.Write(data); err != nil {
        return fmt.Errorf("write %s: %w", path, err)
    }
    return nil
}
```

---

## 10. Custom Error Types: When to Build One

| Need | Build a Type? |
|------|---------------|
| One named failure, callers branch | No -- use a sentinel |
| One named failure, no caller branching | No -- use opaque (fmt.Errorf) |
| Failure carries structured data callers will read | Yes -- typed error |
| Multiple variants of same conceptual failure | Yes -- typed error with field |
| You want to match on category in errors.Is | Yes -- implement Is |
| You want to extract via errors.As | Yes -- it's a pointer-receiver type |

Don't build a type just because you can. Most errors are opaque, and that is fine.

---

## 11. Anti-Patterns to Avoid

```go
// ANTI -- string compare error messages
if strings.Contains(err.Error(), "not found") { ... }
// Use errors.Is or errors.As

// ANTI -- swallow with no log, no return
_ = doThing()
// At minimum: log it with structured fields

// ANTI -- panic for normal validation
if !valid(input) { panic("bad input") }
// Return an error

// ANTI -- catch a panic to convert to error in production code
// Reserved for program boundaries (HTTP middleware, goroutine entry points)

// ANTI -- multi-return error then ignore the value
val, err := f()
if err != nil { return err }
_ = val // Use it or you didn't need to call f
```

---

## 12. Logging vs Returning

Each error gets logged at most once. The rule:

- The caller closest to the user (HTTP handler, RPC handler, CLI top level) logs.
- Every layer in between propagates without logging.

If everyone logs, you get N copies of the same error in the logs and they're hard to correlate.

```go
// Repository -- propagate
func (r *Repo) Load(ctx context.Context, id string) (*Order, error) {
    if err := r.db.QueryRowContext(...).Scan(...); err != nil {
        return nil, fmt.Errorf("scan order: %w", err)
    }
}

// Service -- propagate
func (s *Service) Get(ctx context.Context, id string) (*Order, error) {
    o, err := s.repo.Load(ctx, id)
    if err != nil {
        return nil, fmt.Errorf("get order %s: %w", id, err)
    }
    return o, nil
}

// HTTP handler -- log + respond
func (h *Handler) Get(w http.ResponseWriter, r *http.Request) {
    o, err := h.svc.Get(r.Context(), id)
    if err != nil {
        slog.ErrorContext(r.Context(), "get order failed", "id", id, "err", err)
        http.Error(w, "internal", http.StatusInternalServerError)
        return
    }
    json.NewEncoder(w).Encode(o)
}
```

---

## 13. Cross-References

- [concurrency.md](concurrency.md) -- error propagation across goroutines via errgroup
- [testing-go.md](testing-go.md) -- testing errors with errors.Is, table-driven cases
- [full-guide.md](full-guide.md) -- general project structure
