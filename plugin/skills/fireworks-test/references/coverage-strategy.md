# Coverage Strategy — Deep Reference

> Part of the `fireworks-test` skill. See `../SKILL.md` for the master guide.

---

## Meaningful Coverage vs 100% Coverage

100% code coverage is a vanity metric. It tells you every line was executed, not that every behavior was verified. A test suite with 100% coverage can still have zero meaningful assertions.

The goal is **meaningful coverage**: every critical behavior has a test that would fail if the behavior changed.

### The Quality Test for Coverage

For any line of code that is "covered," ask: **If I change this line, will a test fail?**

```ts
// This function has 100% line coverage from the test below
function calculateTotal(items: CartItem[]): number {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}

// But this test is MEANINGLESS despite covering 100% of the function
it('should calculate total', () => {
  const result = calculateTotal([{ price: 10, quantity: 2 }]);
  expect(result).toBeDefined(); // Passes even if the math is wrong!
});

// THIS test provides meaningful coverage
it('should sum price * quantity for all items', () => {
  const result = calculateTotal([
    { price: 10, quantity: 2 },
    { price: 5, quantity: 3 },
  ]);
  expect(result).toBe(35); // 10*2 + 5*3 = 35
});
```

---

## What to Cover: Priority Order

### Tier 1: Must Cover (Business Logic)

These are the functions where bugs cost money. Cover them extensively.

- **Price calculations** — discounts, tax, totals, markups
- **Inventory rules** — low stock thresholds, reorder points, quantity validation
- **Data transformations** — CSV import/export, report generation, data aggregation
- **Validation logic** — input validation, business rule enforcement
- **Authentication/authorization** — login, role checks, permission gates

### Tier 2: Should Cover (Data Flow)

These are the paths where data moves between systems. Cover the happy path and key error cases.

- **IPC handlers** — request/response mapping, error handling
- **Store actions** — state transitions, async operations, error states
- **API response mapping** — transform external data to internal format
- **Database queries** — CRUD operations return correct results

### Tier 3: Nice to Cover (UI Behavior)

These verify user-facing behavior. Cover the most important interactions.

- **Form submissions** — validation, submission, success/error feedback
- **Navigation flows** — routing, breadcrumbs, back/forward
- **Interactive components** — search, filter, sort, pagination
- **Loading/error states** — spinners, error messages, empty states

### What NOT to Cover

- **Framework boilerplate** — React component rendering, Zustand store creation
- **Type declarations** — `.d.ts` files, type exports
- **Generated code** — auto-generated types, codegen output
- **Constants and configuration** — static values, theme tokens, route paths
- **Simple pass-through functions** — trivial getters, re-exports, wrapper functions
- **Dev-only code** — debug logging, development tools, storybook stories

---

## Coverage Thresholds Configuration

### Vitest Coverage Config

```ts
// vitest.config.ts
export default defineConfig({
  test: {
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],

      // Include only source code
      include: ['src/**/*.{ts,tsx}'],

      // Exclude non-testable code
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/**/*.stories.{ts,tsx}',
        'src/**/*.d.ts',
        'src/main/index.ts',           // Electron entry
        'src/preload/index.ts',         // Bridge code
        'src/types/**',                 // Type declarations
        'src/**/index.ts',              // Re-export barrels
      ],

      // Thresholds — fail CI if coverage drops below these
      thresholds: {
        // Global thresholds
        branches: 70,
        functions: 70,
        lines: 75,
        statements: 75,

        // Per-directory overrides for critical code
        // 'src/utils/**': {
        //   branches: 90,
        //   functions: 90,
        //   lines: 90,
        // },
      },
    },
  },
});
```

### Recommended Thresholds

| Code Area | Line Coverage | Branch Coverage | Rationale |
|-----------|--------------|-----------------|-----------|
| **Business logic** (`utils/`, `services/`) | 85-90% | 80-85% | Bugs here cost money |
| **Store logic** (`stores/`) | 80-85% | 75-80% | State bugs cause UI corruption |
| **Components** (`components/`) | 65-75% | 60-70% | UI has many visual-only paths |
| **Main process** (`main/`) | 60-70% | 55-65% | Hard to unit test, covered by E2E |
| **Overall project** | 75% | 70% | Balanced target |

