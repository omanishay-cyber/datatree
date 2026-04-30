# fireworks-test — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 3. Edge Case Test Matrix

Every function needs at least 3 test cases: happy path, edge case, error case. For critical business logic, use the full matrix:

| # | Category | Examples | Why It Matters |
|---|----------|---------|----------------|
| 1 | **Happy Path** | Normal, expected inputs; typical user scenario | Basic correctness — if this fails, nothing works |
| 2 | **Boundary Values** | 0, 1, -1, MAX_SAFE_INTEGER, empty string length, array of length 1 | Off-by-one errors are the #1 most common bug |
| 3 | **Null/Undefined** | `null` params, `undefined` optional fields, missing object keys | JavaScript's billion-dollar mistake; functions must handle missing data |
| 4 | **Empty Collections** | `[]`, `{}`, `''`, `new Map()`, `new Set()` | Iteration over nothing should not crash; `.reduce()` on empty arrays traps |
| 5 | **Type Coercion** | `"123"` vs `123`, `true` vs `1`, `"0"` vs `0` vs `false` | JS implicit coercion causes subtle bugs; strict equality prevents some |
| 6 | **Error Conditions** | Invalid input format, network timeout, file not found, permission denied | Error handling paths are often untested and fail silently in production |
| 7 | **Concurrent Operations** | Multiple async calls, race conditions, rapid successive calls | Promise.all failures, stale state, duplicate submissions |
| 8 | **Large Input** | 10,000+ items in array, deeply nested objects, very long strings | Exposes O(n^2) algorithms, stack overflows, memory issues |
| 9 | **Special Characters** | Unicode (CJK, RTL, emoji), SQL injection strings, HTML entities, newlines | Encoding bugs, XSS vectors, display corruption, CSV/export breakage |
| 10 | **State Transitions** | Before init, during async operation, after cleanup, double-init, use-after-dispose | Lifecycle bugs: calling methods on unmounted components, using closed DB |

### How to Apply the Matrix

For a function like `calculateDiscount(price, discountPercent)`:

```
Happy path:      calculateDiscount(100, 10)  => 90
Boundary:        calculateDiscount(0, 10)    => 0
                 calculateDiscount(100, 0)   => 100
                 calculateDiscount(100, 100) => 0
Null/Undefined:  calculateDiscount(null, 10) => throws or returns 0
Empty:           N/A for numbers — skip this category when inapplicable
Type coercion:   calculateDiscount("100", 10) => 90 or throws
Error:           calculateDiscount(100, -5)  => throws (negative discount)
                 calculateDiscount(100, 150) => throws (>100% discount)
Large input:     calculateDiscount(Number.MAX_SAFE_INTEGER, 50) => correct result
Special chars:   N/A for numbers
State:           N/A for pure function
```

Not every category applies to every function. Use judgment. But CHECK every category — don't skip one just because it seems unlikely.

---

## 4. Test Naming Convention

Tests are documentation. The name of a test should describe behavior, not implementation.

### The Formula

```
it('should [expected behavior] when [condition/input]')
```

### Good Names (Behavior-Focused)

```typescript
it('should return empty array when no products match filter')
it('should throw ValidationError when price is negative')
it('should apply 10% loyalty discount for customers with 5+ years membership')
it('should debounce search input and only fire after 300ms of inactivity')
it('should preserve cart items across page navigation')
it('should display "No results" message when API returns empty array')
it('should disable submit button while form is submitting')
it('should retry failed request up to 3 times')
```

### Bad Names (Implementation-Focused)

```typescript
it('should call filter() on the array')          // Tests HOW, not WHAT
it('should set isLoading to true')                // Tests internal state
it('should invoke the useEffect hook')            // Tests framework internals
it('should render a div with className "active"') // Tests CSS details
it('should dispatch SET_PRODUCTS action')         // Tests implementation detail
it('test 1')                                      // Useless
it('works')                                       // Useless
```

### Why This Matters

When a test named "should call filter() on the array" fails, you learn nothing about what broke. When "should return empty array when no products match the search filter" fails, you immediately know what behavior is broken and can fix it.

### Group with `describe`

