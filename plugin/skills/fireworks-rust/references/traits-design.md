# fireworks-rust -- Traits Design

> Object-safe traits, dyn vs impl, marker traits, blanket impls.
> The Rust approach to abstraction.

---

## 1. Traits: The Core Concept

A trait is a set of method signatures that a type can implement. Traits are Rust's only form of polymorphism.

```rust
pub trait Summarizable {
    fn summary(&self) -> String;

    // Default implementations
    fn short_summary(&self) -> String {
        let s = self.summary();
        if s.len() > 80 { format!("{}...", &s[..77]) } else { s }
    }
}

pub struct Article {
    pub title: String,
    pub body: String,
}

impl Summarizable for Article {
    fn summary(&self) -> String {
        format!("{}: {}", self.title, &self.body[..100.min(self.body.len())])
    }
}
```

### Required vs Provided Methods

- Required: declared with `fn sig;` and no body. Every impl must provide one.
- Provided: have a default body. Impls may override.

Use provided methods to share implementation while keeping the interface small.

---

## 2. Static Dispatch (impl Trait)

```rust
fn process(item: impl Summarizable) {
    println!("{}", item.summary());
}

// Each concrete type generates its own compiled version
process(article);   // specialised for Article
process(tweet);     // specialised for Tweet
```

Zero runtime cost, larger binary. Use when:

- Called many times in a hot loop.
- Implementation is small.
- Caller knows the concrete type at compile time.

### Returning impl Trait

```rust
fn make_handler() -> impl Fn(i32) -> i32 {
    let x = 10;
    move |y| x + y
}
```

Compiler infers the concrete return type. Caller can't name it but can call methods on it. Useful for iterator and closure returns.

### Generic Bound Syntax

```rust
fn process<T: Summarizable>(item: T) {
    println!("{}", item.summary());
}

// Equivalent to:
fn process<T>(item: T)
where
    T: Summarizable,
{
    println!("{}", item.summary());
}

// Equivalent to:
fn process(item: impl Summarizable) {
    println!("{}", item.summary());
}
```

All three generate the same code. The `where` clause is cleanest for multiple bounds; `impl Trait` is cleanest for simple cases.

---

## 3. Dynamic Dispatch (dyn Trait)

```rust
fn process(item: &dyn Summarizable) {
    println!("{}", item.summary());
}

let items: Vec<Box<dyn Summarizable>> = vec![
    Box::new(article),
    Box::new(tweet),
];

for item in &items {
    process(item.as_ref());
}
```

Single compiled function; method calls go through a vtable. Use when:

- Heterogeneous collection (`Vec<Box<dyn T>>`).
- Return value depends on runtime condition.
- You specifically want smaller binary size.

### &dyn vs Box<dyn> vs Arc<dyn>

```rust
fn takes_ref(x: &dyn Summarizable) { ... }         // borrowed
fn takes_box(x: Box<dyn Summarizable>) { ... }     // owned
fn takes_arc(x: Arc<dyn Summarizable + Send + Sync>) { ... }  // shared across threads
```

Choice:

- `&dyn` -- when caller owns, just wants to borrow.
- `Box<dyn>` -- single owner, different types based on runtime.
- `Arc<dyn>` -- multiple owners, possibly across threads.

---

## 4. Object Safety

Not every trait can be used as `dyn Trait`. Object-safe rules (simplified):

A trait is object-safe if:

- None of its methods return `Self` by value (except via `Box<Self>`).
- None of its methods are generic (generic parameters on methods, not on the trait).
- No method has `where Self: Sized` (unless marked so explicitly).

### Object-Safe

```rust
pub trait Logger {
    fn log(&self, msg: &str);
    fn level(&self) -> Level;
}
// OK -- no Self returns, no generic methods
```

### NOT Object-Safe

```rust
pub trait Cloneable {
    fn clone_it(&self) -> Self;
}
// ERROR: dyn Cloneable can't return unknown-size Self

pub trait Converter {
    fn convert<T>(&self, t: T) -> T;
}
// ERROR: generic method
```

