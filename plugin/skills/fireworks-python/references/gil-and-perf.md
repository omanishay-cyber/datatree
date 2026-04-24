# fireworks-python -- GIL and Performance

> The GIL, profiling, multiprocessing, when to drop to Rust.
> How to make Python fast -- and when to stop trying.

---

## 1. The GIL in One Page

The Global Interpreter Lock is a mutex around the Python interpreter. At any moment, only one thread executes Python bytecode. It was added to make reference counting thread-safe without per-object locks, and it's been the biggest single source of "Python is slow" folklore.

The GIL is released during:

- Blocking I/O (network reads/writes, disk I/O).
- `time.sleep`.
- Calls into C extensions that explicitly release it (NumPy, Pandas, PIL, cryptography, etc.).

The GIL is held during:

- Pure-Python CPU work (loops, arithmetic, string manipulation).
- Object creation and method dispatch.
- Most standard library calls.

### Implications

| Workload | Threads | Asyncio | Multiprocessing | Native |
|----------|---------|---------|-----------------|--------|
| Network I/O bound | Helps | Helps more | Overkill | Rarely needed |
| Disk I/O bound | Helps | Helps | Rarely helps | Use OS async I/O |
| Pure Python CPU | No help | No help | Helps | Best option |
| NumPy/Pandas CPU | Helps | No help | Helps | Best option |
| Mixed | Profile | Profile | Profile | Profile |

---

## 2. Free-Threaded Python (3.13+)

Python 3.13 introduced an experimental no-GIL build, called free-threaded or nogil. Install with:

```bash
# uv
uv python install 3.13t

# pyenv
pyenv install 3.13-nogil
```

Most pure-Python code works unmodified. C extensions need opt-in support; compatibility is still maturing in 2025-2026.

### When to Consider Free-Threaded

- CPU-bound workload, willing to be cutting-edge.
- Dependencies explicitly support free-threaded mode.
- Willing to deal with experimental status (e.g., some races surface that the GIL was hiding).

### When to Skip It