These are starting points. Adjust based on your project's maturity and bug history.

---

## Regression Test Strategy

### The Rule: Bug Fix = New Test

Every bug fix MUST include a test that:
1. **Fails** before the fix is applied (proving the bug exists)
2. **Passes** after the fix is applied (proving the fix works)
3. **Stays in the suite forever** (preventing regression)

```ts
// Bug: calculateDiscount returns negative price for 100% discount
// Reported: 2024-03-15, Ticket: #247

it('should return 0 when discount is 100% (regression #247)', () => {
  // This test failed before the fix and passes after
  const result = calculateDiscount(49.99, 100);
  expect(result).toBe(0); // Was returning -49.99
});
```

### Naming Convention for Regression Tests

Include the ticket/issue number in the test name:

```ts
it('should handle empty cart gracefully (regression #312)')
it('should not double-apply loyalty discount (regression #298)')
it('should preserve decimal precision in tax calculation (regression #351)')
```

This creates a traceable link between bugs and their prevention tests.

---

## Test Pyramid

```
         /    E2E     \          5-10 tests
        /  (Playwright) \        Critical user journeys
       /________________\        Slow (seconds each)
      /                  \
     /    Integration     \      20-50 tests
    /   (Vitest + real     \     Multi-module flows
   /    stores/services)    \    Medium speed (100ms-1s)
  /________________________\
 /                          \
/        Unit Tests           \  100+ tests
/ (Vitest + mocks)             \ Individual functions/components
/______________________________\ Fast (ms each)
```

### Distribution Guidelines

- **Unit tests (70% of total)**: Every utility function, every store action, every component in isolation.
- **Integration tests (20% of total)**: Store + component together, service + database, multi-step form flows.
- **E2E tests (10% of total)**: Complete user journeys — add product, generate report, import data, authentication flow.

### Anti-Pattern: The Inverted Pyramid

If you have more E2E tests than unit tests, your suite is:
- **Slow** — E2E tests take seconds each
- **Flaky** — more moving parts = more false failures
- **Hard to debug** — failures do not pinpoint the broken function
- **Expensive to maintain** — UI changes break many tests

Fix by converting E2E tests into unit/integration tests where possible. Keep E2E only for flows that cross multiple pages or involve real browser behavior.

---

## Running Coverage

```bash
# Run tests with coverage report
npx vitest run --coverage

# View the HTML report
open coverage/index.html

# Run coverage for a specific directory
npx vitest run --coverage src/utils/

# Check if coverage meets thresholds (for CI)
npx vitest run --coverage --coverage.thresholds.100
# Exits with non-zero if any threshold is not met
```

### CI Integration

```yaml
# In GitHub Actions
- name: Run tests with coverage
  run: npx vitest run --coverage

- name: Upload coverage to artifact
  uses: actions/upload-artifact@v4
  with:
    name: coverage-report
    path: coverage/
```

---

## Coverage Anti-Patterns

### 1. Writing Tests Just to Increase Coverage

```ts
// BAD: This test exists only to cover the line, not to verify behavior
it('should create a new instance', () => {
  const store = useProductStore; // Covers the import line
  expect(store).toBeDefined();  // Meaningless assertion
});
```

### 2. Ignoring Uncoverable Code Instead of Refactoring

```ts
// BAD: Suppressing coverage warnings on complex code
/* istanbul ignore next */
function complexUntestableFunction() {
  // 200 lines of tangled logic
}

// GOOD: Refactor so it's testable, then test it
function pureCalculation(input: Data): Result { /* ... */ }
function sideEffect(result: Result): void { /* ... */ }
```

### 3. Chasing 100% at the Expense of Test Quality

Time spent going from 90% to 100% coverage is almost always better spent writing more meaningful assertions for the existing 90%.
