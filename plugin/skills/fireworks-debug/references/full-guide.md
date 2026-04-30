# fireworks-debug — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 5. Bug Classification Decision Tree

| Type | Indicators | First Action | Reference |
|------|-----------|-------------|-----------|
| **Visual** | Wrong color/layout/missing element | Inspect CSS in DevTools | `references/bug-patterns.md` |
| **Runtime Error** | Red console error, stack trace | Read FULL stack trace | `references/error-lookup-table.md` |
| **Silent Failure** | No error, feature not working | console.log at boundaries | `references/data-flow-tracing.md` |
| **Intermittent** | Works sometimes, fails sometimes | Suspect async/race condition | `references/bug-patterns.md` |
| **Build Error** | tsc/Vite/electron-builder fails | Read exact error code | `references/build-errors.md` |
| **Electron-Specific** | Works in browser, not Electron | Check which process | `references/electron-debug.md` |
| **Memory Leak** | App slows over time | DevTools Memory tab | `references/memory-leak-detection.md` |
| **Performance** | Slow startup/action/rendering | DevTools Performance tab | `references/performance-debugging.md` |
| **Regression** | Worked before, broken now | Git bisect | `references/git-bisect.md` |
| **State Bug** | Wrong/stale data in UI | Zustand DevTools | `references/state-debugging.md` |
| **Network** | API calls failing | Network tab | `references/network-debugging.md` |

---

## 6. Red Flag Thoughts

When you catch yourself thinking any of these, STOP.

| Red Flag Thought | Why It Is Dangerous | What To Do Instead |
|-----------------|--------------------|--------------------|
| "Quick fix now, clean up later" | "Later" never comes | Design the fix properly (Step 8) |
| "Just try X and see" | Untested changes introduce new bugs | Form a hypothesis first (Step 5) |
| "It is probably just..." | Minimizing leads to shallow fixes | Trace the full path (Step 3) |
| "This should work" | "Should" is not evidence | Run the app and verify (Gates 1-6) |
| "It works on my machine" | Environment-specific bugs are real | Check all environments |
| "I will add tests later" | Technical debt accrues interest | Add the test now or note it explicitly |
| "The error message is misleading" | It almost never is | Re-read the error 3 more times |
| "Let me just restart fresh" | Restarting without understanding wastes time | Capture what you know first (Step 1) |

---

## 7. Rationalizations Table

| # | Rationalization | Reality | Counter-Action |
|---|----------------|---------|----------------|
| 1 | "The bug is too complex to reproduce" | Every bug has a cause | Gather more data. Simplify the repro. |
| 2 | "It must be a framework bug" | 99% of bugs are YOUR code | Assume your code until proven otherwise |
| 3 | "The types say it is correct" | TS checks compile-time, not runtime | Add runtime validation at boundaries |
| 4 | "I already checked that" | Memory is unreliable under stress | Check it again. Log it this time. |
| 5 | "This fix is too small to break anything" | Smallest changes cause biggest bugs | Run all 6 verification gates regardless |
| 6 | "It works in dev, so prod is fine" | Dev and prod differ in many ways | Test in production mode |
| 7 | "I do not need to understand old code" | Modifying unknown code guarantees bugs | Read the full function first |
| 8 | "The user is doing something wrong" | User behavior IS the requirement | Reproduce exactly what the user did |

---

## 8. Architecture Review (Phase 4.5)

**Trigger**: When 3+ fix attempts fail, or the same area produces repeated bugs.

1. **List recent bugs in this area** — are they symptoms of a deeper design issue?
2. **Identify the pattern** — is the code using the right abstraction?
3. **Check coupling** — is one change breaking distant code?
4. **Check state management** — is state in the right place?
5. **Check error handling** — are errors propagated or swallowed?
6. **Decision**: Localized fix sufficient, or does the area need refactoring?

If refactoring is needed, create a separate task. Do NOT mix bug fixes with refactors.

---

## 9. The 3-Strike Rule

After **3 failed hypotheses**, STOP guessing.

```
HYPOTHESIS LOG:
- H1: [hypothesis] -> DISPROVED because [evidence]
- H2: [hypothesis] -> DISPROVED because [evidence]
- H3: [hypothesis] -> DISPROVED because [evidence]
- STRIKE 3 REACHED. Requesting user guidance.
```

---

## 10. Self-Diagnosis Taxonomy (AgentRx)

When debugging stalls, diagnose YOUR OWN process:

| # | Category | Symptom | Fix |
|---|----------|---------|-----|
| 1 | **Plan Adherence** | Skipping steps | Re-read Section 3. Execute in order. |
| 2 | **Invention** | Making up facts | Verify EVERY assumption with code or logs. |
| 3 | **Invalid Invocation** | Wrong command/args | Read --help. Check docs. |
| 4 | **Misinterpretation** | Misreading errors | Re-read error 3 times, character by character. |
| 5 | **Premature Conclusion** | Done without evidence | Run all 6 verification gates. |
| 6 | **Tunnel Vision** | Only one file/area | Step back. Check full call chain. |
| 7 | **Wrong Abstraction** | Debugging UI when bug is in IPC | Classify bug (Section 5) first. |
| 8 | **Stale Context** | Outdated mental model | Re-read current code. |
| 9 | **Tool Misuse** | Wrong debugging approach | Match tool to bug type (Section 5). |