```typescript
describe('ProductStore', () => {
  describe('addProduct', () => {
    it('should add product to the list', () => { ... });
    it('should update product count', () => { ... });
    it('should throw when product already exists', () => { ... });
  });

  describe('removeProduct', () => {
    it('should remove product from list by ID', () => { ... });
    it('should return false when product not found', () => { ... });
  });
});
```

---

## 5. Mocking Decision Tree

Mocking is powerful but dangerous. Over-mocking makes tests pass even when the real code is broken. Under-mocking makes tests slow and flaky. Follow this tree:

### Decision Flowchart

```
Is it an external system (network, file system, database, IPC)?
  YES -> MOCK IT
    - IPC calls: mock ipcRenderer.invoke / ipcMain.handle
    - Network: mock fetch / axios
    - File system: mock fs/promises
    - Database: mock sql.js or use in-memory instance
    - Timers/Date: use vi.useFakeTimers()
  NO -> Is it an internal module you own?
    YES -> Is it a simple utility (pure function, no side effects)?
      YES -> DON'T MOCK — test through the public API
      NO -> Does it have complex setup or side effects?
        YES -> MOCK IT (but consider integration test instead)
        NO -> DON'T MOCK — use the real implementation
    NO -> It's a third-party library
      -> DON'T MOCK the library itself
      -> MOCK the boundary where your code calls it
```

### MOCK these (external boundaries):

| Dependency | Why Mock | How |
|-----------|---------|-----|
| **IPC calls** (Electron) | Can't invoke main process in unit tests | `vi.mock('electron')` |
| **Network requests** | Slow, unreliable, external dependency | `vi.fn()` on fetch/axios |
| **File system** | Side effects, platform-dependent paths | `vi.mock('fs/promises')` |
| **Database** | Slow, stateful, needs setup/teardown | In-memory sql.js mock |
| **Timers/Date** | Non-deterministic, makes tests flaky | `vi.useFakeTimers()` |
| **Random/crypto** | Non-deterministic | `vi.spyOn(Math, 'random')` |
| **Environment variables** | Varies between machines | `vi.stubEnv('KEY', 'value')` |

### DON'T mock these (internal code):

| Dependency | Why Not Mock | What Instead |
|-----------|-------------|-------------|
| **Internal utility functions** | You'd test a fake instead of real code | Test through the public API |
| **React child components** | Loses integration confidence | Render the real component |
| **Zustand store logic** | Store IS the business logic | Test the real store |
| **Type transformations** | Pure functions are fast and deterministic | Test directly |
| **Validation functions** | Core business rules must be real | Test directly |

### Specific Decisions for the user Stack

| Dependency | Mock? | Strategy |
|-----------|-------|----------|
| Electron IPC (`ipcRenderer.invoke`) | YES | `vi.mock('electron')` with typed mock responses |
| sql.js database | YES | Create mock db object with `run`, `exec`, `prepare` |
| File system (`fs/promises`) | YES | `vi.mock('fs/promises')` with mock return values |
| `fetch` / network calls | YES | `global.fetch = vi.fn()` or MSW for integration tests |
| Zustand store (in component tests) | MAYBE | Prefer rendering with real store; mock only for isolation |
| Zustand store (unit testing store) | NO | Test store directly with `getState()` / `setState()` |
| React child components | NO | Render the real children; mock only for heavy/slow children |
| `Date.now()` / `setTimeout` | YES | `vi.useFakeTimers()` for deterministic time control |
| `crypto.randomUUID()` | YES | `vi.fn().mockReturnValue('test-uuid-123')` |
| `console.log/error` | YES | `vi.spyOn(console, 'error')` to verify error logging |
| CSS/Tailwind classes | NO | Never mock styling — test visible behavior instead |
| Router / Navigation | MAYBE | Mock for unit tests, use real router in integration tests |

### The Golden Rule of Mocking

**Mock at the boundary, test through the interface.**

If you are mocking 5+ things in a single test, the code under test has too many dependencies. Refactor the code, not the test.

---

## 6. File Organization

### Colocated Test Files (Preferred)

