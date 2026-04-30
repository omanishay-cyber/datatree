---
name: fireworks-test
description: TDD and quality assurance superbrain — red-green-refactor, edge case matrices, Vitest, Playwright, E2E patterns
version: 2.0.0
author: mneme
tags: [test, TDD, testing, vitest, playwright, coverage, mock, edge-case, E2E]
triggers: [test, TDD, testing, vitest, playwright, coverage, mock, edge case, spec, unit test, integration test]
---

# Fireworks Test — Ultimate TDD & Quality Assurance Superbrain

> Consolidated from: super-tester, test-writer, test-fixer, tdd-guide, qa-runner, e2e-runner agents + tdd-workflow skill

---

## 1. Core Principle

**Code without tests is unverified speculation.**

The test defines what "correct" means BEFORE code is written. If you cannot describe the expected behavior in a test, you do not yet understand the requirement well enough to implement it. Tests are not afterthoughts — they are the specification. They are the contract between intent and implementation.

A test that passes gives confidence. A test that fails gives direction. No tests at all gives nothing — only hope, and hope is not a strategy.

Every function, every component, every module earns its place in the codebase by proving it works. The proof is the test. Tests are documentation that never lies because they are executed, not merely read.

A passing test suite is not proof of perfection. It is proof that every behavior you thought to verify is working. The quality of your tests determines the quality of that proof.

---

## 2. TDD Red-Green-Refactor Protocol

This protocol is strict. No exceptions. No shortcuts. No "I'll add tests later." Later never comes. Every piece of new functionality follows this cycle.

### Phase 1: RED — Write a Failing Test

1. **Identify the next behavior** to implement. One behavior at a time. Not two. Not "the whole feature." One.
2. **Write a test** that describes the expected behavior. The test is a sentence: "Given X, when Y, then Z." Use the Arrange-Act-Assert pattern:
   - **Arrange**: Set up the preconditions and inputs.
   - **Act**: Execute the function or trigger the behavior.
   - **Assert**: Verify the output or side effect matches expectations.
3. **Run the test.** It MUST fail. If it passes without writing any new code, one of these is true:
   - The behavior already exists (you're writing a redundant test — investigate)
   - The test is wrong (it's not actually testing what you think)
   - The assertion is too weak (it passes vacuously)
4. **Confirm the failure message makes sense.** The error should clearly indicate what's missing or wrong. If the failure message is cryptic, improve the test before moving on.

```
RED means: I have defined what "correct" looks like, and I've proven
that the codebase doesn't do it yet.
```

### Phase 2: GREEN — Write Minimal Code to Pass

1. **Write the simplest code** that makes the failing test pass. This is not the time for elegance, optimization, or handling edge cases. Hardcode values if you must. Return constants if that's all the test demands.
   - No extra features beyond what the test demands.
   - No premature optimization.
   - No edge case handling unless a test demands it.
   - Hard-coded return values are acceptable if they pass the test (the next test will force generalization).
2. **Run the test.** It MUST pass. If it doesn't:
   - Read the error message carefully
   - Fix the code (not the test, unless the test was wrong)
   - Run again
3. **Run ALL tests**, not just the new one. Nothing else should break. Every previously passing test must still pass. If a new change breaks an old test, you have introduced a regression.

```
GREEN means: The code now does what the test demands.
Nothing more, nothing less.
```

### Phase 3: REFACTOR — Improve While Green

1. **Look at the code.** Is there duplication? Can you extract a function? Rename a variable? Simplify a conditional? Remove dead code? Improve type annotations?
2. **Make one improvement at a time.**
3. **Run ALL tests after EVERY change.** They MUST still pass. If a test fails during refactoring, you've changed behavior, not just structure. Undo and try again.
4. **Look at the tests too.** Are they readable? Is there duplication in test setup? Can you extract a helper? Tests are production code — they deserve the same quality.
5. **Do NOT add new functionality during refactoring.** Refactoring changes structure, not behavior. If you want new behavior, go back to RED.

```
REFACTOR means: The code is cleaner, but the behavior is identical.
The tests prove it.
```

### Phase 4: REPEAT

Go back to RED. Pick the next behavior. Write the next failing test. Continue until the feature is complete.

### The Rhythm

```
RED    -> Define what "correct" means
GREEN  -> Make it correct
REFACTOR -> Make it clean
REPEAT -> Next behavior
```

**Cadence**: Each RED-GREEN-REFACTOR cycle should take 2-10 minutes. If a cycle takes more than 15 minutes, the step is too big. Break it down.

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
