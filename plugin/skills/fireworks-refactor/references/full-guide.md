# fireworks-refactor — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 5. Import Consolidation

### Barrel Files (index.ts)

```typescript
// src/components/index.ts — barrel file
export { Button } from './Button';
export { Input } from './Input';
export { Modal } from './Modal';
export { Table } from './Table';

// Consumer uses clean imports:
import { Button, Input, Modal } from '@/components';
```

### Re-Export Patterns

```typescript
// Re-export with rename (resolve naming conflicts)
export { default as ProductTable } from './ProductTable';

// Re-export entire module
export * from './types';

// Re-export subset
export { createStore, useStore } from './store';
```

### Circular Dependency Detection

```
Symptom: Runtime error "Cannot access X before initialization"
Symptom: Import returns undefined at runtime but types work

Detection:
  1. Run `madge --circular src/` if available
  2. Manual: if A imports B and B imports A, that is circular

Fix:
  1. Extract shared types/interfaces into a third file
  2. Use dependency injection instead of direct imports
  3. Restructure so dependency flows one direction
```

---

## 6. Framework Migration Rules

### The Golden Rule: NEVER Big-Bang

```
WRONG: Rewrite everything from Framework A to Framework B in one PR
RIGHT: Run old and new side by side, migrate one piece at a time
```

### Incremental Migration Strategy

```
Phase 1: INVENTORY
  - List every usage of the old framework/pattern
  - Categorize by complexity and risk
  - Identify the simplest, most isolated piece to migrate first

Phase 2: ADAPTER LAYER
  - Create an adapter/wrapper that exposes the new API
  - Old code calls the adapter, adapter calls the new implementation
  - Both old and new implementations exist simultaneously

Phase 3: GRADUAL ROLLOUT
  - Migrate one file/component/module at a time
  - Each migration is its own commit with its own verification
  - Use feature flags to toggle between old/new if needed

Phase 4: CLEANUP
  - Once all consumers use the new implementation, remove the old
  - Remove the adapter layer (it was temporary scaffolding)
  - Remove feature flags
  - Final verification: full test suite + manual smoke test
```

> Full migration reference: `references/framework-migration.md`

---

## 7. Pre/Post Verification

### Before First Change (Baseline)

```bash
# TypeScript compilation check
tsc --noEmit

# Full test suite
npm test          # or: npx vitest run / npx jest

# Lint check
npm run lint      # or: npx eslint src/

# Record results: test count, pass count, fail count, skip count
# These numbers MUST match after refactoring
```

### After EVERY Change

```bash
# Same checks, same order
tsc --noEmit
npm test
npm run lint

# Compare with baseline:
#   - Same number of tests passing
#   - No new failures
#   - No new lint warnings
#   - No new TypeScript errors
```

### If Verification Fails

```
1. UNDO the change immediately (git checkout -- . or git stash)
2. Analyze WHY the tests broke
3. The refactoring changed behavior — this is a BUG in the refactoring
4. Try a different approach that preserves behavior
5. If 3 different approaches all break tests -> STOP (see 3-Strike Rule)
```

---

## 8. YAGNI Enforcement Checklist

Before creating any abstraction, check every box:

```
[ ] Does this abstraction have MORE THAN ONE concrete implementation today?
    If no -> do not create it. Inline the logic.

[ ] Does this config system configure MORE THAN ONE value today?
    If no -> hardcode the value. Extract config when you need it.

[ ] Does this plugin system have MORE THAN ONE plugin today?
    If no -> remove the plugin system. Direct implementation.

[ ] Does this generic factory produce MORE THAN ONE type today?
    If no -> remove the generic. Use the concrete type.

[ ] Does this event system have MORE THAN ONE listener today?
    If no -> direct function call instead of event dispatch.

[ ] Does this base class have MORE THAN ONE subclass today?
    If no -> remove the base class. Inline into the one subclass.

[ ] Is this utility function used in MORE THAN ONE place today?
    If no -> inline it at the call site. Extract when reuse appears.
```

> "The best abstraction is no abstraction — until you need one." — YAGNI Principle

---

## 9. Verification Gates

Every refactoring must pass ALL gates before it can be considered complete:

```
GATE 1: TypeScript Compilation
  - `tsc --noEmit` returns exit code 0
  - No new errors or warnings compared to baseline

GATE 2: Test Suite
  - All tests that passed before still pass
  - No test count decrease (did not accidentally delete tests)
  - No new test failures

GATE 3: No New `any` Types
  - `grep -rn ": any" src/ | wc -l` must not increase
  - `grep -rn "as any" src/ | wc -l` must not increase

GATE 4: No New Lint Warnings
  - `npm run lint` warning count must not increase
  - No new suppression comments (// eslint-disable) added

GATE 5: Bundle Size
  - If applicable, `npm run build` and check output size
  - Bundle size must not increase by more than 1%
  - If it increases, justify why

GATE 6: Import Resolution
  - All imports resolve correctly
  - No circular dependencies introduced
  - No missing module errors

GATE 7: Runtime Verification
  - App starts without errors
  - Core features still work (manual smoke test)
  - No console errors in browser DevTools
```

