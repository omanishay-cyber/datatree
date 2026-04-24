# Self-Verification Protocol — Detailed Reference

The self-verification protocol is a 6-step discipline executed before declaring any implementation work complete. It is derived from the BMAD Method v6 "Fresh Eyes" review and the Unicorn Team's anti-premature-completion research. The core insight: **the person who wrote the code is the worst judge of whether it works**, because they see what they intended, not what they built. This protocol forces you to look at evidence instead of assumptions.

Self-verification is mandatory for L1+ projects. For L0, it is optional but recommended.

---

## The 6-Step BMAD Verification Checklist

### Step 1: Re-Read the Plan

Go back to the original requirements — the PRD, tech spec, user story, or task description. Read them as if for the first time. Do not rely on your memory of what was asked.

**Verification Actions**:
1. Open the original requirement document (not your memory of it)
2. Read every acceptance criterion line by line
3. For each criterion, ask: "Where in my implementation is this satisfied?"
4. If you cannot point to a specific line of code or behavior, it is not satisfied

**Checklist**:
```
[ ] Original requirements re-read (PRD, tech spec, or story)
[ ] Every acceptance criterion reviewed against implementation
[ ] Any descoped criteria have explicit user approval documented
[ ] No requirements were silently dropped or interpreted loosely
```

**Common Failure**: Remembering what you *planned* to build instead of checking what you *actually* built. Plans diverge from implementation. Always re-read the source of truth.

---

### Step 2: Enumerate Deliverables

List every deliverable that was promised. Check each one exists and is complete. This is a physical inventory, not a mental check.

**Verification Actions**:
1. List every file that was created or modified
2. For each file, confirm the changes match the plan
3. Check for any files that should have been created but were not
4. Check for any files that were changed but should not have been (unintended side effects)
5. Run `git diff --staged` (or review all modified files) and read every changed line

**Checklist**:
```
[ ] Complete file list with change summary produced
[ ] Every changed line reviewed — no skimming
[ ] No debugging artifacts (console.log, debugger, alert)
[ ] No commented-out code left behind
[ ] No unrelated modifications mixed in
[ ] No formatting-only changes mixed with logic changes
[ ] No temporary workarounds without documentation
```

**Fresh Eyes Technique**: Before reviewing, mentally state the expected behavior in plain language. Then read the code and verify it matches. If you cannot state the expected behavior clearly, you do not understand the feature well enough to review it.

---

### Step 3: Cite Evidence for Each Requirement

For every requirement, provide concrete evidence that it is met. "I implemented it" is not evidence. Observable behavior is evidence.

**Verification Actions**:
- For UI changes: take a screenshot or describe the exact visual state
- For logic changes: show the output of a test or manual verification
- For data changes: query the database and show the result
- For IPC changes: demonstrate the round-trip from renderer to main and back
- For error handling: trigger the error condition and show the graceful recovery

**Evidence Required**:
- One piece of observable evidence per requirement
- Evidence must come from the running application, not from reading the code

**Checklist**:
```
[ ] Every functional requirement has cited evidence
[ ] Every acceptance criterion has a pass/fail determination with proof
[ ] Evidence comes from execution, not from reading code
[ ] Edge cases verified: empty input, null values, boundary conditions
[ ] Error paths verified: invalid input, network failure, permission denied
```

**Common Failure**: Confusing "the code looks correct" with "the feature works." Code that looks correct can still have runtime errors, missing imports, wrong variable names, or broken state management.

---

### Step 4: Verify the Application Runs

The application must actually start, render, and function. This is not optional, even for "small" changes.

**Verification Actions**:
1. Start the application (`npm run dev` or equivalent)
2. Navigate to the affected page or feature
3. Perform the primary user action (the main thing the feature does)
4. Perform at least one edge case action (empty input, maximum value, etc.)
5. Confirm no console errors in DevTools
6. Confirm no TypeScript errors (`tsc --noEmit`)

**Checklist**:
```
[ ] Application starts without errors
[ ] Feature page/component renders correctly
[ ] Primary user flow works end-to-end
[ ] At least one edge case tested manually
[ ] No console errors or warnings in DevTools
[ ] tsc --noEmit produces zero errors
```

**Common Failure**: Skipping app startup because "it's just a CSS change" or "I only modified types." Even type-only changes can break imports, and CSS changes can break layouts in ways that are invisible without rendering.