---

## 11. Smart Log Insertion Guide

**Priority 1** (highest signal): Error catch blocks, IPC boundaries, store actions.
**Priority 2**: Function entry/exit, conditional branches, loop iterations.
**Priority 3**: Variable assignments, event listeners, timer callbacks.

**Format**: `console.log('[LAYER] action:', { key: value });`

---

## 12. Scope Boundaries

**MINIMUM** (every session): Read error, classify, reproduce, find root cause, `tsc --noEmit`, verify fix.
**MAXIMUM** (never exceed without permission): No unrelated refactors, no feature additions, max 5 files, max 60 min without consulting user.

---

## 13. Quick-Ref: Common Bug Patterns

| Pattern | Symptom | Fix |
|---------|---------|-----|
| Stale closure | Old state in callback | Add to deps or use useRef |
| Silent IPC fail | No error, no result | Check channel name match |
| Async ordering | Data overwritten | AbortController or sequence ID |
| Windows paths | ENOENT on Windows | Use path.join() everywhere |
| DB race condition | Intermittent corrupt data | Async mutex + transactions |
| Type assertion crash | Runtime crash, TS passes | Runtime validation at boundaries |
| useEffect loop | Infinite re-renders | Check deps, memoize objects |
| Zustand stale | Old state in component | Use selector + shallow |
| Event listener leak | Growing memory | Return cleanup from useEffect |
| Electron require | Module not found in renderer | Use preload + contextBridge |

See `references/error-lookup-table.md` for 40+ error messages mapped to fixes.

---

## 14. Reference Files

| File | Contents |
|------|----------|
| `references/verification-gates.md` | Full 6-gate protocol with evidence templates |
| `references/bug-patterns.md` | Deep-dive patterns for Electron + React + TS + Windows |
| `references/electron-debug.md` | Electron IPC, preload, main process, packaging |
| `references/error-recovery.md` | Error boundaries, retry strategies, graceful degradation |
| `references/data-flow-tracing.md` | Layer-by-layer data flow tracing methodology |
| `references/build-errors.md` | TypeScript, Vite, Electron Builder errors |
| `references/error-lookup-table.md` | 40+ error messages mapped to causes and fixes |
| `references/error-fingerprints.md` | Error classification taxonomy and fingerprinting |
| `references/memory-leak-detection.md` | Full memory leak detection and fix workflow |
| `references/performance-debugging.md` | Performance decision tree and profiling guide |
| `references/git-bisect.md` | Regression debugging with git bisect |
| `references/stack-trace-guide.md` | How to read and interpret stack traces |
| `references/state-debugging.md` | Zustand state debugging patterns |
| `references/network-debugging.md` | API and network debugging protocol |

---

## 15. Kaizen Post-Mortem Analysis

After resolving any bug, run structured post-mortem:
- **5 Whys**: Ask "why" 5 times to find root cause behind root cause
- **Fishbone/Ishikawa**: Categorize contributing factors (Code, Environment, Process, People, Tools, Data)
- **PDCA Cycle**: Plan fix → Do minimal change → Check with tests → Act on learnings
- **Poka-Yoke**: After fix, add compile-time/type-system guards to make the bug class impossible

---

## 16. Phase 4 Empirical Validation

After any agent reports debugging findings, run a DETERMINISTIC SCRIPT to verify:
- Agent says "3 issues found" → script actually counts issues in code
- Agent says "memory leak fixed" → profiler confirms memory stable
- Agent says "race condition resolved" → stress test passes 100/100
- NEVER trust agent claims without empirical verification

---

## 17. Evidence Decay

Debug findings have a shelf life:
- Error patterns in error-registry.json decay after 90 days if not re-confirmed
- Stack traces from old versions are STALE — re-verify before applying old fixes
- Run `/fpf:actualize` concept: when code changes, check if documented bugs are still valid

---

## 18. Cross-References

| Related Skill | When To Use |
|--------------|-------------|
| `fireworks-performance` | Performance profiling, bundle analysis, render optimization |
| `fireworks-test` | Writing tests, TDD workflow, test debugging |
| `fireworks-refactor` | Code cleanup, architecture improvements (after bug is fixed) |
| `quality-gate` | Final verification before declaring work complete |
| `electron-patterns` | Electron-specific architecture and IPC patterns |

---

## Activation Checklist

When this skill activates, execute in order:

1. **Read the error** — capture every detail (Step 1)
2. **Classify** — use Decision Tree (Section 5) to pick the right approach
3. **Check error-lookup-table** — is this a known pattern with a known fix?
4. **Start T0** — attempt pattern-match fix (0-10 min)
5. **Escalate to T1** — begin 10-Step Protocol if T0 fails
6. **Track hypotheses** — invoke 3-Strike Rule at H3 (Section 9)
7. **Self-diagnose** — if stuck, check the 9 failure categories (Section 10)
8. **Verify** — run ALL 6 Verification Gates (Section 4)
9. **Anti-premature-completion** — hit every "Wait" checkpoint (Section 4)
10. **Document** — record the fix (Step 10)
