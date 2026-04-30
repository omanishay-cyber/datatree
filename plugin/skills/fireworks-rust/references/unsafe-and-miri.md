# fireworks-rust -- Unsafe Rust and Miri

> The four superpowers, invariant documentation, Miri workflow.
> When to use unsafe, how to verify it, and how to keep it small.

---

## 1. The Bargain

Safe Rust prevents undefined behaviour (UB) at compile time. `unsafe` lets you bypass specific compiler checks. The bargain:

- You get access to operations the compiler can't verify (raw pointers, FFI, mutable statics).
- You take responsibility for upholding the invariants the compiler normally enforces.

Violating those invariants is undefined behaviour: the compiler may optimise as if it never happens, and your program may do anything at all. Segfaults, corrupted data, incorrect results -- everything is on the table.

### The Cardinal Rule

**Minimise unsafe. Wrap it in a safe abstraction. Document every invariant.**

A codebase with 10,000 lines of safe Rust and 50 lines of well-audited unsafe is healthier than 10,000 lines of cautious unsafe.

---

## 2. The Four Superpowers

Inside an `unsafe` block or function, you can:

1. Dereference a raw pointer (`*const T` or `*mut T`).
2. Call an unsafe function or method.
3. Access or modify a mutable static variable.
4. Implement an unsafe trait.
5. (Bonus) Access fields of a union.

That is the entire list. Everything else (overflow checks, array bounds, ownership, borrow checks) remains enforced inside unsafe blocks.

### Raw Pointer Dereference

```rust
let x = 42;
let p: *const i32 = &x;
unsafe {
    println!("{}", *p);
}
```

Raw pointers are like C pointers. They can be null, dangling, unaligned, point to freed memory. The compiler won't stop you; you must ensure validity.

### Calling Unsafe Functions

```rust
extern "C" {
    fn strlen(s: *const i8) -> usize;
}

unsafe {
    let n = strlen(cstr.as_ptr() as *const i8);
}
```

FFI functions are unsafe because Rust can't verify their invariants.

Standard library unsafe functions include `slice::from_raw_parts`, `String::from_raw_parts`, `Vec::from_raw_parts`, `mem::transmute`, `ptr::read`, `ptr::write`.

### Mutable Statics

```rust
static mut COUNTER: u32 = 0;

fn increment() {
    unsafe {
        COUNTER += 1;  // race condition if called from multiple threads
    }
}
```

`static mut` is dangerous because multiple threads can race on it. Prefer `AtomicU32`, `OnceLock`, or `Mutex<T>` inside a `static`.

### Unsafe Traits

```rust
pub unsafe trait Soundness {
    // Implementing this trait means you promise some invariant.
}

unsafe impl Soundness for MyType {}
```

The `unsafe` on the trait declaration means implementers promise something the compiler can't check. `Send` and `Sync` are examples from stdlib -- declaring `unsafe impl Sync for T` promises that T can actually be shared safely.

---

## 3. Safety Invariants and the /// # Safety Convention

Every unsafe function documents its preconditions:

```rust
/// Reads a value from a raw pointer.
///
/// # Safety
/// - `ptr` must be non-null.
/// - `ptr` must point to a valid, initialised value of type T.
/// - The memory at `ptr` must not be mutated while this read is in progress.
/// - T must not be a reference type (use `read_volatile` otherwise).
pub unsafe fn read_ptr<T>(ptr: *const T) -> T {
    ptr.read()
}
```

Every `unsafe` block has a comment explaining why the invariants hold:

```rust
let slice: &[u8] = unsafe {
    // SAFETY: `buf` is a valid allocation of exactly `len` bytes,
    // created by `alloc` on the preceding line. The allocation is not
    // freed for the rest of this function. `u8` has alignment 1 so no
    // alignment concerns. We own this slice, no aliasing.
    std::slice::from_raw_parts(buf, len)
};
```

No `unsafe` block without a SAFETY comment. Treat this as a hard rule.

### Why Comments Matter

- Reviewers can verify the reasoning.
- Future changes to the surrounding code can be checked against documented invariants.
- Tools like `cargo geiger` count unsafe blocks; good comments make the audit productive.

---

## 4. Common Patterns for Safe Abstractions

### Pattern: Safe Wrapper Over Unsafe Internals

```rust
pub struct MyVec<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> MyVec<T> {
    pub fn push(&mut self, value: T) {
        if self.len == self.cap {
            self.grow();
        }
        unsafe {
            // SAFETY: self.len < self.cap by the grow() call above,
            // so self.ptr.add(self.len) is within the allocation.
            self.ptr.add(self.len).write(value);
        }
        self.len += 1;
    }
}
```

Users of `MyVec::push` don't see unsafe. The module internally uses unsafe for performance but presents a safe API.

### Pattern: unsafe Block in Safe fn

Not every function that uses unsafe must itself be marked unsafe. If the function internally satisfies all invariants, the public signature can be safe.

