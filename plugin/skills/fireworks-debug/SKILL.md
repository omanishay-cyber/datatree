---
name: fireworks-debug
version: 2.0.0
author: mneme
description: >
  Activates when the user reports a bug, error, crash, exception, failure, or unexpected behavior.
  Triggers on symptoms like "broken", "not working", "wrong output", stack traces, or error codes.
  Scientific 10-step debugging protocol with time-boxed escalation, self-diagnosis,
  verification gates, anti-premature-completion, and Electron-specific patterns.
triggers:
  - debug
  - bug
  - error
  - crash
  - broken
  - not working
  - fix
  - wrong
  - exception
  - failure
tags:
  - debugging
  - error-handling
  - root-cause-analysis
  - electron
  - react
  - typescript
  - zustand
  - sql.js
---

# Fireworks Debug v2 — Scientific Debugging Superbrain

> Consolidates: super-debugger, debugger, error-handler, data-flow-tracer, root-cause-analyst,
> build-error-resolver, electron-debugger agents + bug-hunter, error-recovery skills.

---

## 1. Core Philosophy

**"Tests pass" is NOT evidence. The ONLY evidence is command output proving the thing works.**

- Symptoms are not causes. A symptom tells you WHERE to look, not WHAT is broken.
- Fix root causes, not symptoms. Patching a symptom creates a second bug.
- Every hypothesis must be tested. Intuition is a starting point, never a conclusion.
- The debugger who reads the error message carefully beats the one who guesses quickly.
- Reproducibility is the foundation of all debugging.
- The minimal fix is the correct fix. Refactors belong in separate commits.
- A bug is not fixed until a human can use the feature and see correct behavior end-to-end.
- Document every fix. The next person to hit this bug might be you in 3 months.

---

## 2. Time-Boxed Escalation Protocol

Every debugging session follows escalation tiers. If a tier expires without resolution, escalate.

| Tier | Time | Strategy | Actions |
|------|------|----------|---------|
| **T0: Quick** | 0-10 min | Pattern match | Check `references/error-lookup-table.md`. Read error message. Check recent changes with `git diff`. Apply known fix if pattern matches. |
| **T1: Systematic** | 10-30 min | 10-Step Protocol | Full Steps 1-10 (Section 3). Trace data flow. Binary search. Hypothesize-test cycle. |
| **T2: Deep** | 30-60 min | Expanded tools | Git bisect (`references/git-bisect.md`). Memory profiling (`references/memory-leak-detection.md`). Performance tracing (`references/performance-debugging.md`). Extensive logging. |
| **T3: Architecture** | 60+ min | Review the pattern | Architecture Review (Section 8). Question the design. Consult user. Consider refactoring entirely. |

**Rule**: Track elapsed time. When a tier expires, STOP current approach and escalate.

---

## 3. The 10-Step Scientific Debugging Protocol

### Step 1: CAPTURE
Record exact evidence before touching anything.

```
BUG REPORT:
- Error: [exact message]
- Stack: [file:line for top 3 frames]
- Repro: [step-by-step]
- Environment: [OS, versions, branch]
- First seen: [when]
- Classification: [see Section 5]
```

### Step 2: REPRODUCE
**Gate**: You MUST see the bug happen before proceeding.

- Follow exact repro steps. Run `npm run dev`. Watch terminal AND renderer console.
- If you cannot reproduce, gather more info — do NOT proceed to fix.
- Record deterministic vs intermittent. If intermittent, note frequency.

### Step 3: TRACE
Read relevant source code. Understand the execution path.

- Start at error location (file:line from stack trace). See `references/stack-trace-guide.md`.
- Trace call chain. For IPC: renderer -> preload -> main -> handler -> response.
- For state bugs: component -> store action -> state -> re-render. See `references/state-debugging.md`.
- Use data-flow-tracing methodology: see `references/data-flow-tracing.md`.

### Step 4: BINARY SEARCH
Narrow scope by isolating sections.

- Comment out half the suspicious code. Does the bug persist?
- Repeat until narrowed to 5-10 lines.
- Alternative: console.log at layer boundaries with timestamps.
- For regressions: use `git bisect`. See `references/git-bisect.md`.

### Step 5: HYPOTHESIZE
Form exactly ONE falsifiable hypothesis about the root cause.

- "The bug occurs because X is null when Y expects it to be an array."
- The hypothesis must explain ALL observed symptoms, not just some.

### Step 6: TEST
Design a minimal experiment to prove or disprove the hypothesis.

- Add a single console.log or assertion.
- If confirmed, proceed to Step 7. If disproved, return to Step 4 or 5.
- Track hypothesis count — see 3-Strike Rule (Section 9).

### Step 7: ROOT CAUSE — The "5 Whys" Drill-Down

Identify the exact line, condition, or interaction. Then drill deeper:

```
WHY 1: The product list is empty. WHY?
WHY 2: The store products array is []. WHY?
WHY 3: The IPC handler returns [] for the query. WHY?
WHY 4: The SQL WHERE clause uses 'category' but the column is 'cat_id'. WHY?
WHY 5: The schema was changed in commit abc123 but the query was not updated.
ROOT CAUSE: Schema-query mismatch introduced in commit abc123.
```

**Output**: "ROOT CAUSE: In [file]:[line], [what happens] because [why]."

### Step 8: DESIGN FIX
Plan the minimal change. Consider side effects and edge cases.

- **Wait. Before writing code, answer**: Does this fix handle null, empty array, undefined?
- If the fix touches 3+ files, pause — you may have the wrong root cause.
- Write the planned diff BEFORE implementing.

### Step 9: IMPLEMENT
Make the change. Run ALL Verification Gates (Section 4).

- Implement the planned fix — do not deviate from the design.
- Run `tsc --noEmit` immediately. Fix any type errors.
- Run through ALL 6 Verification Gates.

### Step 10: DOCUMENT
```
FIX RECORD:
- Bug: [one-line description]
- Root Cause: [what and why — include the 5 Whys chain]
- Fix: [what was changed]
- Files: [list of modified files]
- Prevention: [how to prevent this class of bug]
- Verification: [evidence confirming the fix works]
```

---

## 4. The 6 Verification Gates

Every fix MUST pass all 6 gates. See `references/verification-gates.md` for full details.

| Gate | Name | What It Checks |
|------|------|----------------|
| 1 | Static Analysis | `tsc --noEmit` passes, no new errors |
| 2 | Dev Server | App starts, no crashes, no console errors |
| 3 | Content Verification | Actual content/behavior is correct |
| 4 | Route Health | All affected pages render correctly |
| 5 | Plan Audit | Re-read original task, ALL items completed |
| 6 | E2E | Full user flow tested end-to-end |

**"Wait" Checkpoints** (from s1 paper — force yourself to pause and think):

- **Wait before Step 5** (Hypothesize): "Have I read the FULL error message and stack trace?"
- **Wait before Step 8** (Design Fix): "Does my root cause explain ALL symptoms?"
- **Wait before Step 9** (Implement): "Have I considered edge cases?"
- **Wait before declaring done**: "Have I run ALL 6 gates with actual evidence?"

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