- Production service today.
- Heavy dependence on PyTorch, TensorFlow, or older C extensions (most are catching up but not all).
- I/O-bound workload (free-threaded doesn't help).

---

## 3. Choosing Your Concurrency Model

### asyncio: I/O-Bound at Scale

```python
async def fetch_all(urls: list[str]) -> list[bytes]:
    async with aiohttp.ClientSession() as s:
        async with asyncio.TaskGroup() as tg:
            tasks = [tg.create_task(fetch_one(s, u)) for u in urls]
    return [t.result() for t in tasks]
```

Strengths:

- Single process, no inter-process communication.
- Thousands of concurrent operations.
- Deterministic scheduling (in theory).

Weaknesses:

- The whole program must be async. Sync calls block the loop.
- CPU work starves the loop.
- Debugging requires understanding scheduler behaviour.

### threading: I/O-Bound, Small Scale

```python
from concurrent.futures import ThreadPoolExecutor

with ThreadPoolExecutor(max_workers=10) as pool:
    results = list(pool.map(fetch_one, urls))
```

Strengths:

- Works with existing sync code.
- Simple programming model.
- Good for 10s or low 100s of concurrent operations.

Weaknesses:

- Each thread has memory overhead (~MB per thread).
- Context switches have real cost.
- No help for CPU-bound pure Python.

### multiprocessing: CPU-Bound

```python
from multiprocessing import Pool

def process_chunk(chunk: list[int]) -> int:
    return sum(x * x for x in chunk)

if __name__ == "__main__":
    with Pool() as pool:
        results = pool.map(process_chunk, chunks)
```

Strengths:

- True parallelism, one GIL per process.
- Crashes isolated to one worker.
- Good for CPU-bound Python that doesn't call into C.

Weaknesses:

- Startup cost per process.
- IPC requires serialisation -- slow for large arguments.
- Memory duplication (one process's heap is independent).

### subprocess: Fully Separate Programs

```python
result = subprocess.run(
    ["./fast_rust_tool", "--input", "data.json"],
    capture_output=True,
    text=True,
    check=True,
)
```

For when you want a completely separate process. Good for isolation, language polyglot, or running untrusted code.

---

## 4. Profiling: Where Is the Time Going?

### cProfile

```bash
python -m cProfile -o profile.out -s cumulative myscript.py
```

Or programmatically:

```python
import cProfile, pstats

with cProfile.Profile() as pr:
    run_workload()

pr.dump_stats("profile.out")
stats = pstats.Stats("profile.out").sort_stats("cumulative")
stats.print_stats(20)
```

### Reading cProfile Output

```
   ncalls  tottime  percall  cumtime  percall filename:lineno(function)
     1000    0.123    0.000    0.850    0.001 mymodule.py:45(process)
     1000    0.720    0.001    0.720    0.001 json:encode()
        1    0.010    0.010    0.900    0.900 myscript.py:10(main)
```

- `ncalls` -- how many times the function was called.
- `tottime` -- time spent inside the function itself.
- `cumtime` -- time spent inside plus in callees.

If `tottime` is high, the function is the hot spot. If `cumtime` is high but `tottime` is low, a callee is the hot spot.

### Line-Level Profiling (line_profiler)

```bash
pip install line_profiler
```

```python
@profile
def hot_function(items: list[int]) -> int:
    total = 0
    for x in items:
        total += x * x  # line-by-line cost visible
    return total
```

```bash
kernprof -l -v myscript.py
```

Line profiling is slow (orders of magnitude overhead), but it tells you exactly which line in a function is the bottleneck.

### py-spy: Sampling Profiler for Running Processes

```bash
pip install py-spy
py-spy record -o flame.svg --pid 1234
py-spy dump --pid 1234   # current stack of every thread
py-spy top --pid 1234    # live top-style view
```

Zero instrumentation cost. Samples the running process from the outside. Ideal for production diagnosis.

### Flame Graphs

```bash
py-spy record -o flame.svg -- python myscript.py
```

The SVG opens in a browser. Each bar is a function; width shows time spent. Click a bar to zoom. The single most intuitive way to see where time goes.

---

## 5. Memory Profiling

### tracemalloc (stdlib)

```python
import tracemalloc

tracemalloc.start()
# ... run workload ...
snapshot = tracemalloc.take_snapshot()
for stat in snapshot.statistics("lineno")[:10]:
    print(stat)
```

Shows where in the code memory was allocated. Good for leak hunting.

### memory_profiler

```bash
pip install memory_profiler
```

```python
@profile
def memory_hungry():
    x = [i for i in range(10_000_000)]
    y = [i * 2 for i in x]
    return sum(y)
```

```bash
python -m memory_profiler myscript.py
mprof run myscript.py
mprof plot  # visualise memory over time
```

### Common Memory Bloat Causes

- Unbounded caches (`lru_cache(maxsize=None)`).
- Large objects retained by closures.
- Circular references holding objects past useful life.
- Pandas DataFrames not released after use.
- Accumulating results in a list instead of streaming.

---

## 6. Optimisation Recipes

Ordered by leverage (biggest wins first).

### Use Generators for Streaming

```python
# BAD -- materialises full list
results = [transform(x) for x in huge_file]

# GOOD -- streams
results = (transform(x) for x in huge_file)
total = sum(results)
```

Saves memory proportional to the input size.

### Avoid Repeated Lookups

```python
# BAD -- attribute lookup per iteration
for x in items:
    obj.list.append(transform(x))

# GOOD -- hoist lookups
append = obj.list.append
for x in items:
    append(transform(x))
```

Modest speedup. Only worth it on tight hot loops.

### Use Built-ins

```python
# BAD
total = 0
for x in items:
    total += x

# GOOD
total = sum(items)
```

Built-ins are implemented in C and release the GIL appropriately. Often 10x faster than pure-Python equivalents.

### Prefer Comprehensions to Map/Filter

```python
# Slower
result = list(map(lambda x: x * 2, items))

# Faster and clearer
result = [x * 2 for x in items]
```

Comprehensions are compiled to specialised bytecode that's faster than map+lambda.

### Pre-Size Lists

```python
# Slower
result = []
for x in items:
    result.append(transform(x))

# Faster when size is known
result = [None] * len(items)
for i, x in enumerate(items):
    result[i] = transform(x)

# Still faster
result = [transform(x) for x in items]
```

The comprehension is the fastest. Pre-sizing only matters for cases where you can't use a comprehension.

### String Concatenation

```python
# BAD -- O(n^2) for n concatenations
s = ""
for chunk in chunks:
    s += chunk

# GOOD -- O(n)
s = "".join(chunks)
```

### Use `slots` for Many Instances

```python
@dataclass(slots=True)
class Point:
    x: float
    y: float
```

`slots=True` eliminates the per-instance `__dict__`. Halves memory and speeds up attribute access. Use for classes that have many instances (millions of points, records, etc.).

### Vectorise with NumPy

```python
import numpy as np

# Python loop
result = [x * x + y * y for x, y in zip(xs, ys)]

# NumPy
xs_np = np.array(xs)
ys_np = np.array(ys)
result = xs_np ** 2 + ys_np ** 2
```

NumPy operations run in C and release the GIL. Typical speedup: 10-100x for numeric loops.

---

## 7. When to Drop to Native Code

Decision tree:

```
Is the code CPU-bound?
  No -> asyncio or threading is enough
  Yes -> continue

Is the bottleneck in a vectorisable loop?
  Yes -> NumPy / Pandas / Polars
  No -> continue

Is the hot path 1-3 functions?
  Yes -> Cython or Numba for those
  No -> continue

Do you need a persistent performance win across a module?
  Yes -> Rust via pyo3
```

### Numba (JIT for numeric code)

```python
from numba import jit

@jit(nopython=True)
def hot_loop(arr):
    total = 0.0
    for i in range(arr.shape[0]):
        total += arr[i] * arr[i]
    return total
```

Pros: zero build tooling, works on pure Python with numbers.
Cons: limited type support; cold start is slow.

### Cython (ahead-of-time compiled)

```python
# fast.pyx
def hot_loop(list items) -> int:
    cdef int total = 0
    cdef int x
    for x in items:
        total += x * x
    return total
```

Pros: full Python compatibility, optional static types for speed.
Cons: separate build step; project-specific tooling.

### Rust via pyo3

```rust
use pyo3::prelude::*;

#[pyfunction]
fn hot_loop(items: Vec<i64>) -> i64 {
    items.iter().map(|x| x * x).sum()
}

#[pymodule]
fn mypackage(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hot_loop, m)?)?;
    Ok(())
}
```

```toml
[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"
```

```bash
maturin develop --release
```

Pros: 10-100x speedup is typical; memory safety; can release GIL for true parallelism.
Cons: Rust learning curve; extra build tooling.

For sustained hot paths, Rust via pyo3 is the current best answer. Some major libraries (cryptography, pydantic) rewrote hot paths in Rust and saw transformative improvements.

---

## 8. multiprocessing Gotchas

### On Windows and macOS (spawn)

Child processes re-import the main module. Any top-level code runs again per worker. Always guard with:

```python
if __name__ == "__main__":
    with Pool() as pool:
        ...
```

Without the guard, spawn creates an infinite fork bomb.

### Argument and Return Serialisation

```python
# BAD -- each worker receives a full copy of big_array
with Pool() as pool:
    pool.map(process, [big_array, big_array, big_array])

# BETTER -- use shared memory or memory-mapped files
from multiprocessing import shared_memory
```

Serialisation is slow for large objects. For sharing big numpy arrays, use `shared_memory` or memmap files.

### Daemon Threads and multiprocessing

multiprocessing workers cannot themselves spawn daemon processes. Don't nest pools.

### macOS Fork Safety

On macOS, `fork` can interact badly with Objective-C runtimes. Since Python 3.8, the default start method on macOS is `spawn`, not `fork`. Most scientific code assumes fork; you may need:

```python
import multiprocessing as mp
mp.set_start_method("fork")  # if safe for your codebase
```

---

## 9. GPU Acceleration

For numeric code, GPUs blow everything else away.

- **cupy** -- drop-in NumPy replacement on NVIDIA GPUs.
- **pytorch** -- tensor operations, autograd.
- **jax** -- differentiable NumPy with JIT.

```python
import cupy as cp

a = cp.arange(1_000_000)
b = cp.arange(1_000_000)
c = a + b  # runs on GPU
```

Threshold: roughly >1M elements before GPU transfer cost is amortised. Smaller arrays are faster on CPU.

---

## 10. Real-World Performance Workflow

1. Does the code run? If not, correctness first.
2. Is the runtime acceptable? If yes, stop. Premature optimisation is still premature.
3. Profile (cProfile, py-spy). Identify the top 1-2 hotspots.
4. Apply the cheapest fix:
   - Is the algorithm O(n^2) when it could be O(n log n)? Fix that first.
   - Is there an obvious built-in or library that replaces the loop?
5. Re-profile. Confirm the fix landed.
6. Iterate.
7. Only after exhausting Python-level fixes: consider Cython / Numba / Rust.

Most "slow Python" can be fixed by changing one or two lines or switching a library. Reaching for native code before profiling is almost always wasted work.

---

## 11. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| Optimising without a profile | Measure first |
| Using threading for CPU-bound pure Python | multiprocessing or native |
| Calling Python from a hot C loop | Push the whole loop to C |
| Large argument serialisation in multiprocessing | Shared memory or memmap |
| Premature async conversion | Async is a means, not an end |
| Loop in Python what NumPy does in C | Vectorise |
| Global dict for shared state across processes | Use manager or shared memory |
| Unbounded lru_cache | Set maxsize |
| Profiling one run | Run multiple times; compare with benchstat equivalent |
| Benchmarking with random data every run | Fix seeds for reproducibility |

---

## 12. Cross-References

- [asyncio-pitfalls.md](asyncio-pitfalls.md) -- async concurrency details
- [typing-deep-dive.md](typing-deep-dive.md) -- typing around multiprocessing
- [packaging-modern.md](packaging-modern.md) -- building Rust extensions
- [full-guide.md](full-guide.md) -- general discipline