```rust
pub fn concat_strs(a: &str, b: &str) -> String {
    let total_len = a.len() + b.len();
    let mut v = Vec::with_capacity(total_len);
    unsafe {
        // SAFETY: v has capacity for total_len bytes.
        std::ptr::copy_nonoverlapping(a.as_ptr(), v.as_mut_ptr(), a.len());
        std::ptr::copy_nonoverlapping(b.as_ptr(), v.as_mut_ptr().add(a.len()), b.len());
        v.set_len(total_len);
        // SAFETY: the bytes are valid UTF-8 because a and b are valid &str.
        String::from_utf8_unchecked(v)
    }
}
```

The public `concat_strs` is safe; internal unsafe is the implementation detail.

### Pattern: Unsafe Trait with Safe Methods

```rust
/// # Safety
/// Implementers must ensure that `bytes()` returns a slice whose contents
/// are valid UTF-8.
pub unsafe trait ValidUtf8 {
    fn bytes(&self) -> &[u8];
}

pub fn render(x: &dyn ValidUtf8) -> &str {
    unsafe {
        // SAFETY: trait contract guarantees valid UTF-8.
        std::str::from_utf8_unchecked(x.bytes())
    }
}
```

The trait carries the unsafe; callers of the safe `render` rely on the trait's contract.

---

## 5. Send, Sync, and Auto Traits

`Send` and `Sync` are auto-traits: the compiler derives them based on structure.

```rust
// Auto-derived: all fields are Send + Sync
struct Order {
    id: String,
    total: u64,
}

// Auto-NOT-derived: Rc is !Send
struct Node {
    data: String,
    next: Rc<Node>,
}
```

### When to Manually Implement

```rust
pub struct FfiHandle {
    ptr: *mut ffi::Handle,
}

// The raw pointer is !Send by default.
// We know the FFI is thread-safe, so we opt in.
unsafe impl Send for FfiHandle {}
unsafe impl Sync for FfiHandle {}
```

Only `unsafe impl` -- you're asserting a fact the compiler can't check. Document why the assertion holds:

```rust
// SAFETY: The underlying C library explicitly supports concurrent use
// per the documentation at https://example.com/ffi-docs#thread-safety.
unsafe impl Send for FfiHandle {}
```

### Opt Out with PhantomData

```rust
use std::marker::PhantomData;

pub struct NotThreadSafe<T> {
    value: T,
    _not_send_sync: PhantomData<std::rc::Rc<()>>,
}
```

`PhantomData<Rc<()>>` carries the `!Send + !Sync` auto-trait status. Lets you forbid cross-thread use even though your type's actual data is fine.

---

## 6. FFI Patterns

### Calling C from Rust

```rust
extern "C" {
    fn libfoo_init() -> i32;
    fn libfoo_process(data: *const u8, len: usize) -> i32;
    fn libfoo_free(handle: *mut ffi::Handle);
}

pub fn initialise() -> Result<(), InitError> {
    let rc = unsafe { libfoo_init() };
    if rc != 0 {
        return Err(InitError::FromCode(rc));
    }
    Ok(())
}
```

Rules:

- Every C function call is unsafe.
- Validate return codes explicitly.
- Convert C error codes to Rust Result at the boundary.

### String Conversion

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

extern "C" {
    fn libfoo_greet(name: *const c_char) -> *const c_char;
}

pub fn greet(name: &str) -> Result<String, GreetError> {
    let c_name = CString::new(name)
        .map_err(|_| GreetError::InvalidName)?;

    let c_result = unsafe { libfoo_greet(c_name.as_ptr()) };
    if c_result.is_null() {
        return Err(GreetError::NullReturn);
    }

    let s = unsafe {
        // SAFETY: libfoo_greet promises to return a valid C string.
        CStr::from_ptr(c_result).to_string_lossy().into_owned()
    };
    Ok(s)
}
```

`CString` owns its memory; `CStr` is a borrow. `c_char` is `i8` on most platforms but exists to document intent.

### Exposing Rust to C (pyo3, cbindgen)

For Python extensions, use `pyo3`. For plain C consumers, use `cbindgen` to generate a header. Both handle the unsafe boundary for you.

```rust
use pyo3::prelude::*;

#[pyfunction]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[pymodule]
fn mymodule(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(add, m)?)?;
    Ok(())
}
```

pyo3 macros hide the unsafe. You write safe Rust; the macro handles ABI details.

---

## 7. Common Unsafe Pitfalls

### Pitfall: Aliasing

```rust
let mut v = vec![1, 2, 3];
let p = v.as_ptr();
let pm = v.as_mut_ptr();