### Escape Hatch: where Self: Sized

```rust
pub trait Logger {
    fn log(&self, msg: &str);
    fn log_debug<T: std::fmt::Debug>(&self, value: &T) where Self: Sized {
        self.log(&format!("{:?}", value));
    }
}
```

The `where Self: Sized` on `log_debug` excludes it from the `dyn` vtable. You can still call it on concrete types; you just can't call it through `&dyn Logger`.

---

## 5. Associated Types

```rust
pub trait Parser {
    type Output;
    type Error;
    fn parse(&self, s: &str) -> Result<Self::Output, Self::Error>;
}

struct JsonParser;
impl Parser for JsonParser {
    type Output = serde_json::Value;
    type Error = serde_json::Error;
    fn parse(&self, s: &str) -> Result<Self::Output, Self::Error> {
        serde_json::from_str(s)
    }
}

// Callers refer via <Parser as ParserT>::Output
fn use_parser<P: Parser>(p: &P, s: &str) -> Result<P::Output, P::Error> {
    p.parse(s)
}
```

Associated types vs generic parameters: associated types fix one type per impl; generic parameters allow multiple impls for the same type with different parameters.

```rust
// Associated: Parser can only have ONE impl per type
pub trait Parser { type Output; ... }

// Generic: Convert can have MANY impls per type
pub trait Convert<T> {
    fn convert(&self) -> T;
}

// Can impl multiple
impl Convert<String> for MyType { ... }
impl Convert<Vec<u8>> for MyType { ... }
```

---

## 6. Trait Bounds and Where Clauses

```rust
fn sort_and_display<T>(mut items: Vec<T>) -> String
where
    T: std::cmp::Ord + std::fmt::Display,
{
    items.sort();
    items.iter().map(|x| format!("{}", x)).collect::<Vec<_>>().join(", ")
}
```

### Higher-Ranked Trait Bounds (HRTB)

```rust
fn call_with_str<F>(f: F) -> i32
where
    F: for<'a> Fn(&'a str) -> i32,
{
    f("hello") + f("world")
}
```

`for<'a>` means "for any lifetime 'a". Needed when the closure must accept references of any lifetime.

### Conditional Impls

```rust
impl<T: Clone> Cache<T> {
    fn snapshot(&self) -> Vec<T> {
        self.items.iter().cloned().collect()
    }
}

// Also works for multiple conditions
impl<T: Serialize + DeserializeOwned> Persistent for Cache<T> {
    fn save(&self) { ... }
    fn load() -> Self { ... }
}
```

Only types that match the constraints get the impl. Enables different APIs for different type properties.

---

## 7. Blanket Impls

Impl for "every type satisfying X".

```rust
pub trait MyDebug {
    fn my_debug(&self) -> String;
}

impl<T: std::fmt::Debug> MyDebug for T {
    fn my_debug(&self) -> String {
        format!("{:?}", self)
    }
}
```

Now every type that implements `Debug` automatically has a `my_debug` method. Standard library uses this extensively:

- `impl<T: Display> ToString for T` -- every Displayable type gets `.to_string()`.
- `impl<T> Borrow<T> for T` -- every T can be borrowed as &T.
- `impl<T: Clone> Clone for Box<T>` -- boxed clones if inner clones.

### Orphan Rule

You can only implement a trait for a type if you own one of them. Prevents conflicts across crates.

```rust
// Owned crate. Either the trait or the type must be yours.
impl Display for MyType { ... }        // OK -- MyType is mine
impl MyTrait for i32 { ... }           // OK -- MyTrait is mine
impl Display for i32 { ... }           // ERROR -- neither mine
```

Workaround: newtype pattern.

```rust
struct IntWrapper(i32);

impl Display for IntWrapper {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "wrapped: {}", self.0)
    }
}
```

---

## 8. Marker Traits