---

### Step 5: Check Both Themes

Every UI change must be verified in both light mode and dark mode. This is a non-negotiable rule in this codebase.

**Verification Actions**:
1. Switch to light theme and verify the feature
2. Switch to dark theme and verify the feature
3. Check for: invisible text, missing borders, broken contrast, unreadable hover states, hard-coded colors

**Checklist**:
```
[ ] Light mode: text is readable on all backgrounds
[ ] Light mode: borders and dividers are visible
[ ] Light mode: interactive elements have visible hover/focus states
[ ] Dark mode: text is readable on all backgrounds
[ ] Dark mode: borders and dividers are visible
[ ] Dark mode: interactive elements have visible hover/focus states
[ ] No hardcoded colors — all use Tailwind theme tokens
[ ] Glassmorphism effects render correctly in both themes
```

**Common Failure**: Testing only in the developer's preferred theme. Dark mode issues are the #1 source of visual regressions in this codebase.

---

### Step 6: Confirm No Regressions

Changes must not break existing functionality. The scope of regression checking scales with the change scope.

**Verification Actions**:
1. Run the full test suite (`npm test`)
2. If the change touches shared utilities, verify all consumers still work
3. If the change touches the database schema, verify existing queries still work
4. If the change touches IPC channels, verify all callers and handlers still work
5. Navigate to 2-3 adjacent features and confirm they still function

**Checklist**:
```
[ ] Full test suite passes (all tests green)
[ ] tsc --noEmit clean (catches broken imports and type mismatches)
[ ] Adjacent feature 1 verified: [name]
[ ] Adjacent feature 2 verified: [name]
[ ] No TODO, FIXME, HACK, or XXX comments left behind
[ ] No console.log or debugger statements in production code
[ ] Shared utilities still work for all consumers
```

**Common Failure**: Renaming a function in a shared module without updating all call sites. TypeScript will catch this, but only if you run `tsc --noEmit`.

---

## Anti-Premature-Completion Patterns

These patterns define what counts as evidence and what does not. Premature completion is the #1 source of rework.

### What IS Evidence

| Category | Valid Evidence |
|----------|---------------|
| UI feature works | Screenshot or description of the rendered feature in the running app |
| Logic is correct | Test output showing expected values, or manual verification with specific inputs and outputs |
| Data is persisted | Database query returning the saved data after the operation |
| Error handling works | Error triggered intentionally and graceful recovery observed |
| Performance meets target | Measurement showing the metric is within the defined bounds |
| Both themes work | Visual confirmation of correct rendering in both light and dark mode |
| No regressions | Full test suite passing, plus manual verification of adjacent features |

### What is NOT Evidence

| Invalid "Evidence" | Why It Fails |
|-------------------|--------------|
| "I implemented it" | Self-report without observable proof. Code can look correct but behave incorrectly at runtime. |
| "Tests pass" (without inspection) | Tests may not cover the actual change, or may be testing the wrong thing entirely. See the "Tests Pass" Trap below. |
| "It compiles" / "tsc --noEmit is clean" | Type-safe code can still have runtime bugs: wrong logic, stale closures, race conditions, missing state updates. |
| "The code looks right" | Reading code tests your understanding of the code, not the code's actual behavior. |
| "It worked before I changed it" | Your change may have broken it. The point of regression checking is to verify this assumption. |
| "I checked dark mode" (vague claim) | Did you check the specific component that changed, or just glance at any dark-themed page? Specificity required. |
| "No errors in console" (without navigating to the feature) | Console silence on the homepage does not prove the feature page is error-free. |

### The "Tests Pass" Trap

"Tests pass" is necessary but **not sufficient** evidence. Tests are only as good as what they test. Common traps:

1. **Tests existed before your change and still pass** — They may not test the new behavior at all. Passing old tests proves you did not break old things, not that new things work.

2. **Tests you wrote pass** — You may have written tests that validate your implementation rather than the requirement (testing what you built instead of what was asked for).

3. **Unit tests pass but integration is broken** — Individual functions work but they do not work together. The IPC handler returns correct data but the UI component does not render it.

4. **Tests pass but the UI is wrong** — Logic is correct but the rendered output does not match expectations. A sort function works but the sorted column header does not indicate the sort direction.

5. **All tests pass but coverage is low** — 100% pass rate on 3 tests means almost nothing. Coverage must be >= 80% for changed files.

