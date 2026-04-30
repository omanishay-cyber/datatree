# fireworks-go -- Profiling Reference

> pprof, execution traces, allocation reduction.
> Measure first. Optimise second. Never the other way around.

---

## 1. The Profiling Discipline

Performance work has a strict order:

1. **Measure** -- get a baseline. Without one, you cannot tell if a change helped.
2. **Profile** -- find the hot spot. Most code is not on a hot path; do not waste time on it.
3. **Hypothesise** -- name the bottleneck (CPU, memory, lock contention, syscall, GC).
4. **Change one thing** -- isolate the variable.
5. **Re-measure** -- confirm the change helped, by how much, and that nothing else regressed.
6. **Decide** -- keep, revert, or iterate.

Skipping step 1 or 2 is the most common mistake. Premature optimisation is not just a maxim; it is a measurable engineering anti-pattern.

---

## 2. pprof Setup

### In a Long-Running Server

```go
import (
    "net/http"
    _ "net/http/pprof" // registers /debug/pprof/* handlers
)

func main() {
    go func() {
        http.ListenAndServe("localhost:6060", nil)
    }()
    // ... your real server
}
```

The blank import is what registers the handlers. The pprof endpoints expose raw profiling data; do not expose them on a public address. Bind to localhost or a private network only.

### In a Test or Benchmark

```bash
go test -bench=. -cpuprofile=cpu.out -memprofile=mem.out
go test -bench=. -trace=trace.out
```

Profiles are written to files; analyse them after the test finishes.

### In a CLI Tool

```go
import "runtime/pprof"

func main() {
    if *cpuprofile != "" {
        f, _ := os.Create(*cpuprofile)
        defer f.Close()
        pprof.StartCPUProfile(f)
        defer pprof.StopCPUProfile()
    }
    // ... do work
}
```

---

## 3. CPU Profiling

```bash
# 30-second sample from a running server
go tool pprof http://localhost:6060/debug/pprof/profile?seconds=30
```

Inside pprof:

```
(pprof) top10            # 10 hottest functions, by cumulative time
(pprof) top10 -cum       # ranked by cumulative (callers)
(pprof) list MyFunction  # source view with per-line cost
(pprof) web              # opens an SVG callgraph in the browser (requires graphviz)
(pprof) peek MyFunction  # callers and callees of one function
```

### Reading top10

```
(pprof) top10
Showing top 10 nodes out of 142
      flat  flat%   sum%        cum   cum%
     1.2s 24.0% 24.0%     1.2s 24.0%  runtime.kevent
     0.8s 16.0% 40.0%     0.9s 18.0%  encoding/json.encodeStringValue
     0.6s 12.0% 52.0%     1.5s 30.0%  github.com/example/server.(*Handler).Get
```

Two columns matter:

- **flat** -- time spent in the function itself (excluding callees).
- **cum** -- time spent in the function plus everything it called.

If `flat` is high, the function itself is hot. Optimise its body.
If `cum` is high but `flat` is low, the function is slow because of what it calls. Look at its callees.

### Sample-Based, Not Instrumented

CPU profiling samples the call stack at ~100Hz. It does not instrument every function call. Implication: very fast functions are statistically invisible; very tail-heavy distributions need longer samples to characterise.

---

## 4. Memory Profiling

```bash
go tool pprof http://localhost:6060/debug/pprof/heap
```

Or via test:

```bash
go test -bench=. -memprofile=mem.out -benchmem
go tool pprof mem.out
```

Inside pprof:

```
(pprof) top10
(pprof) list MyFunction
(pprof) sample_index=alloc_objects  # number of allocations, not bytes
(pprof) sample_index=alloc_space    # bytes allocated (default)
(pprof) sample_index=inuse_objects  # live allocations
(pprof) sample_index=inuse_space    # live bytes (default for heap)
```

### inuse vs alloc

- `inuse_*` -- what is currently retained.
- `alloc_*` -- what was allocated since the program started.

For memory leaks: look at `inuse_space` increasing over time. For GC pressure: look at `alloc_space` to find allocation hot paths.

### Common Allocation Hot Paths

- String concatenation in a loop -- use `strings.Builder` or `bytes.Buffer`.
- `fmt.Sprintf` for hot paths -- use `strconv.AppendInt`/`AppendFloat` directly.
- Returning slices that grow -- pre-size with `make([]T, 0, expectedLen)`.
- `interface{}` boxing of small values -- generics or concrete types remove this.
- `time.After` in a hot loop -- it allocates a timer; use `time.NewTimer` once.
- JSON encode/decode of small messages -- `encoding/json` is reflective and slow; consider `easyjson`/`segmentio/encoding/json` for hot paths.

