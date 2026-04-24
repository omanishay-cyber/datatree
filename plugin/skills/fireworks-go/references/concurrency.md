# fireworks-go -- Concurrency Reference

> Goroutines, channels, errgroup, sync primitives, atomic.
> The Go memory model and how to write code that is safe under it.

---

## 1. The Go Memory Model in One Page

Go's memory model defines what one goroutine can see of writes made by another. The single rule you must internalise:

**A read of a variable in goroutine B is only guaranteed to see a write from goroutine A if a happens-before relationship has been established between them.**

Happens-before is established by:

- Channel send/receive (a send happens-before the corresponding receive completes).
- `sync.Mutex.Unlock` happens-before the next `Lock` succeeds.
- `sync.WaitGroup.Add` and `Wait` calls.
- `sync.Once.Do` -- the first call's body happens-before any other call returns.
- `atomic` operations using their documented release/acquire semantics.

If none of those mechanisms link your two goroutines, the read may see a stale or torn value, and the race detector will flag it.

**Plain shared-memory access without synchronization is a bug**, even if it "works" in your tests.

---

## 2. Goroutines

### Lifecycle Discipline

Every `go` statement must answer three questions before you write it:

1. **How does this goroutine know to stop?** -- ctx cancellation, done channel, closed input channel.
2. **Who is waiting for it?** -- WaitGroup.Wait, errgroup.Wait, result channel receive.
3. **What does it leak if those signals never fire?** -- goroutine, file handle, DB connection, memory.

If you cannot answer all three, do not write `go`.

### Goroutine Leak Detection

Add `go.uber.org/goleak` to your test setup. It snapshots the live goroutines at test start and verifies they're all gone at end.

```go
import "go.uber.org/goleak"

func TestMain(m *testing.M) {
    goleak.VerifyTestMain(m)
}
```

Any leaked goroutine fails the test. You will catch leaks at PR time, not in production.

### The `for ... go` Trap (pre Go 1.22)

```go
// PRE-1.22 BUG -- all goroutines see the same v
for _, v := range items {
    go func() {
        process(v)
    }()
}

// FIX (any version) -- pass v as argument
for _, v := range items {
    go func(v Item) {
        process(v)
    }(v)
}
```

Go 1.22 fixed this for `for ... range` loops by giving each iteration its own variable scope. But if your code may run on older Go, or you use a non-range loop, still pass explicitly. It is also clearer to a reader.

---

## 3. Channels: Direction, Buffering, Closing

### Direction

Always narrow channel direction at API boundaries.

```go
// Producer returns receive-only
func Producer(ctx context.Context) <-chan Event { ... }

// Consumer takes receive-only
func Consume(in <-chan Event) { ... }

// Adapter takes a receive and returns a receive
func Filter(in <-chan Event, pred func(Event) bool) <-chan Event { ... }
```

Inside the function, the channel is bidirectional. The narrowing is for callers.

### Buffering

| Buffered? | Behaviour | Use When |
|-----------|-----------|----------|
| Unbuffered (`make(chan T)`) | Sender blocks until receiver is ready | Default; you want synchronous handoff |
| Small buffer (1-N) | Sender absorbs N items before blocking | You have a known burst pattern |
| Large buffer | Sender effectively never blocks | Almost never -- usually a bug |

Bigger buffers don't make code faster. They make problems harder to diagnose because they hide backpressure.

### Closing

The single rule: **only the sender closes the channel, and only when it knows no more sends will happen.**

```go
// RIGHT -- producer goroutine owns close
func Producer(ctx context.Context) <-chan Event {
    out := make(chan Event)
    go func() {
        defer close(out) // closed when the goroutine exits
        for ; ; {
            select {
            case <-ctx.Done():
                return
            case out <- nextEvent():
            }
        }
    }()
    return out
}

// WRONG -- receiver closes (panic if producer sends again)
// WRONG -- multiple producers, each closes (panic on second close)
```

For multiple producers, use a `sync.WaitGroup` to wait for all producers, then close once:

```go
var wg sync.WaitGroup
out := make(chan Event)

for i := 0; i < producers; i++ {
    wg.Add(1)
    go func() {
        defer wg.Done()
        // produce
        out <- e
    }()
}

go func() {
    wg.Wait()
    close(out)
}()
```

### Reading a Closed Channel

Reading from a closed channel returns the zero value and `ok = false`:

```go
v, ok := <-ch
if !ok {
    // channel closed and drained
}

// for-range on a channel exits when it's closed
for v := range ch {
    process(v)
}
```

### Sending to a Closed Channel Panics

```go
close(ch)
ch <- 1  // panic: send on closed channel
```

This is why "only the sender closes" is the rule. Receivers don't have the information needed to safely close.

---

## 4. select