**Rule**: Always verify the actual application behavior in addition to running tests. Tests are a safety net, not a substitute for verification.

---

## 3-Strike Rule

After 3 failed attempts to fix the same issue, **STOP** and ask the user.

### Why This Rule Exists

Repeated failed attempts signal one of three problems:
1. **Misunderstanding**: You are solving the wrong problem
2. **Missing context**: There is information you do not have
3. **Fundamentally wrong approach**: The solution requires a different strategy entirely

Continuing to brute-force past 3 attempts wastes context window, risks introducing new bugs through cascading "fixes," and frustrates the user who watches attempts pile up.

### How to Count Strikes

- **Strike 1**: First fix attempt that does not resolve the issue
- **Strike 2**: Second fix attempt (must be a genuinely different approach) that does not resolve the issue
- **Strike 3**: Third fix attempt — **STOP HERE**

Each strike must be a genuinely different approach. Tweaking the same line of code three times counts as one strike, not three. Changing the CSS property from `margin-top: 10px` to `margin-top: 12px` to `margin-top: 8px` is one strike because it is the same approach (adjusting the same property).

### What to Report on Strike 3

```
3-STRIKE STOP: [Issue Description]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Attempts Made:
1. [What was tried first] — Failed because: [specific reason]
2. [What was tried second] — Failed because: [specific reason]
3. [What was tried third] — Failed because: [specific reason]

Root Cause Assessment:
- [What I believe the actual root cause is]
- [Why my approaches are not addressing it]
- [What information or context I might be missing]

Suggested Next Steps:
- [Alternative approach 1 that has not been tried]
- [Alternative approach 2 that has not been tried]
- [Specific question for the user that would unblock progress]
```

### Examples of When to Stop

- Fix causes a different error each time — you are chasing symptoms, not the root cause
- Fix works in isolation but breaks when integrated — you are missing understanding of the system's interactions
- Fix requires changing code you do not fully understand — you need domain knowledge from the user
- The same test keeps failing despite logically correct code — the test may be wrong, or your assumption about the expected behavior is wrong
- Each fix introduces a new regression — the approach is fundamentally incompatible with the existing architecture

---

## Evidence Format Template

When completing a task, present verification evidence in this structured format:

```
VERIFICATION REPORT
━━━━━━━━━━━━━━━━━━

Task: [What was requested — reference requirement ID if applicable]
Level: L[0-4]
Date: [YYYY-MM-DD]

Step 1 — Re-Read Plan:
  [X] Requirements re-read: [document name]
  [X] All [N] acceptance criteria reviewed

Step 2 — Enumerate Deliverables:
  Files Changed:
  - [path/to/file1.ts] — [summary of change]
  - [path/to/file2.tsx] — [summary of change]
  - [path/to/file3.test.ts] — [summary of change]

Step 3 — Cite Evidence:
  [X] [Requirement/AC 1] — Evidence: [specific observable proof]
  [X] [Requirement/AC 2] — Evidence: [specific observable proof]
  [ ] [Requirement/AC 3] — NOT MET: [why, and what is needed]

Step 4 — Application Runs:
  [X] App starts without errors
  [X] Feature works: [specific action performed and result observed]
  [X] Edge case tested: [what was tested]
  [X] tsc --noEmit: 0 errors

Step 5 — Both Themes:
  [X] Light mode: [specific observation]
  [X] Dark mode: [specific observation]

Step 6 — No Regressions:
  [X] Test suite: [N] tests, [N] passing, [N] failing
  [X] Adjacent features verified: [feature 1], [feature 2]
  [X] No debug artifacts in code

Overall: PASS / FAIL
Issues Found: [count]
Issues Resolved: [count]
Remaining Issues: [count and details, or "None"]
```

---

## Common False-Completion Patterns

These are the most frequent ways work is incorrectly declared "done." Each one has caused real rework in production projects.

### 1. "Done" Without Running the App

**Pattern**: All code changes are made, TypeScript compiles, but the app was never started.
**Reality**: Import errors, missing environment variables, runtime type mismatches, and broken state management are only visible at runtime.
**Rule**: The app must start and the feature must be exercised before declaring done.

### 2. "Done" Without Checking Dark Mode