---

## 5. Goroutine Profiling

```bash
curl http://localhost:6060/debug/pprof/goroutine?debug=2 > goroutines.txt
```

The `debug=2` form gives full stack traces for every live goroutine, grouped by what they're doing. Read this when:

- The process feels sluggish but CPU is low (goroutines blocked).
- You suspect a goroutine leak.
- You're diagnosing a deadlock.

Common patterns to look for:

- Many goroutines blocked on `chansend` -- something is not draining the channel.
- Many goroutines blocked on `sync.runtime_SemacquireMutex` -- lock contention.
- Goroutine count increasing over time without bound -- leak.

---

## 6. Block Profiling and Mutex Profiling

These are off by default because they have measurable runtime cost.

```go
import "runtime"

func main() {
    runtime.SetBlockProfileRate(1)        // 1 = sample every event
    runtime.SetMutexProfileFraction(1)
    // ...
}
```

```bash
go tool pprof http://localhost:6060/debug/pprof/block
go tool pprof http://localhost:6060/debug/pprof/mutex
```

The block profile shows where goroutines block on channel operations. The mutex profile shows where they contend on locks.

Use these only when you've identified contention as a likely bottleneck (long GC pauses, low CPU but slow, or `pprof.lookup("threadcreate")` is high).

---

## 7. Execution Trace

The trace is more detailed than profiles -- it captures every scheduling event, GC pause, syscall.

```bash
go test -trace=trace.out ./mypackage
go tool trace trace.out
```

The web UI (`go tool trace` opens a browser) shows:

- Goroutine timelines: when each one is running, blocked, or runnable.
- GC events: stop-the-world pauses, mark phases.
- Syscall blocking: which calls block which goroutines and for how long.
- Network and synchronization events.

Use traces when:

- A profile shows nothing obvious but the program is slow.
- You suspect scheduler-related issues (e.g., starvation).
- You need to see GC behaviour over time.

---

## 8. Benchmark Workflows

### Single Benchmark

```go
func BenchmarkParseOrderID(b *testing.B) {
    for i := 0; i < b.N; i++ {
        _, _ = ParseOrderID("ord_abc123")
    }
}
```

```bash
go test -bench=BenchmarkParseOrderID -benchmem
```

### Sub-Benchmarks for Variants

```go
func BenchmarkParse(b *testing.B) {
    inputs := []struct {
        name  string
        input string
    }{
        {"short", "ord_a"},
        {"medium", "ord_abc123def"},
        {"long", "ord_" + strings.Repeat("a", 60)},
    }
    for _, in := range inputs {
        b.Run(in.name, func(b *testing.B) {
            b.ResetTimer()
            for i := 0; i < b.N; i++ {
                _, _ = ParseOrderID(in.input)
            }
        })
    }
}
```

### Comparing Before and After with benchstat

```bash
go install golang.org/x/perf/cmd/benchstat@latest

# baseline
go test -bench=. -count=10 ./mypackage > old.txt

# make change
git apply optimisation.patch

# new
go test -bench=. -count=10 ./mypackage > new.txt

benchstat old.txt new.txt
```

```
name           old time/op    new time/op    delta
Parse-8          24.3ns +/- 1%    18.1ns +/- 2%   -25.5%  (p=0.000 n=10+10)
Parse-8         8.0B/op       0.0B/op            -100.0%
Parse-8        1 allocs/op   0 allocs/op         -100.0%
```

`p < 0.05` indicates a statistically significant difference. Repeat counts of 10+ are typical for noisy systems.

### Benchmark Sanity Checks

- Always `b.ResetTimer()` after setup.
- Always include `-benchmem` for allocation cost.
- Use realistic inputs. Synthetic ones (like 0-length slices) are misleading.
- Run on a quiet machine. Background work skews timings.

---

## 9. Allocation Reduction Recipes

### Pre-Size Slices

```go
// BAD
var out []Result
for _, x := range xs {
    out = append(out, transform(x))
}

// GOOD
out := make([]Result, 0, len(xs))
for _, x := range xs {
    out = append(out, transform(x))
}
```

`append` doubles the capacity each time it grows. Pre-sizing skips the regrowing.

### Reuse Buffers via sync.Pool

```go
var bufPool = sync.Pool{
    New: func() interface{} { return new(bytes.Buffer) },
}

func handle(w http.ResponseWriter, r *http.Request) {
    buf := bufPool.Get().(*bytes.Buffer)
    defer func() {
        buf.Reset()
        bufPool.Put(buf)
    }()
    // use buf
}
```