`select` picks one ready case at random. If none are ready, it blocks.

```go
select {
case v := <-ch1:
    handle(v)
case ch2 <- x:
    // sent
case <-ctx.Done():
    return ctx.Err()
case <-time.After(1 * time.Second):
    // timeout
default:
    // none ready -- non-blocking poll
}
```

### The Done Channel Idiom

Always include `<-ctx.Done()` in any `select` that blocks. Without it, your goroutine cannot be cancelled.

```go
for {
    select {
    case <-ctx.Done():
        return ctx.Err()
    case msg := <-in:
        if err := process(ctx, msg); err != nil {
            return err
        }
    }
}
```

### time.After Allocates -- Use time.NewTimer in Hot Loops

```go
// WRONG in a hot loop -- allocates a new timer each iteration
for {
    select {
    case <-time.After(1 * time.Second):
    case <-ch:
    }
}

// RIGHT -- reuse a timer
timer := time.NewTimer(1 * time.Second)
defer timer.Stop()
for {
    timer.Reset(1 * time.Second)
    select {
    case <-timer.C:
    case <-ch:
        if !timer.Stop() {
            <-timer.C // drain
        }
    }
}
```

For timeouts attached to a context, prefer `context.WithTimeout` over `time.After`.

---

## 5. errgroup

`golang.org/x/sync/errgroup.Group` is the right tool for "run N things concurrently, fail if any fail, cancel siblings on first failure."

### Basic Pattern

```go
import "golang.org/x/sync/errgroup"

func FetchAll(ctx context.Context, urls []string) ([][]byte, error) {
    g, ctx := errgroup.WithContext(ctx)
    results := make([][]byte, len(urls))

    for i, url := range urls {
        i, url := i, url // (only needed pre Go 1.22)
        g.Go(func() error {
            data, err := fetch(ctx, url)
            if err != nil {
                return fmt.Errorf("fetch %s: %w", url, err)
            }
            results[i] = data
            return nil
        })
    }

    if err := g.Wait(); err != nil {
        return nil, err
    }
    return results, nil
}
```

Three things to notice:

1. `errgroup.WithContext` returns a derived context that's cancelled when the first goroutine returns an error.
2. We write into `results[i]` directly -- no shared map, no mutex needed, because each goroutine writes a unique index.
3. `g.Wait()` returns the first non-nil error.

### Bounded Concurrency

```go
g.SetLimit(10) // never run more than 10 goroutines at once
```

Use this when fanning out a large number of tasks to avoid exhausting some resource (file handles, DB connections).

### When NOT to Use errgroup

- You need partial results even if some fail -- collect errors yourself.
- You need different cancellation semantics (continue on error) -- use raw goroutines + WaitGroup.

---

## 6. sync Primitives Deep Dive

### sync.Mutex

```go
var (
    mu    sync.Mutex
    cache = map[string]string{}
)

func Get(k string) string {
    mu.Lock()
    defer mu.Unlock()
    return cache[k]
}
```

Idioms:

- Always `defer mu.Unlock()` on the line after `mu.Lock()`.
- Keep critical sections short. Compute outside, write inside.
- Never call user code (interfaces, callbacks) while holding a lock -- deadlock risk.

### sync.RWMutex

```go
var (
    mu    sync.RWMutex
    cache = map[string]string{}
)

func Get(k string) (string, bool) {
    mu.RLock()
    defer mu.RUnlock()
    v, ok := cache[k]
    return v, ok
}

func Set(k, v string) {
    mu.Lock()
    defer mu.Unlock()
    cache[k] = v
}
```

The lock is more expensive than `sync.Mutex` per operation. Only use it when you've measured that read contention dominates.

### sync.WaitGroup

```go
var wg sync.WaitGroup
for _, item := range items {
    wg.Add(1)
    go func(item Item) {
        defer wg.Done()
        process(item)
    }(item)
}
wg.Wait()
```

Two rules:

- `wg.Add` must be called by the parent goroutine, before launching the child. Adding inside the child races with `Wait`.
- `wg.Done` must be deferred, so a panic still decrements.

For most worker pool patterns, `errgroup` is cleaner. Use raw `WaitGroup` only when you don't need error propagation.

### sync.Once

```go
var (
    once     sync.Once
    instance *Service
)

func Default() *Service {
    once.Do(func() {
        instance = newService()
    })
    return instance
}
```

The body of `Do` runs exactly once across all goroutines. Subsequent calls block until the first one finishes, then return immediately.

### sync.Map

```go
var m sync.Map

m.Store("key", value)
v, ok := m.Load("key")
m.Delete("key")
m.Range(func(k, v interface{}) bool {
    return true // continue
})
```

`sync.Map` is optimised for two patterns:

1. Cache-like: writes are rare after initial population.
2. Many goroutines each read/write disjoint key sets.