```
src/
  components/
    ProductCard/
      ProductCard.tsx
      ProductCard.test.tsx        <- Unit test right next to component
      ProductCard.stories.tsx     <- Storybook (if applicable)
    SearchBar/
      SearchBar.tsx
      SearchBar.test.tsx
  stores/
    productStore.ts
    productStore.test.ts          <- Store unit test
  utils/
    formatCurrency.ts
    formatCurrency.test.ts        <- Utility unit test
  services/
    database.ts
    database.test.ts              <- Service unit test
```

### Separate Test Directories (For Integration & E2E)

```
tests/
  integration/
    product-flow.test.ts          <- Multiple modules working together
    checkout-process.test.ts
  e2e/
    product-management.spec.ts    <- Full user flows (Playwright)
    auth-flow.spec.ts
  fixtures/
    products.json                 <- Shared test data
    customers.json
  helpers/
    render-with-providers.tsx     <- Test utilities
    mock-ipc.ts
```

### Test Types and When to Use Each

| Type | Scope | Speed | When to Use |
|------|-------|-------|-------------|
| **Unit** | Single function/component | Fast (ms) | Pure logic, data transforms, isolated components |
| **Integration** | Multiple modules together | Medium (100ms-1s) | Store + component, service + database, multi-step flows |
| **E2E** | Full application flow | Slow (seconds) | Critical user journeys, cross-module flows, visual verification |

### The Testing Pyramid

```
        /  E2E  \          <- Few (5-10): critical user journeys
       /----------\
      / Integration \      <- Some (20-50): module interactions
     /----------------\
    /    Unit Tests     \   <- Many (100+): every function/component
   /--------------------\
```

### Naming Conventions

```
*.test.ts       — Unit/integration tests (Vitest)
*.spec.ts       — E2E tests (Playwright)
*.test.tsx      — React component tests (Vitest + React Testing Library)
__mocks__/      — Manual mock files (Vitest auto-discovers)
__fixtures__/   — Test data files
```

---

## 7. Test Verification Gate

Before declaring any feature "done", pass this gate:

### Checklist

- [ ] **All new code has tests.** No untested functions, no untested branches. No exceptions. If you wrote code, you wrote tests.
- [ ] **All tests pass.** Run: `npx vitest run --reporter=verbose`
- [ ] **No tests were skipped.** `.skip` and `.todo` are for in-progress work, not for making the suite pass. No `.skip`, no `.todo` on tests that should be running. If a test is skipped, there must be a tracked issue explaining why.
- [ ] **No tests were deleted to make gate pass.** Removing a test to make the suite green is cheating. Deleting a failing test is fraud. Fix the code or fix the test.
- [ ] **Edge cases covered.** At least 3 test cases per function: happy path, edge case, error case. Critical business logic gets the full matrix (Section 3).
- [ ] **TypeScript compiles.** Run: `tsc --noEmit` — zero errors. Type errors are test failures too.
- [ ] **No `any` types in tests.** Tests should be as strictly typed as production code.
- [ ] **Assertions are meaningful.** No `expect(true).toBe(true)`. No `expect(result).toBeDefined()` when you know the exact value. Assert specific values, specific types, specific error messages.
- [ ] **Tests run in isolation.** Each test can run alone. No test depends on another test's side effects. Run with `--shuffle` to verify.
- [ ] **No flaky tests.** Run the suite 3 times. Same result every time. If a test sometimes fails, it's a real bug — in the code or in the test.
- [ ] **Mock boundaries are correct.** Are you mocking at the right level? Too many mocks = testing the mocks, not the code.

### Gate Commands

```bash
# Full verification — run all tests with verbose output
npx vitest run --reporter=verbose

# Type check
tsc --noEmit

# Check for skipped tests (should return nothing)
grep -rn "\.skip\|\.only\|xit\|xdescribe" src/ --include="*.test.*"

# Run with shuffle to detect order-dependent tests
npx vitest run --sequence.shuffle

# Run tests with coverage report
npx vitest run --coverage

# Run specific test file
npx vitest run src/utils/formatCurrency.test.ts

# Run tests matching a pattern
npx vitest run -t "should calculate discount"

# Watch mode during development
npx vitest --watch
```

---

## 8. Anti-Premature-Completion Protocol

"All tests pass" is necessary but NOT sufficient. Before marking a testing task as complete, verify:

### The 5-Point Verification