Traits with no methods; they communicate a property.

```rust
// Standard marker traits:
// Send -- safe to send to another thread
// Sync -- safe to share between threads (via &T)
// Copy -- type is bit-copy safe
// Sized -- compile-time known size
// Unpin -- can be moved after being pinned
```

Most are auto-derived by the compiler based on structure. You can opt out with a `PhantomData<NotSomething>` or by holding a type that isn't.

### Custom Marker Traits

```rust
pub unsafe trait SafeForConcurrency: Send + Sync {}

unsafe impl SafeForConcurrency for MyType {}
```

The `unsafe trait` declaration signals "implementing this is a promise". The implementer asserts the invariant; callers can rely on it.

---

## 9. From / Into / TryFrom / TryInto

These traits define conversions between types. They're the Rust idiom for "accept anything that can become this".

```rust
impl From<&str> for OrderID {
    fn from(s: &str) -> Self {
        OrderID(s.to_string())
    }
}

// Automatic: From<T> for U gives Into<U> for T
let id: OrderID = "abc".into();

// Preferred: accept Into<T> instead of T
fn process(id: impl Into<OrderID>) {
    let id = id.into();
    ...
}

// Caller can pass &str, String, or OrderID -- all convert
process("abc");
process(String::from("abc"));
process(OrderID("abc".to_string()));
```

### TryFrom for Fallible Conversion

```rust
#[derive(Debug)]
struct InvalidTier(i64);
impl std::error::Error for InvalidTier { }
impl std::fmt::Display for InvalidTier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid tier: {}", self.0)
    }
}

enum Tier { Free, Pro, Enterprise }

impl TryFrom<i64> for Tier {
    type Error = InvalidTier;
    fn try_from(n: i64) -> Result<Self, InvalidTier> {
        match n {
            0 => Ok(Tier::Free),
            1 => Ok(Tier::Pro),
            2 => Ok(Tier::Enterprise),
            other => Err(InvalidTier(other)),
        }
    }
}
```

### Boundary Pattern

```rust
// Accept broadly, store narrowly
struct Order {
    id: OrderID,
    total_cents: u64,
}

impl Order {
    fn new(id: impl Into<OrderID>, total_cents: u64) -> Self {
        Order { id: id.into(), total_cents }
    }
}

// Caller's convenience
let o = Order::new("ord_abc", 1000);
```

---

## 10. Deref and AsRef

`Deref` lets a wrapper type act like its inner.

```rust
struct SmartBox<T>(T);

impl<T> std::ops::Deref for SmartBox<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}

let b = SmartBox(String::from("hello"));
println!("{}", b.len());   // calls String::len via deref
```

This is why `&String` can be used where `&str` is expected: `String` has `Deref<Target = str>`.

### AsRef

```rust
fn read_file(path: impl AsRef<Path>) -> io::Result<String> {
    std::fs::read_to_string(path.as_ref())
}

read_file("hello.txt");            // &str -> AsRef<Path>
read_file(PathBuf::from("hello")); // PathBuf -> AsRef<Path>
read_file(&my_path);
```

Implement `AsRef<T>` to signal "I can cheaply borrow as &T". Less flexible than Deref (no auto-conversions) but more explicit.

### When to Use Which

- `Deref` -- the wrapper IS a kind of the inner type. String is "a kind of" str.
- `AsRef` -- the wrapper can be viewed as T. PathBuf can be viewed as Path.
- `From`/`Into` -- the wrapper can be constructed from or converted to.
- `Borrow` -- the wrapper can be used in place of T for hashing/comparison.

Use the most specific one that fits. Each enables different call-site patterns.

---

## 11. Extension Trait Pattern

Add methods to a type you don't own.

```rust
pub trait StringExt {
    fn reverse(&self) -> String;
}

impl StringExt for String {
    fn reverse(&self) -> String {
        self.chars().rev().collect()
    }
}

// Now anywhere StringExt is in scope:
let reversed = "hello".to_string().reverse();
```