For the general case, prefer a plain `map` + `sync.RWMutex`. The plain version is often faster because it avoids `sync.Map`'s amortised complexity.

### sync.Pool

```go
var bufPool = sync.Pool{
    New: func() interface{} {
        return bytes.NewBuffer(make([]byte, 0, 1024))
    },
}

func process(data []byte) {
    buf := bufPool.Get().(*bytes.Buffer)
    defer func() {
        buf.Reset()
        bufPool.Put(buf)
    }()
    // use buf
}
```

Caveats:

- Items in the pool can be GC'd between uses. Do not store anything that requires explicit cleanup.
- `Pool` adds value only when allocations are expensive and frequent. Profile first.

### sync/atomic

For simple counters and flags, `atomic` is cheaper than a mutex.

```go
import "sync/atomic"

type Counter struct {
    n atomic.Int64
}

func (c *Counter) Inc()         { c.n.Add(1) }
func (c *Counter) Value() int64 { return c.n.Load() }

type FlagOnce struct {
    set atomic.Bool
}

func (f *FlagOnce) TrySet() bool { return f.set.CompareAndSwap(false, true) }
```

Go 1.19 added typed atomic wrappers (`atomic.Int64`, `atomic.Pointer[T]`). Use these instead of the older `atomic.LoadInt64` / `atomic.StoreInt64` functions.

---

## 7. Common Concurrency Patterns

### Pipeline

Each stage takes `<-chan In` and returns `<-chan Out`. Stages run concurrently, items flow through.

```go
func gen(ctx context.Context, items []int) <-chan int {
    out := make(chan int)
    go func() {
        defer close(out)
        for _, x := range items {
            select {
            case <-ctx.Done(): return
            case out <- x:
            }
        }
    }()
    return out
}

func square(ctx context.Context, in <-chan int) <-chan int {
    out := make(chan int)
    go func() {
        defer close(out)
        for x := range in {
            select {
            case <-ctx.Done(): return
            case out <- x * x:
            }
        }
    }()
    return out
}

// Wire up
ctx, cancel := context.WithCancel(context.Background())
defer cancel()
results := square(ctx, gen(ctx, []int{1, 2, 3, 4}))
for r := range results {
    fmt.Println(r)
}
```

### Fan-Out / Fan-In

```go
func fanOut(in <-chan Job, n int) []<-chan Result {
    outs := make([]<-chan Result, n)
    for i := 0; i < n; i++ {
        outs[i] = worker(in)
    }
    return outs
}

func fanIn(ins ...<-chan Result) <-chan Result {
    out := make(chan Result)
    var wg sync.WaitGroup
    for _, in := range ins {
        wg.Add(1)
        go func(in <-chan Result) {
            defer wg.Done()
            for r := range in {
                out <- r
            }
        }(in)
    }
    go func() {
        wg.Wait()
        close(out)
    }()
    return out
}
```

### Rate Limiting

```go
import "golang.org/x/time/rate"

limiter := rate.NewLimiter(10, 1) // 10 events per second, burst 1

for _, req := range requests {
    if err := limiter.Wait(ctx); err != nil {
        return err
    }
    process(req)
}
```

### Semaphore (Bounded Parallelism Without errgroup)

```go
sem := make(chan struct{}, 10)
var wg sync.WaitGroup

for _, item := range items {
    sem <- struct{}{} // acquire
    wg.Add(1)
    go func(item Item) {
        defer wg.Done()
        defer func() { <-sem }() // release
        process(item)
    }(item)
}
wg.Wait()
```

`golang.org/x/sync/semaphore` provides a more featureful version with weighted acquires.

---

## 8. Detecting and Diagnosing Deadlocks

Run with the race detector:

```bash
go test -race ./...
go run -race ./cmd/server
```

If the runtime detects all goroutines blocked, it prints a deadlock message and exits:

```
fatal error: all goroutines are asleep - deadlock!
```

This catches the easy case. The hard case is partial deadlock: some goroutines blocked, some still running. Diagnose with `pprof`:

```go
import _ "net/http/pprof"
// ...
go func() { http.ListenAndServe("localhost:6060", nil) }()
```

Then:

```bash
curl http://localhost:6060/debug/pprof/goroutine?debug=2 > goroutines.txt
```

Look for goroutines blocked on `sync.runtime_SemacquireMutex`, `chansend`, or `chanrecv`. Their stack traces tell you which lock or channel.

---

## 9. Cross-References

- [error-handling.md](error-handling.md) -- error propagation across goroutines
- [profiling.md](profiling.md) -- pprof and trace for diagnosing concurrency issues
- [testing-go.md](testing-go.md) -- testing concurrent code, goleak setup
- [full-guide.md](full-guide.md) -- project layout, modules, std library
