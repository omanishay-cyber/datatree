# Logic Review Reference — Fireworks Review

Detailed checklist for the **Logic** lens. Use this reference when reviewing code for correctness, edge cases, data integrity, and error handling.

---

## Bug Patterns

### Off-by-One Errors
- **Array bounds**: accessing `arr[arr.length]` instead of `arr[arr.length - 1]`
- **Loop conditions**: `for (let i = 0; i <= arr.length; i++)` — iterates one too many
- **Slice/substring**: `str.slice(0, length - 1)` — off by one when extracting substrings
- **Pagination**: page 1 offset is `0`, not `1` — `offset = (page - 1) * pageSize`
- **Date arithmetic**: months are 0-indexed (`new Date(2024, 0, 1)` is January, not February)
- **Fencepost errors**: counting intervals vs. counting items (10 fenceposts = 9 gaps)

### Null / Undefined Access
- **Optional chaining missing**: `user.address.city` when `address` might be undefined
- **Array destructuring**: `const [first] = []` gives `undefined`, not an error
- **Object property access**: `obj[key]` where `key` might not exist on `obj`
- **Function return values**: function returns `undefined` on some paths but caller assumes a value
- **DOM queries**: `document.getElementById('x')` returns `null` if element does not exist
- **Map/WeakMap get**: `.get(key)` returns `undefined` when key is absent

### Type Coercion Gotchas
- **Loose equality**: `0 == ''` is `true`, `null == undefined` is `true`, `[] == false` is `true`
- **String concatenation**: `'5' + 3` gives `'53'`, but `'5' - 3` gives `2`
- **Boolean coercion**: `Boolean('')` is `false`, `Boolean('false')` is `true`
- **parseInt pitfalls**: `parseInt('08')` is `8` in modern JS but was `0` in old engines (octal)
- **Number()**: `Number('')` is `0`, `Number(null)` is `0`, `Number(undefined)` is `NaN`
- **JSON.parse**: will throw on invalid JSON — always wrap in try/catch

### Falsy Value Confusion
- Six falsy values: `0`, `''` (empty string), `false`, `null`, `undefined`, `NaN`
- **Common mistake**: `if (!value)` when `value` could legitimately be `0` or `''`
- **Fix**: Use explicit checks — `if (value === null || value === undefined)` or `value == null`
- **Double negation**: `!!value` loses distinction between `0`, `''`, `false`, `null`, `undefined`
- **Default values**: `value || defaultValue` replaces `0` and `''` — use `value ?? defaultValue` instead (nullish coalescing)

---

## Edge Cases Checklist

### Collection Edge Cases
- **Empty array / object**: Does the code handle `[]` or `{}`? Does it crash on `.map()` of empty?
- **Single element**: `arr.reduce()` with no initial value and single element — returns element without calling reducer
- **Sparse arrays**: `Array(5)` creates holes — `.map()` skips holes, `.forEach()` skips holes
- **Duplicate keys**: What happens with duplicate entries in a Set or Map?
- **Large collections**: Does performance degrade with 10k+ items? Infinite scroll? Pagination?

### Numeric Edge Cases
- **Maximum values**: `Number.MAX_SAFE_INTEGER` (2^53 - 1) — beyond this, integer math is unreliable
- **Negative numbers**: Does sorting handle negatives? Does `Math.abs()` handle `-0`?
- **Zero**: Division by zero gives `Infinity`, not an error. `0 === -0` is `true`.
- **NaN**: `NaN !== NaN` is `true`. Use `Number.isNaN()` not global `isNaN()`.
- **Floating point**: `0.1 + 0.2 !== 0.3` — use epsilon comparison or integer math for money
- **Currency**: Never use float for money. Use integer cents or a decimal library.

### String Edge Cases
- **Empty string**: Is `''` handled differently from `null` or `undefined`?
- **Unicode**: Multi-byte characters (`'cafe\u0301'.length` is 5, not 4). Emoji length varies.
- **Whitespace-only**: `'   '.trim()` is `''` — is this treated as empty?
- **Very long strings**: What happens with a 10MB string input?
- **Special characters**: SQL quotes, HTML entities, regex metacharacters, path separators

### Optional Parameters
- **undefined vs. not passed**: `function f(x?) {}` — `x` is `undefined` whether omitted or passed as `undefined`
- **Default parameter evaluation**: Defaults are evaluated at call time, not definition time
- **Destructured defaults**: `const { a = 1 } = obj` — default applies only when `a` is `undefined`, not `null`
- **Rest parameters**: `...args` is always an array, even if empty

### Concurrency Edge Cases
- **Concurrent access**: Two tabs modifying the same database record
- **Rapid clicks**: Button clicked twice before first request completes
- **Stale data**: Reading a value that was updated by another process since last fetch
- **Race between navigation**: User navigates away while async operation is in progress

---

## Control Flow Issues

### Unreachable Code
- Code after `return`, `throw`, `break`, or `continue` in the same block
- `if (true)` branches that make the `else` dead code
- Switch cases that always fall through to default
- Functions that always throw — code after the call is unreachable

### Missing Break in Switch
- Intentional fallthrough should have a `// falls through` comment
- Missing break causes execution of subsequent cases
- Prefer returning from switch cases in functions over break statements
- Consider object lookup or Map over switch when mapping values

### Early Return Skipping Cleanup
- **Pattern**: `if (error) return;` before closing a file handle or database connection
- **Fix**: Use `try/finally` or RAII-style patterns (cleanup registered before potential exit)
- **React**: Returning early before `useEffect` cleanup is registered (hook order violation)
- **Timers**: Returning before `clearInterval` / `clearTimeout` is called