Library authors use this to extend third-party types while respecting the orphan rule.

---

## 12. Type State Pattern

Use traits to encode state at the type level.

```rust
pub struct Connection<State> {
    fd: i32,
    _state: std::marker::PhantomData<State>,
}

pub struct Disconnected;
pub struct Connected;

impl Connection<Disconnected> {
    pub fn new() -> Self { ... }
    pub fn connect(self) -> Result<Connection<Connected>, Error> { ... }
}

impl Connection<Connected> {
    pub fn send(&self, data: &[u8]) -> Result<(), Error> { ... }
    pub fn disconnect(self) -> Connection<Disconnected> { ... }
}

// Can't call send() on a Disconnected connection -- compile-time error
```

Type state encodes protocols in the type system. Compile-time enforcement of "first connect, then send".

---

## 13. Builder Pattern

For structs with many optional fields.

```rust
pub struct RequestBuilder {
    url: String,
    method: Method,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
    timeout: Option<Duration>,
}

impl RequestBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        RequestBuilder {
            url: url.into(),
            method: Method::Get,
            headers: HashMap::new(),
            body: None,
            timeout: None,
        }
    }

    pub fn method(mut self, m: Method) -> Self {
        self.method = m;
        self
    }

    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.insert(k.into(), v.into());
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    pub fn build(self) -> Request {
        Request {
            url: self.url,
            method: self.method,
            headers: self.headers,
            body: self.body,
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
        }
    }
}

// Usage
let req = RequestBuilder::new("https://example.com")
    .method(Method::Post)
    .header("Content-Type", "application/json")
    .timeout(Duration::from_secs(5))
    .build();
```

`derive_builder` is a crate that generates this boilerplate.

---

## 14. Standard Library Traits You Should Know

| Trait | Purpose | When to Impl |
|-------|---------|--------------|
| Display | User-facing formatting (`{}`) | User-visible strings |
| Debug | Developer formatting (`{:?}`) | Always derive unless sensitive |
| Clone | Duplicate value | When callers need copies |
| Copy | Bitwise duplicate | Small POD types only |
| Default | Default construction | Structs with sensible defaults |
| PartialEq / Eq | Equality comparison | Types you want to compare |
| PartialOrd / Ord | Ordering comparison | Types you want to sort |
| Hash | Hash for HashMap keys | Types used as keys |
| Iterator | Iterator protocol | Custom iterators |
| IntoIterator | Convert to iterator | Collection types |
| FromStr | Parse from &str | Types with string repr |
| AsRef<T> | Borrow as T | Wrapper types |
| Deref | Act like inner type | Smart pointers |
| Drop | Custom cleanup | RAII resources |
| From / Into | Infallible conversion | Type boundaries |
| TryFrom / TryInto | Fallible conversion | Narrowing conversions |

---

## 15. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `impl<T> MyTrait for T` for any T | Add bounds; otherwise conflicts |
| Big traits with 10+ methods | Split by role (OrderSaver + OrderLoader) |
| `dyn Trait` in a hot loop | Try impl Trait; measure if you really need dyn |
| Associated types when you need multiple | Use generic parameter |
| Generic parameter when you have one impl | Use associated type for clarity |
| Manual impl of Debug | Derive; only override for sensitive fields |
| Trait returning Self by value | Return Box<Self> or split trait |
| Requiring Send+Sync+'static on every generic | Only add what you actually need |
| impl<T> Clone for Box<T> where T: Clone (stdlib has it) | Rely on stdlib blanket impls |
| Complex lifetime parameters on traits | Consider owned types or Arc |

---

## 16. Cross-References

- [borrow-checker.md](borrow-checker.md) -- lifetimes in trait objects
- [async-rust.md](async-rust.md) -- async fn in traits
- [errors.md](errors.md) -- Error trait, From impl for error conversion
- [full-guide.md](full-guide.md) -- common standard library traits
