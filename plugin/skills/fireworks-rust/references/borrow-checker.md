# fireworks-rust -- Borrow Checker

> Ownership, borrowing, lifetimes, NLL, PhantomData.
> How the compiler reasons about memory and how you cooperate with it.

---

## 1. The Three Rules

1. Every value has exactly one owner at any moment.
2. When the owner goes out of scope, the value is dropped (its `Drop` impl runs).
3. You can have either one exclusive reference (`&mut T`) or any number of shared references (`&T`) -- but not both at the same time.

Everything else in this document is a consequence of these three rules.

---

## 2. Ownership

### Value Types

```rust
let s1 = String::from("hello");
let s2 = s1;             // ownership moves to s2
// println!("{}", s1);    // ERROR: borrow of moved value
```

After `let s2 = s1`, the value that was at `s1` is at `s2`. `s1` is no longer valid -- the compiler statically prevents any use. This is called a "move".

### Copy Types

```rust
let x: i32 = 5;
let y = x;
println!("{} {}", x, y);  // OK -- i32 is Copy
```

Types that are cheap to duplicate implement `Copy`: integers, floats, bools, chars, and tuples/arrays of Copy types. Assignment copies the bits instead of moving.

You can derive `Copy` for small plain-data structs:

```rust
#[derive(Copy, Clone)]
struct Point { x: f64, y: f64 }
```

Copy requires Clone. The rule of thumb: Copy for stack-only plain data; Clone for heap-allocated or non-trivial types.

### Drop

When an owned value goes out of scope, its `Drop::drop` runs:

```rust
struct FileHandle { fd: i32 }

impl Drop for FileHandle {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

fn main() {
    let f = FileHandle { fd: 3 };
    // f.drop() called here automatically at end of scope
}
```

This is RAII: acquire resources in the constructor, release in `Drop`. Consistent pattern across file handles, mutex guards, sockets, database connections.

---

## 3. Borrowing

### Shared References `&T`

```rust
fn print(s: &String) {
    println!("{}", s);
}

let s = String::from("hello");
print(&s);
print(&s);      // OK -- multiple shared borrows
println!("{}", s);  // OK -- owner still valid after borrow
```

Multiple shared references can coexist. The owner cannot mutate while any shared reference exists.

### Exclusive References `&mut T`

```rust
fn append_world(s: &mut String) {
    s.push_str(", world!");
}

let mut s = String::from("hello");
append_world(&mut s);
println!("{}", s);  // "hello, world!"
```

Only one `&mut T` at a time. No `&T` coexisting. This prevents data races at compile time.

### Non-Lexical Lifetimes (NLL)

References are valid only until their last use, not until the end of their scope.

```rust
let mut v = vec![1, 2, 3];
let r = &v;
println!("{}", r[0]);       // last use of r
v.push(4);                   // OK -- r is dead
```

Before NLL (Rust 2018), this wouldn't compile. Now the borrow checker tracks "where was this reference last used" and ends the borrow there.

### Reborrowing

```rust
fn takes_mut(x: &mut i32) {
    let y: &mut i32 = &mut *x;  // reborrow
    *y = 42;
    // y's borrow ends here
    *x = 43;  // now we can use x again
}
```

Reborrowing is how functions pass mutable references through. The reborrowed reference has a shorter lifetime than the original, allowing the original to be reused afterward.

---

## 4. Lifetimes

A lifetime is a named region of the program where a reference is valid. Most of the time, the compiler infers them. Sometimes you need to write them.

### When You Need Explicit Lifetimes

When a function returns a reference, the compiler needs to know which input it's derived from.

```rust
// Fails -- ambiguous which input the output references
fn longer(a: &str, b: &str) -> &str { ... }

// OK -- explicit annotation
fn longer<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}
```

The lifetime `'a` says: "the output lives at least as long as both inputs".

### Lifetime Elision Rules

The compiler elides lifetimes in common cases:

1. Each input reference gets its own lifetime parameter.
2. If there's exactly one input lifetime, it's assigned to all output references.
3. If there are multiple input lifetimes but one is `&self`, the `self` lifetime is assigned to output references.

```rust
fn first_word(s: &str) -> &str { ... }           // rule 2
fn method(&self, s: &str) -> &str { ... }        // rule 3

// Not covered by rules -- must be explicit
fn combine<'a>(a: &'a str, b: &'a str) -> &'a str { ... }
```

### Structs with References

```rust
struct Parser<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Parser { source, position: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.position..]
    }
}
```

The lifetime parameter `'a` says the `Parser` borrows from something that lives at least as long as `'a`. The struct cannot outlive its source.