### Exception Swallowing
- **Empty catch blocks**: `catch (e) {}` — silently discards the error
- **Generic catch-all**: `catch (e) { console.log('error') }` — loses error context
- **Missing re-throw**: Catching an error you cannot handle without re-throwing
- **Logging without handling**: `catch (e) { console.error(e) }` — caller thinks operation succeeded
- **Fix**: Either handle the error meaningfully, re-throw, or use error boundaries

---

## Async Issues

### Missing Await
- Calling an async function without `await` — returns a Promise, not the value
- **Subtle**: `array.forEach(async (item) => { await process(item) })` — forEach does NOT await
- **Fix for forEach**: Use `for...of` loop or `Promise.all(array.map(...))`
- **Conditional await**: `if (condition) await doThing()` — missing braces can cause `await` to apply to wrong expression

### Unhandled Promise Rejection
- Async function called without `.catch()` or try/catch around `await`
- Promise chain without terminal `.catch()`
- `Promise.all()` rejects on first failure — other promises continue running
- **Fix**: Always have error handling at the top-level call site

### Parallel vs. Sequential Async
- **Sequential** (slow): `await a(); await b(); await c();` — each waits for the previous
- **Parallel** (fast): `await Promise.all([a(), b(), c()])` — all run concurrently
- **When to use sequential**: When operations depend on each other or share resources
- **When to use parallel**: When operations are independent
- **Promise.allSettled()**: When you want all results regardless of individual failures

### Race Conditions in State Updates
- **React**: Setting state based on current state without using the updater function
  - Wrong: `setState(count + 1)` — uses stale `count` if called multiple times
  - Right: `setState(prev => prev + 1)` — always uses latest state
- **Stale closures**: `useEffect` capturing a value that changes before the effect runs
- **Concurrent writes**: Two processes writing to the same database row
- **Event ordering**: Assuming events fire in a specific order without guarantees

---

## Data Integrity

### Mutation of Shared State
- Modifying an object/array that is referenced elsewhere
- **Redux/Zustand**: Mutating state directly instead of creating a new reference
- **Props mutation**: Modifying a prop object in a child component
- **Array methods**: `.sort()`, `.reverse()`, `.splice()` mutate in place — use `.toSorted()`, `.toReversed()`, `.toSpliced()` or spread first
- **Object spread**: `{...obj}` is shallow — nested objects are still references

### Shallow vs. Deep Copy
- **Spread operator**: `{...obj}` and `[...arr]` are shallow copies
- **JSON round-trip**: `JSON.parse(JSON.stringify(obj))` — deep but loses Date, undefined, functions, BigInt, circular refs
- **structuredClone()**: Native deep clone, handles most types but not functions
- **When shallow is fine**: Immutable nested data, primitive-only objects
- **When deep is needed**: Nested objects that will be modified independently

### Reference vs. Value Comparison
- **Objects/Arrays**: `{a: 1} === {a: 1}` is `false` — reference comparison
- **Strings/Numbers**: `'abc' === 'abc'` is `true` — value comparison
- **React renders**: Component re-renders when prop reference changes, even if values are identical
- **Set/Map keys**: Objects as keys use reference identity, not structural equality
- **Fix**: Use `JSON.stringify()` for structural comparison, or deep-equal libraries

### Serialization Round-Trip Loss
- **Date**: `JSON.stringify(new Date())` gives a string — `JSON.parse()` does NOT restore it to a Date
- **BigInt**: `JSON.stringify(BigInt(9007199254740993))` throws — BigInt is not serializable
- **undefined**: `JSON.stringify({a: undefined})` gives `{}` — property is lost
- **Functions**: Lost in JSON serialization entirely
- **Buffer/Uint8Array**: Serialized as object with numeric keys, not as binary data
- **NaN/Infinity**: Serialized as `null` in JSON
- **Circular references**: `JSON.stringify` throws on circular structures

---

## Error Handling Patterns

### Catch Blocks That Swallow Errors
```typescript
// BAD: Error is completely lost
try { await saveData(); } catch (e) {}

// BAD: Error logged but caller thinks success
try { await saveData(); } catch (e) { console.error(e); }

// GOOD: Error is handled or re-thrown
try { await saveData(); } catch (e) {
  if (e instanceof NetworkError) {
    showRetryDialog();
  } else {
    throw e; // Re-throw unexpected errors
  }
}
```

### Generic Error Messages
- `"Something went wrong"` tells the user nothing
- Include context: what operation failed, what the user can do about it
- Log the technical error for debugging, show user-friendly message for display
- Different error types deserve different messages (network vs validation vs auth)

### Missing Finally Cleanup
```typescript
// BAD: Connection leaks if processData throws
const conn = await getConnection();
const data = await processData(conn);
conn.release();

// GOOD: Always release
const conn = await getConnection();
try {
  return await processData(conn);
} finally {
  conn.release();
}
```

### Error Type Narrowing
```typescript
// BAD: Assumes all errors are Error instances
catch (e) { console.error(e.message); }

// GOOD: Narrow the type
catch (e) {
  if (e instanceof Error) {
    console.error(e.message);
  } else {
    console.error('Unknown error:', String(e));
  }
}
```
- In TypeScript, `catch (e)` gives `e: unknown` in strict mode
- Always narrow before accessing properties
- Third-party libraries may throw non-Error values (strings, objects, numbers)