1. **Tests actually test the right behavior.** Read each test name and its assertions. Does the test verify what it claims? A test named "should validate email" that only checks `result !== null` is worthless. Would it catch a real bug?

2. **Assertions are meaningful.** Watch for these anti-patterns:
   ```typescript
   // MEANINGLESS — always passes, vacuous assertions
   expect(true).toBe(true);
   expect(result).toBeDefined();
   expect(typeof result).toBe('object');

   // MEANINGFUL — verifies specific behavior
   expect(result.total).toBe(142.50);
   expect(result.items).toHaveLength(3);
   expect(result.items[0].name).toBe('Hennessy VS');
   expect(result.errors).toEqual([]);
   ```

3. **Edge cases are covered.** Check the Edge Case Test Matrix (Section 3). For each function, verify at least: happy path, one boundary value, one error condition. Not just the happy path. What happens with empty input? Null? Huge numbers? Special characters?

4. **Negative tests exist.** It is not enough to test what should happen. Test what should NOT happen:
   ```typescript
   it('should NOT allow negative quantities', () => {
     expect(() => addToCart(product, -1)).toThrow('Quantity must be positive');
   });
   ```

5. **The test fails when the code is wrong.** The ultimate verification: temporarily break the implementation. Does the test catch the breakage? If you change a `>` to `>=` and no test fails, the boundary is not tested.

### The Mutation Test (Mental Model)

For each test, mentally ask: "If I changed the code to return a wrong value, would this test fail?" If the answer is "no," the test isn't testing anything meaningful.

---

## 9. The 3-Strike Rule

If a test keeps failing after 3 fix attempts, **STOP.**

### Why

