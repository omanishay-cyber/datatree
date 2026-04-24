---
name: fireworks-refactor
description: Safe refactoring superbrain — code smell detection, TypeScript migration, dead code removal, pre/post verification
version: 1.0.0
author: mneme
tags: [refactoring, typescript, code-quality, dead-code, migration]
triggers: [refactor, code smell, dead code, migration, cleanup, TypeScript strict, rename, extract, consolidate]
---

# Fireworks Refactor — Safe Refactoring Superbrain

> "Refactoring is not about making code pretty. It is about making code correct, maintainable, and safe to change — without breaking anything."

This skill provides a comprehensive, safety-first refactoring system. Every refactoring operation is wrapped in verification gates that ensure no behavior changes slip through. The name "fireworks" reflects what happens when you refactor carelessly — explosions. This skill prevents that.

---

## 1. Safe Refactoring Protocol

Every refactoring MUST follow this exact sequence. No exceptions. No shortcuts.

### The 5-Step Refactoring Cycle

```
Step 1: UNDERSTAND
  - Read the code you intend to refactor, fully
  - Identify all callers / consumers / dependents
  - Map the data flow through the target code
  - Document the current behavior (inputs -> outputs)

Step 2: VERIFY BASELINE
  - Run `tsc --noEmit` — must pass clean
  - Run the full test suite — must pass clean
  - Record the baseline: test count, pass count, coverage %
  - If baseline is broken, FIX TESTS FIRST (separate commit)

Step 3: REFACTOR (one change at a time)
  - Make ONE logical refactoring change
  - Do NOT combine multiple refactorings in a single step
  - Keep each change small enough to reason about
  - If the change touches more than 3 files, break it down further

Step 4: VERIFY POST-CHANGE
  - Run `tsc --noEmit` — must still pass clean
  - Run the full test suite — must still pass clean
  - Compare: same test count, same pass count
  - Check: no new lint warnings, no new `any` types

Step 5: COMMIT (if tests pass)
  - Commit with descriptive message: `refactor: <what changed and why>`
  - If tests fail -> UNDO immediately, try a different approach
  - Never commit broken code, not even "temporarily"
```

### Critical Rules

- **One refactoring per commit** — makes bisecting and reverting trivial
- **Never refactor and add features simultaneously** — separate concerns
- **Never refactor code you do not understand** — read first, refactor second
- **Never refactor without tests** — if no tests exist, write them first (separate commit)

---

## 2. Code Smell Quick-Reference

| Smell | Symptom | Fix (One-Liner) |
|---|---|---|
| **Long Method** | Function > 20 lines, multiple levels of abstraction | Extract Method — break into focused helper functions |
| **Feature Envy** | Method uses another class's data more than its own | Move Method — relocate to the class it actually belongs to |
| **Data Clump** | Same group of variables passed together everywhere | Extract Class/Interface — group into a cohesive object |
| **Primitive Obsession** | Using strings/numbers where a domain type belongs | Replace Primitive with Value Object (e.g., `Money`, `Email`) |
| **Switch Statement Smell** | Switch/if-else chain on type discriminator in multiple places | Replace Conditional with Polymorphism or strategy map |
| **Parallel Inheritance** | Adding a subclass in one hierarchy forces a subclass in another | Merge hierarchies or use composition over inheritance |
| **Lazy Class** | Class does almost nothing, just delegates or wraps trivially | Inline Class — collapse into its consumer |
| **Speculative Generality** | Abstract class with one subclass, interface with one impl | Remove abstraction — YAGNI until proven otherwise |
| **Temporary Field** | Object field only set/used in certain conditions | Extract Class for the conditional behavior, or use Optional |
| **Message Chain** | `a.getB().getC().getD().doThing()` — long dot chains | Hide Delegate — introduce a method on `a` that encapsulates the chain |
| **Middle Man** | Class delegates almost every method to another object | Remove Middle Man — let callers use the delegate directly |
| **Shotgun Surgery** | One change requires editing many classes/files | Move Method/Field — consolidate scattered logic into one place |
| **Divergent Change** | One class changed for many different reasons | Extract Class — split by responsibility (SRP) |
| **God Class** | Class with 500+ lines, 10+ responsibilities, knows everything | Extract Class repeatedly — decompose into focused collaborators |

> Full catalog with detailed examples: `references/code-smells.md`

---

## 3. TypeScript Strict Mode Migration

### The `any` Elimination Path

```
any (unsafe) -> unknown (type-safe) -> proper type with type guard (ideal)
```

### Step-by-Step Migration

```bash
# 1. Find all `any` usage
grep -rn ": any" src/
grep -rn "as any" src/
grep -rn "<any>" src/

# 2. Categorize by difficulty
#    Easy:    `any` where the type is obvious from context
#    Medium:  `any` in function params/returns — need interface design
#    Hard:    `any` in generics or complex utility types

# 3. Replace in order: Easy -> Medium -> Hard
#    Each replacement is its own commit
```

### Strict Flag Progression

Enable flags one at a time. Fix ALL errors from one flag before enabling the next.

```jsonc
// tsconfig.json — enable in this order:
{
  "compilerOptions": {
    // Phase 1: Catch null/undefined bugs
    "strictNullChecks": true,

    // Phase 2: Catch implicit any
    "noImplicitAny": true,

    // Phase 3: Catch function type mismatches
    "strictFunctionTypes": true,

    // Phase 4: Catch uninitialized properties
    "strictPropertyInitialization": true,

    // Phase 5: Full strict (enables all of the above + more)
    "strict": true
  }
}
```

### Type Guard Patterns

```typescript
// Basic type guard
function isProduct(val: unknown): val is Product {
  return typeof val === 'object' && val !== null && 'sku' in val && 'price' in val;
}

// Discriminated union guard
function isSuccess<T>(result: Result<T>): result is SuccessResult<T> {
  return result.status === 'success';
}

// Array type guard
function isStringArray(val: unknown): val is string[] {
  return Array.isArray(val) && val.every(item => typeof item === 'string');
}
```

> Full migration reference: `references/typescript-migration.md`

---

## 4. Dead Code Detection Protocol

### Search Checklist

```bash
# 1. Unused exports — search for the export name across the codebase
#    If an exported symbol is never imported anywhere, it is dead code

# 2. Unreferenced functions — check every function/method for callers
#    Grep for the function name; if only the definition appears, it is dead

# 3. Commented-out code — REMOVE IT. Version control exists for a reason.
grep -rn "// .*function\|// .*const\|// .*return\|// .*import" src/

# 4. Unused imports — TypeScript compiler warns about these
tsc --noEmit --noUnusedLocals --noUnusedParameters

# 5. Unused dependencies — packages in package.json never imported
#    Check each dependency: grep -r "from ['\"]<package>" src/
```

### Safe Removal Protocol

Before removing any code, verify:
- No dynamic imports (`import()` with variable paths)
- No reflection or string-based lookups
- Not test-only code (check `__tests__/`, `*.test.*`, `*.spec.*`)
- Not used by build scripts or config files
- Not referenced in HTML templates or CSS

> Full dead code reference: `references/dead-code.md`

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