**Pattern**: Feature looks great in light mode. Dark mode is not checked.
**Reality**: Hard-coded colors, missing `dark:` Tailwind variants, invisible text on dark backgrounds, broken contrast ratios.
**Rule**: Both themes must be visually verified for every UI change.

### 3. "Done" With Only Happy Path Tested

**Pattern**: The main flow works. Empty input, null values, maximum lengths, and error conditions are not tested.
**Reality**: Users will encounter edge cases within hours. Untested edge cases become production bugs.
**Rule**: Test at least one edge case and one error path per feature.

### 4. "Done" Without Checking Adjacent Features

**Pattern**: The new feature works perfectly, but a shared utility was changed and an adjacent feature now crashes.
**Reality**: Shared code means shared risk. Any change to shared modules requires verifying all consumers.
**Rule**: Navigate to 2-3 features that share code with the changed modules.

### 5. "Done" Based on Code Reading Alone

**Pattern**: The code looks correct. No tests were run, no app was started, no output was observed.
**Reality**: Code that looks correct can have subtle bugs — wrong variable references, off-by-one errors, stale closures, race conditions.
**Rule**: Evidence must come from execution, not from reading source code.

### 6. "Done" With TODO Comments Left Behind

**Pattern**: The feature is implemented but contains `// TODO: handle edge case` or `// FIXME: temporary workaround`.
**Reality**: TODOs are deferred bugs. If the edge case matters enough to note, it matters enough to implement now.
**Rule**: No TODO, FIXME, HACK, or XXX comments in completed work. Either implement it or explicitly descope it with user approval.

### 7. "Done" Without Updating Status Files

**Pattern**: The code is complete and verified, but `workflow-status.yaml` and session files are not updated.
**Reality**: The next session starts with stale state. Work may be repeated or skipped because the status file does not reflect reality.
**Rule**: Update `workflow-status.yaml` and session notes after every milestone completion.

---

## Quality Check Standards

### Quality Metrics Thresholds

| Metric | Threshold | Action if Exceeded |
|--------|-----------|-------------------|
| Function length | 50 lines | Split into smaller functions |
| Nesting depth | 3 levels | Use early returns or extract helper |
| Parameters per function | 4 | Use an options object |
| Cyclomatic complexity | 10 | Simplify control flow |
| File length | 300 lines | Consider splitting module |
| Duplicated blocks | 0 | Extract to shared utility |

### Security Questions

| # | Question | What to Check | Pass Criteria |
|---|----------|---------------|---------------|
| 1 | Are any secrets in the code? | API keys, passwords, tokens, encryption keys | No secrets in source; all in env vars or encrypted config |
| 2 | Is PII protected? | User names, emails, addresses, financial data | No PII in logs, error messages, or analytics |
| 3 | Is input validated? | Form inputs, IPC messages, query parameters, file paths | All input validated at entry point; types checked; length limited |
| 4 | Is authentication enforced? | Protected routes, API endpoints, IPC handlers | Every protected resource checks auth before access |
| 5 | Is authorization enforced? | Role-based access, data ownership | Users can only access their own data |
| 6 | Are common attacks prevented? | SQL injection, XSS, path traversal | Parameterized queries; output encoding; path validation |

### Test Coverage Thresholds by Level

| Level | Minimum Coverage | Target Coverage |
|-------|-----------------|-----------------|
| L0 | 0% (existing tests pass) | N/A |
| L1 | 60% | 80% |
| L2 | 80% | 90% |
| L3 | 80% | 90% |
| L4 | 90% | 95% |

---

## Protocol Execution Summary

Execute all 6 steps in order. Record the result of each step using the evidence format template above.

```
SELF-VERIFICATION RESULT
━━━━━━━━━━━━━━━━━━━━━━━
Step 1: Re-Read the Plan           [PASS/FAIL — details]
Step 2: Enumerate Deliverables     [PASS/FAIL — details]
Step 3: Cite Evidence              [PASS/FAIL — details]
Step 4: Verify App Runs            [PASS/FAIL — details]
Step 5: Check Both Themes          [PASS/FAIL — details]
Step 6: Confirm No Regressions     [PASS/FAIL — details]

Overall: [PASS/FAIL]
Issues Found: [count]
Issues Resolved: [count]
Remaining Issues: [count and details]
```

If any step FAILs, fix the issues and re-run that step. Do not re-run steps that already passed unless the fix could have affected them. If the same step fails 3 times, invoke the 3-Strike Rule and escalate to the user.