---

## 10. Anti-Premature-Completion

### What "Done" Actually Means

```
NOT DONE:
  "I refactored the code"
  "The code looks cleaner now"
  "I moved the function to a better location"
  "I renamed the variables for clarity"

ACTUALLY DONE:
  "I refactored X. tsc --noEmit passes. All 47 tests pass (same as before).
   No new any types. No new lint warnings. Bundle size unchanged.
   Verified the app starts and core features work."
```

### Completion Checklist

```
[ ] tsc --noEmit passes clean
[ ] All tests pass (same count as baseline)
[ ] No behavior change (same inputs produce same outputs)
[ ] No new any types introduced
[ ] No new lint warnings introduced
[ ] All imports resolve
[ ] App runs without runtime errors
[ ] Both light and dark themes render correctly (if UI was touched)
[ ] Commit message describes what changed and why
```

---

## 11. The 3-Strike Rule

```
If 3 different refactoring approaches all break tests:

STOP REFACTORING.

The code has hidden coupling that you do not understand yet.

Action plan:
  1. Revert to the clean baseline
  2. Write characterization tests that capture the CURRENT behavior
     (even if the behavior seems wrong)
  3. Map every dependency — who calls this code, what it calls,
     what state it reads, what state it mutates
  4. Draw the dependency graph on paper or in a comment block
  5. Identify the hidden coupling that makes refactoring dangerous
  6. Ask the user: "This code has hidden coupling between X and Y.
     Should I untangle the coupling first (larger effort) or
     leave this code as-is for now?"

Never brute-force a refactoring. If it resists 3 times, it is
telling you something important about the codebase.
```

---

### 15 Composable Architecture Rules
Toggle these per project — not all apply everywhere:
1. Single Responsibility Principle
2. Open/Closed Principle
3. Liskov Substitution Principle
4. Interface Segregation Principle
5. Dependency Inversion Principle
6. Command-Query Separation (CQS)
7. Functional Core / Imperative Shell
8. Tell, Don't Ask
9. Law of Demeter
10. DRY (Don't Repeat Yourself)
11. YAGNI (You Aren't Gonna Need It)
12. Composition over Inheritance
13. Program to Interfaces
14. Fail Fast
15. Least Surprise

For each refactor, identify which rules are being VIOLATED and which rules guide the FIX. Never apply all 15 blindly.

---

## 12. Reference Links

### Detailed Reference Files

| Reference | Path | Content |
|---|---|---|
| Code Smells Catalog | `references/code-smells.md` | Full catalog with detection patterns, bad/good examples, technique names |
| TypeScript Migration | `references/typescript-migration.md` | `any` audit, type guards, strict flags, incremental strategy |
| Dead Code Detection | `references/dead-code.md` | Detection methods, safe removal, import/dependency cleanup |
| Framework Migration | `references/framework-migration.md` | Incremental migration, adapter pattern, feature flags, rollback |

### Canonical Books & Resources

- **Refactoring** by Martin Fowler — the definitive refactoring catalog
- **Working Effectively with Legacy Code** by Michael Feathers — refactoring without tests
- **Clean Code** by Robert C. Martin — code smell identification
- **TypeScript Handbook** — https://www.typescriptlang.org/docs/handbook/
- **Refactoring Guru** — https://refactoring.guru/refactoring/catalog

### Related Skills

- `super-refactorer` agent — dispatches refactoring tasks to subagents
- `cleanup-agent` — handles dead code removal and import consolidation
- `typescript-surgeon` — specializes in TypeScript type system work
- `quality-gate` skill — post-refactoring verification
- `tdd-workflow` skill — writing tests before refactoring
- `fireworks-patterns` — identify smells
- `fireworks-test` — characterization tests before refactoring
- `fireworks-review` — review refactored code

---

## Scope Boundaries

- **MINIMUM**: Always write characterization tests before refactoring.
- **MAXIMUM**: One refactoring technique per commit.

---

## Quick-Start Cheatsheet

```
1. Run baseline:     tsc --noEmit && npm test
2. Record results:   X tests passing, Y warnings
3. Make ONE change:  smallest possible refactoring
4. Verify:           tsc --noEmit && npm test
5. Compare:          same X tests passing, same Y warnings
6. Commit:           git commit -m "refactor: <description>"
7. Repeat from 3

If step 4 fails: UNDO, try different approach
If 3 approaches fail: STOP, analyze hidden coupling
```