unsafe {
    // UB: reading through p and writing through pm simultaneously
    let _ = *p;
    *pm = 42;
}
```

The aliasing rules still apply in unsafe. Having both `*const T` and `*mut T` pointing to the same place and using them concurrently is UB.

### Pitfall: Use After Free

```rust
let v = vec![1, 2, 3];
let p = v.as_ptr();
drop(v);
unsafe {
    let _ = *p;  // UB -- v is gone
}
```

Raw pointers don't track lifetimes. You must keep the owner alive.

### Pitfall: Misaligned Access

```rust
let bytes = [0u8; 12];
let p = bytes.as_ptr() as *const u32;
unsafe {
    let _ = *p;  // UB -- u32 requires 4-byte alignment; bytes.as_ptr() may not be aligned
}
```

Fix: use `read_unaligned`:

```rust
unsafe {
    let n: u32 = std::ptr::read_unaligned(p);
}
```

Slower but correct.

### Pitfall: Uninitialised Memory

```rust
let mut v: Vec<i32> = Vec::with_capacity(10);
unsafe {
    v.set_len(10);  // tells Vec we have 10 valid elements
}
// UB -- those 10 elements are uninitialised
for x in &v {
    println!("{}", x);
}
```

Fix: initialise before `set_len`:

```rust
let mut v: Vec<i32> = Vec::with_capacity(10);
for _ in 0..10 {
    v.push(0);
}
```

Or use `MaybeUninit`:

```rust
use std::mem::MaybeUninit;

let mut arr: [MaybeUninit<i32>; 10] = unsafe { MaybeUninit::uninit().assume_init() };
for i in 0..10 {
    arr[i].write(i as i32);
}
let arr: [i32; 10] = unsafe {
    std::mem::transmute::<_, [i32; 10]>(arr)
};
```

---

## 8. Miri: The UB Detector

Miri interprets your code at the MIR (middle intermediate representation) level. It catches:

- Use after free.
- Reading uninitialised memory.
- Out-of-bounds array access.
- Violations of aliasing rules (Stacked Borrows / Tree Borrows).
- Misaligned pointer reads/writes.
- Data races.
- Invalid transmutes.

### Install

```bash
rustup +nightly component add miri
```

Miri only runs on the nightly toolchain. Fine; it's a test-time tool.

### Run

```bash
cargo +nightly miri test
```

Runs your test suite under Miri. Slower than normal cargo test (can be 10-100x), so run it on critical paths only, typically in CI.

### Miri-Only Tests

```rust
#[test]
fn miri_catches_ub() {
    #[cfg(miri)]
    {
        // This test only runs under Miri
    }

    // Normal test logic
}

// Or skip a test under Miri if it's too slow:
#[test]
#[cfg_attr(miri, ignore = "slow under Miri")]
fn integration_test() {
    ...
}
```

### What Miri Misses

- FFI calls (Miri can't simulate them).
- Inline assembly.
- Some platform-specific behaviours.
- Real hardware races (Miri uses a simplified memory model).

For FFI, you need to trust the C side. For assembly, you need to reason manually.

---

## 9. cargo-geiger: Audit Unsafe Count

```bash
cargo install cargo-geiger
cargo geiger
```

Reports how much unsafe code exists in your crate and its dependencies. Useful for:

- Auditing before bringing in a new dependency.
- Catching regressions where a refactor added unexpected unsafe.

Don't treat "more unsafe" as inherently bad -- some crates (tokio, hyper, bytes) legitimately use substantial unsafe for performance. But surprise unsafe is a signal to investigate.

---

## 10. Alternative: Avoid Unsafe Entirely

Most code has no legitimate need for unsafe. Before reaching for it, check:

- Can the standard library do this? (`slice::split_at_mut`, `Vec::drain`, etc.)
- Is there a crate that provides a safe abstraction? (`bytes`, `parking_lot`, `crossbeam`, etc.)
- Can I restructure to avoid it?

Reach for unsafe when:

- FFI is required.
- The safe version has unacceptable performance and benchmarks prove it.
- You're implementing a fundamentally unsafe primitive (allocator, lock-free data structure, zero-copy parser).

For everything else, the safe version is fine.

---

## 11. Clippy Lints for Unsafe

```toml
# Cargo.toml or .clippy.toml
[lints.clippy]
undocumented_unsafe_blocks = "deny"
missing_safety_doc = "deny"
multiple_unsafe_ops_per_block = "warn"
```

These lints enforce:

- Every `unsafe` block has a SAFETY comment.
- Every `unsafe fn` has a `# Safety` doc section.
- Complex unsafe blocks get flagged for review.

Always enable these in any crate that uses unsafe.

---

## 12. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `unsafe` block without SAFETY comment | Document the invariants |
| `unsafe fn` without `# Safety` doc | Add the preconditions |
| Using `unsafe` for performance without benchmarks | Measure first |
| `transmute` to reinterpret bits | Use `from_ne_bytes`, `as_bytes`, or safe alternatives |
| Public `unsafe fn` where safe is possible | Restructure to accept a safe API |
| `*const T` then `*mut T` through the same pointer | Pick one access mode |
| Ignoring Miri warnings | Fix them -- they are real UB |
| `static mut` for shared state | Use `AtomicT`, `OnceLock`, or `Mutex<T>` |
| `unsafe impl Send` without understanding thread safety | Don't -- pick a type that's already Send |
| Unsafe in a hot loop "for speed" without profiling | Profile first; most "optimisations" are no-ops |

---

## 13. Cross-References

- [borrow-checker.md](borrow-checker.md) -- why unsafe exists and what it bypasses
- [async-rust.md](async-rust.md) -- Pin and self-referential state
- [traits-design.md](traits-design.md) -- unsafe traits
- [full-guide.md](full-guide.md) -- when stdlib already has what you need