### 'static Lifetime

```rust
let s: &'static str = "hello";          // string literal
let s: &'static str = Box::leak(Box::new(String::from("hello")));  // intentional leak
```

`'static` means the reference lives for the entire program. String literals are `'static` (they're in the binary). Use with care -- `Box::leak` permanently leaks memory.

### When to Refactor Instead of Annotate

If you find yourself writing multiple lifetime parameters on a struct, step back. Often the cleaner fix is:

- Take `String` (owned) instead of `&str` (borrowed).
- Return `Vec<T>` (owned) instead of `&[T]` (borrowed).
- Use `Arc<T>` for shared ownership instead of lifetime gymnastics.

Lifetimes exist to avoid unnecessary cloning. If cloning is cheap in your case, clone and move on.

---

## 5. Common Borrow Checker Errors

### Cannot Borrow as Mutable More Than Once

```rust
let mut v = vec![1, 2, 3];
let a = &mut v;
let b = &mut v;  // ERROR
a.push(4);
```

Fix: sequence the borrows.

```rust
let mut v = vec![1, 2, 3];
{
    let a = &mut v;
    a.push(4);
}
{
    let b = &mut v;
    b.push(5);
}
```

Or with NLL just ensure `a` isn't used after `b` is taken:

```rust
let mut v = vec![1, 2, 3];
let a = &mut v;
a.push(4);        // last use of a
let b = &mut v;
b.push(5);
```

### Cannot Borrow as Mutable Because It Is Also Borrowed as Immutable

```rust
let mut v = vec![1, 2, 3];
let r = &v;
v.push(4);        // ERROR -- r is still alive
println!("{}", r[0]);
```

Fix: reorder so the shared borrow ends first.

```rust
let mut v = vec![1, 2, 3];
let r = &v;
println!("{}", r[0]);   // last use of r
v.push(4);               // OK
```

### Borrowed Value Does Not Live Long Enough

```rust
fn get_first<'a>(v: &'a Vec<String>) -> &'a String {
    &v[0]
}

fn main() {
    let first = {
        let v = vec![String::from("hello")];
        get_first(&v)        // ERROR -- v dies here, but first still references it
    };
    println!("{}", first);
}
```

Fix: keep the owner alive long enough, or return an owned value.

```rust
fn main() {
    let v = vec![String::from("hello")];
    let first = get_first(&v);
    println!("{}", first);
    // v lives until end of main, longer than first
}
```

### Cannot Move Out of Borrowed Content

```rust
fn first(v: &Vec<String>) -> String {
    v[0]       // ERROR -- can't move out of &Vec
}
```

Fix: clone or borrow.

```rust
fn first(v: &Vec<String>) -> String {
    v[0].clone()
}

fn first(v: &Vec<String>) -> &String {
    &v[0]
}
```

---

## 6. Interior Mutability

Interior mutability lets you mutate through a `&T` by checking the borrow rules at runtime instead of compile time.

### Cell<T>

For Copy types only. Get/set without borrowing.

```rust
use std::cell::Cell;

struct Counter {
    count: Cell<u32>,  // mutable through &Counter
}

impl Counter {
    fn increment(&self) {
        self.count.set(self.count.get() + 1);
    }
}
```

### RefCell<T>

For non-Copy types. Runtime-checked borrows.

```rust
use std::cell::RefCell;

let r = RefCell::new(vec![1, 2, 3]);

{
    let read = r.borrow();        // like &
    println!("{:?}", read);
}

{
    let mut write = r.borrow_mut(); // like &mut
    write.push(4);
}

// Violating rules panics at runtime
let a = r.borrow();
let b = r.borrow_mut();  // PANIC: already borrowed
```

Use `RefCell` sparingly. If you reach for it often, your design probably wants `&mut` or restructuring instead.

### OnceCell / OnceLock

Lazy initialisation, write-once.

```rust
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

fn config() -> &'static Config {
    CONFIG.get_or_init(|| load_config())
}
```

Thread-safe and no initialisation race. Replaces the old `lazy_static!` macro for most uses.

---

## 7. Smart Pointers

### Box<T>

Owned heap allocation. Single owner.

```rust
let b: Box<i32> = Box::new(5);
let bigger: Box<[i32]> = vec![1, 2, 3, 4].into_boxed_slice();
```

Uses:

- Moving large values that would be expensive to copy.
- Enabling recursive types (an `enum` that references itself must box the reference).
- Trait objects: `Box<dyn Trait>`.

### Rc<T>

Reference-counted, single-threaded.

```rust
use std::rc::Rc;

let a = Rc::new(vec![1, 2, 3]);
let b = Rc::clone(&a);   // cheap -- just bumps refcount
let c = Rc::clone(&a);

println!("count = {}", Rc::strong_count(&a));  // 3
```

`Rc` is NOT `Send` or `Sync`. Same-thread only.

### Arc<T>

Atomic reference-counted, multi-threaded.

```rust
use std::sync::Arc;
use std::thread;

let a = Arc::new(vec![1, 2, 3]);
let b = Arc::clone(&a);

thread::spawn(move || {
    println!("{:?}", b);
});
```

Slightly more expensive than `Rc` due to atomic operations, but required for cross-thread sharing.

### Weak<T>

Non-owning reference. Breaks cycles.

```rust
use std::rc::{Rc, Weak};

struct Node {
    parent: Option<Weak<Node>>,  // parent held weakly
    children: Vec<Rc<Node>>,     // children held strongly
}
```

Access a `Weak`'s value via `.upgrade()`, which returns `Option<Rc<T>>`. Returns `None` if the strong references are all gone.

---

## 8. PhantomData

For generic types that logically own a `T` but don't physically contain one.

```rust
use std::marker::PhantomData;

struct Id<T> {
    value: u64,
    _phantom: PhantomData<T>,
}

// UserID and OrderID are different types at compile time,
// even though they have the same runtime layout.
type UserID = Id<User>;
type OrderID = Id<Order>;
```

`PhantomData<T>` affects:

- Drop check: tells the compiler this struct logically owns a T (so T must outlive it).
- Variance: whether Id<Animal> can be used where Id<Dog> is expected.
- Auto traits: whether the struct is Send/Sync/Unpin.

Advanced. Use when you need generic type tracking without actual storage.

---

## 9. Borrow Checker vs Self-Referential Structs

Rust natively forbids structs that reference their own fields:

```rust
// This cannot be written safely
struct SelfRef {
    name: String,
    name_ref: &?,  // should reference self.name, but what lifetime?
}
```

Options:

1. Restructure -- does this really need to be self-referential?
2. Use indices instead of references (`usize` into a `Vec`).
3. Use `ouroboros` or `self_cell` crates for genuinely self-referential types.
4. For async state machines, Pin solves this (see async-rust.md).

Rule of thumb: if you're reaching for self-referential structs, try indices first. 90% of the time it works and is simpler.

---

## 10. Common Patterns to Avoid the Borrow Checker Fights

### Clone for Simple Cases

```rust
// Instead of wrangling lifetimes
fn process(items: &[String]) -> Vec<String> {
    items.iter().cloned().collect()
}
```

If cloning is cheap (short strings, small structs), clone. Don't waste time on lifetime puzzles to save a few hundred nanoseconds.

### Index Instead of Reference

```rust
struct Tree {
    nodes: Vec<Node>,
}

struct Node {
    parent: Option<usize>,   // index, not reference
    children: Vec<usize>,
}
```

Indices have no lifetimes. The trade-off is you lose some type safety (an index into the wrong Vec is a bug) but you avoid lifetime gymnastics entirely.

### Take & Replace for Mutation

```rust
fn transform(s: &mut State) {
    let old = std::mem::take(&mut s.value);   // leaves Default::default() in place
    let new = expensive_compute(old);
    s.value = new;
}
```

`std::mem::take` and `std::mem::replace` let you move out of a `&mut T` by leaving a default in its place.

### Split Borrow

```rust
struct Pair {
    a: Vec<i32>,
    b: Vec<i32>,
}

impl Pair {
    fn work(&mut self) {
        let a = &mut self.a;
        let b = &mut self.b;   // OK -- disjoint fields
        a.push(1);
        b.push(2);
    }
}
```

The compiler understands disjoint field borrows. You can borrow different fields mutably at the same time.

---

## 11. Variance (Advanced)

Variance describes whether a generic type's subtyping follows its parameter's subtyping.

```rust
// &'a T is covariant in 'a: &'static T can be used where &'short T is expected
// &'a mut T is invariant in T: you cannot substitute

fn main() {
    let s: &'static str = "hello";
    let shorter: &str = s;  // OK -- covariance on 'a
}
```

You rarely write variance directly. It comes up when you build libraries or use `PhantomData`. The Rustonomicon has a full chapter if you need it.

---

## 12. Cross-References

- [async-rust.md](async-rust.md) -- Pin and self-referential async state
- [traits-design.md](traits-design.md) -- lifetimes with trait objects
- [errors.md](errors.md) -- borrow-checker-friendly error propagation
- [full-guide.md](full-guide.md) -- practical patterns that avoid fights