The pool amortises allocation cost across requests. Critical for hot HTTP paths.

### Avoid Interface Boxing

```go
// BAD -- boxing each int into interface{}
var values []interface{}
for i := 0; i < 1000; i++ {
    values = append(values, i)
}

// GOOD -- generic, no boxing
var values []int
```

When you must use `interface{}` (or `any`), each non-zero-size value is boxed in a heap allocation. Generics eliminate the boxing.

### Strings vs []byte

If you read bytes from network/disk and immediately convert to string, you double-copy.

```go
// Standard but allocates
s := string(buf)

// Avoids the copy when you control both ends
import "unsafe"
s := unsafe.String(&buf[0], len(buf))
```

The `unsafe` version is only safe if you guarantee `buf` is not modified for the lifetime of `s`. Use sparingly.

### Avoid fmt.Sprintf on Hot Paths

```go
// Allocates on every call
key := fmt.Sprintf("user:%d:profile", userID)

// Cheaper
var buf [32]byte
key := strconv.AppendInt(append(buf[:0], "user:"...), int64(userID), 10)
key = append(key, ":profile"...)
```

Only matters in hot paths. Profile to confirm.

---

## 10. GC Tuning

Go's GC is concurrent and doesn't typically need tuning. Two knobs exist:

### GOGC

```bash
GOGC=200 ./myserver
```

`GOGC=100` (default) means GC runs when heap doubles. `GOGC=200` means GC runs when heap triples. Higher = less CPU, more memory.

### GOMEMLIMIT (Go 1.19+)

```bash
GOMEMLIMIT=4GiB ./myserver
```

Soft limit on total memory. The GC becomes more aggressive as memory approaches the limit. Useful in containers to avoid OOM.

Do not tune these without measuring. Defaults are good for almost every workload.

---

## 11. Diagnosing GC Pauses

```bash
GODEBUG=gctrace=1 ./myserver
```

Output:

```
gc 1 @0.012s 0%: 0.011+0.32+0.017 ms clock, 0.090+0/0.32/0.30+0.13 ms cpu
```

Read this as: GC #1 at t=0.012s used 0% wall clock, with phases of 0.011ms / 0.32ms / 0.017ms (mark prep / mark / mark term).

A typical Go server should show:

- Pauses well under 1ms.
- GC running every few seconds, not constantly.
- Heap size stable in steady state.

If pauses exceed 10ms or GC dominates CPU, you have allocation pressure. Profile heap allocations and reduce them.

---

## 12. Profile-Guided Optimisation (Go 1.21+)

PGO uses a CPU profile from production to guide compiler optimisations.

```bash
# Collect a profile from production
curl http://prod-server/debug/pprof/profile?seconds=120 > default.pgo

# Build with PGO
go build -pgo=default.pgo ./cmd/server
```

Typical wins are 2-7% on hot paths. The profile must be representative of production workload.

PGO is opt-in but cheap. Worth enabling for any production server.

---

## 13. Reading flat vs cum -- Worked Example

Suppose `top10` shows:

```
      flat  flat%   sum%        cum   cum%
       0s    0.0%   0.0%      8.0s  80.0%  main.handleRequest
     6.5s   65.0%  65.0%      6.5s  65.0%  encoding/json.encodeValue
     1.0s   10.0%  75.0%      1.0s  10.0%  runtime.mallocgc
```

Interpretation:

- `handleRequest` has flat=0 cum=80%. It does no work itself; everything's in callees.
- `encoding/json.encodeValue` has flat=65% cum=65%. The bottleneck is here.
- `runtime.mallocgc` has flat=10%. Significant allocation cost; consider reducing it.

Action: focus on JSON encoding. Either pre-marshal hot responses, switch to a faster encoder, or cache the encoded form.

---

## 14. Production Profiling Safely

If you must profile a production server:

- Bind pprof to localhost or a private network only.
- Use shorter sample windows (10-30 seconds).
- Profile during representative load, not idle time.
- Capture the profile, then analyse offline. Do not run interactive pprof against production.

```bash
curl -o cpu.out http://internal-server:6060/debug/pprof/profile?seconds=30
go tool pprof cpu.out  # offline
```

---

## 15. Cross-References

- [concurrency.md](concurrency.md) -- diagnosing lock contention and goroutine blocking
- [testing-go.md](testing-go.md) -- benchmark structure
- [error-handling.md](error-handling.md) -- error path performance
- [full-guide.md](full-guide.md) -- standard library performance notes