Repeated failure means one of:
- The test expectation is wrong (you misunderstood the requirement)
- The approach is wrong (the design doesn't support the behavior)
- There's an environment issue (missing dependency, wrong config)
- The bug is deeper than you think (not in the code you're testing)

### The Protocol

```
Strike 1: Read the error message carefully. Fix the most obvious cause. Run again.
Strike 2: Step back. Re-examine the test AND the implementation. Is the test correct?
           Is the approach correct? Fix the root cause. Run again.
Strike 3: STOP. Do NOT keep trying the same approach.
           Ask the user. Explain what you tried and why it's failing.
```

### After 3 Strikes

1. **Do NOT keep trying.** Repeated failed fixes waste time and compound errors.
2. **Analyze**: Is the test testing the wrong thing? Is the architecture fighting the test?
3. **Ask the user.** Present:
   - What the test is trying to verify
   - What the failure message says
   - What you tried (3 attempts)
   - Your hypothesis for why it keeps failing
4. **Wait for guidance** before making more changes.

### Why This Rule Exists

Brute-forcing test fixes leads to:
- Tests that pass but verify the wrong thing
- Implementation contorted to satisfy a bad test
- Wasted context window and user patience
- "Fixed" tests that break again under different conditions

---

## 10. What NOT to Test

Testing everything is as bad as testing nothing. It wastes time on things that can't break and misses things that can. Focus testing effort on code YOU wrote and behavior that matters.

### Do NOT Test

| Category | Example | Why Not |
|----------|---------|---------|
| **Third-party library internals** | Testing that `Array.filter()` works correctly | The library maintainers already tested it |
| **Exact CSS class names** | `expect(el.className).toBe('bg-blue-500 p-4')` | Fragile; any Tailwind change breaks it. Test visual behavior instead |
| **Framework boilerplate** | Testing that React renders a component at all | React works. Test YOUR component's behavior |
| **Generated code** | Testing auto-generated types, GraphQL codegen | Generated code is deterministic; test the generator config instead |
| **Implementation details** | Testing internal state variable names, private methods | These can change without affecting behavior; testing them creates coupling |
| **TypeScript types at runtime** | Runtime tests for type correctness | `tsc --noEmit` already verifies types at compile time |
| **Trivial getters/setters** | `get name() { return this._name; }` | No logic to test; if it is just returning a value, trust the language |
| **Console.log calls** | Testing that debug logging happens | Unless logging IS the feature, do not test it |
| **Configuration constants** | Testing that `PORT = 3000` | Static values don't need verification |

### DO Test

| Category | Example | Why |
|----------|---------|-----|
| **Business logic** | Discount calculations, tax computation, inventory rules | This is where bugs cost money |
| **Data transformations** | CSV parsing, report generation, API response mapping | Wrong transformations corrupt data |
| **Error handling** | Invalid input handling, network failure recovery | Untested error paths crash in production |
| **User interactions** | Button clicks, form submissions, navigation | User-facing behavior must be verified |
| **State transitions** | Loading states, auth flow, cart updates | State bugs cause UI inconsistencies |
| **Edge cases** | Empty data, maximum values, concurrent operations | Edge cases are where bugs hide |

---

## 11. Reference Files

Detailed patterns and code examples are in the `references/` directory:

| File | Contents |
|------|----------|
| `vitest-patterns.md` | Complete Vitest patterns for Electron + React 18 + TypeScript + Zustand. Basic test structure, async testing, React component testing with Testing Library, Zustand store testing. |
| `mocking-strategies.md` | Detailed mocking patterns for IPC (Electron), Zustand stores, sql.js databases, file system, fetch/network, window/DOM APIs, and environment variables. |
| `playwright-e2e.md` | Playwright E2E patterns for Electron apps. App launch, page objects, fixtures, visual regression, network interception, parallel execution, CI/CD integration. |
| `cypress-patterns.md` | Cypress patterns for web-only projects. Custom commands, interceptors, component testing, visual testing, best practices. |
| `coverage-strategy.md` | Meaningful coverage targets, what to cover, what to skip, coverage configuration for Vitest, regression guard patterns. |

---

## Quick Reference Card

```
START HERE:
  1. Write a failing test (RED)
  2. Write minimal code to pass (GREEN)
  3. Clean up while green (REFACTOR)
  4. Repeat

EVERY FUNCTION GETS:
  - Happy path test
  - Edge case test
  - Error case test

MOCK:
  - External: IPC, network, filesystem, database, timers
  - Don't mock: internal utils, child components, stores, pure functions

VERIFY:
  - npx vitest run --reporter=verbose
  - tsc --noEmit
  - No skipped tests
  - Meaningful assertions
  - Edge cases covered

NAMING:
  - it('should [behavior] when [condition]')
  - describe('ModuleName') > describe('methodName') > it('should...')

FILE LOCATION:
  - Unit: colocated *.test.ts(x) next to source
  - Integration: tests/integration/
  - E2E: tests/e2e/ or e2e/

STUCK?
  - 3 strikes and ask the user
  - Don't brute-force

NEVER TEST:
  - Library internals, CSS classes, framework boilerplate, generated code
```

---

## Scope Boundaries

- **MINIMUM**: Every bug fix must include a regression test. No exceptions — if you fixed a bug, prove it stays fixed.
- **MAXIMUM**: Do not write tests for trivial getters/setters or framework internals. Testing `Array.filter()` works is not your job.

---

## Sacred Regressions

A change that improves average test metrics but BREAKS a critical test is BLOCKED:
- Critical tests: encryption, sync, IPC bridge, auth flow, data integrity
- Mark critical tests with `// @sacred` comment
- If ANY sacred test fails, the change is rejected regardless of other improvements
- Never trade a critical regression for a general improvement

---

## Weighted Rubrics per Artifact Type

Apply different test standards per artifact:

| Artifact | Correctness | Quality | Error Handling | Security | Performance |
|---|---|---|---|---|---|
| Source Code | 0.30 | 0.20 | 0.20 | 0.15 | 0.15 |
| API/Interface | 0.25 | 0.20 | 0.20 | 0.15 | 0.20 |
| Database/Schema | 0.30 | 0.15 | 0.25 | 0.20 | 0.10 |
| IPC Channel | 0.25 | 0.15 | 0.25 | 0.20 | 0.15 |

Score each 1-5, multiply by weight. Total < 3.0 = FAIL. 3.0-4.0 = REVIEW. > 4.0 = PASS.

---

## Related Skills

- **fireworks-debug** — Test failures trigger debugging; use the debug skill's 10-step protocol when a test fails unexpectedly
- **fireworks-refactor** — Characterization tests before refactoring to lock existing behavior
- **fireworks-review** — Test coverage analysis as part of multi-perspective code review
