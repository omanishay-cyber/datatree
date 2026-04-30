---
name: fireworks-review
description: Multi-perspective code review superbrain — 6-lens analysis, severity scoring, pragmatic filtering, PR workflows
version: 2.0.0
author: mneme
tags: [review, code-review, PR, quality, security, performance, logic]
triggers: [review, code review, PR, pull request, check, quality, review my code]
---

# Fireworks Review — Ultimate Multi-Perspective Code Review Superbrain

This skill consolidates the combined intelligence of 7 review agents (code-reviewer, pr-reviewer, pragmatic-code-reviewer, multi-perspective-reviewer, architect-reviewer, security-reviewer, code-simplifier) and the code-review-protocol skill into a single, unified review engine.

**Philosophy**: A code review is not a checkbox. It is a systematic, multi-lens investigation that produces actionable, evidence-backed findings. Every finding must have a file, a line, a code snippet, and a suggested fix. Anything less is noise.

---

## 1. Review Protocol

Every review follows these 4 steps. No shortcuts. No skipping.

### Step 1: Read All Changed Files
- Identify every file in the changeset (diff, PR, or task)
- Read each file **completely** — not just the changed lines, but surrounding context
- Understand what the code does before judging how it does it
- Note the file's existing conventions (naming, error handling, patterns)

### Step 2: Analyze Through 6 Lenses
- Apply each of the 6 review lenses systematically to every changed file
- Do NOT stop at the first finding — exhaust all lenses on all files
- Cross-reference findings across lenses (a logic bug may also be a security issue)
- Check interactions between changed files (does change in A break B?)

### Step 3: Score Findings
- Assign severity (CRITICAL / HIGH / MEDIUM / LOW) to each finding
- Assign confidence percentage (only report >= 80%)
- Group related findings together
- Prioritize by impact: data loss > security > correctness > quality > style

### Step 4: Report Actionable Items
- Use the standard output format (see section 6)
- Every finding includes: file:line, code snippet, explanation, and suggested fix
- Conclude with a clear verdict: APPROVE, REQUEST CHANGES, or NEEDS DISCUSSION
- Summarize the overall health of the changeset in 1-2 sentences

---

## 2. The 6 Review Lenses

| Lens | Focus | Key Questions |
|------|-------|---------------|
| **Logic** | Correctness | Bugs? Edge cases? Off-by-one? Null/undefined? Type coercion? Race conditions? |
| **Security** | Vulnerabilities | OWASP? CWE? Electron-specific? IPC validation? Credential exposure? Dependency risk? |
| **Performance** | Efficiency | Re-renders? Bundle size? Memory leaks? Sync ops on main thread? N+1 queries? |
| **Architecture** | Design | SOLID? Coupling? Cohesion? Abstraction levels? Pattern compliance? |
| **UI/Design** | Visual Quality | Glassmorphism? Dual theme (light+dark)? Animations? Transitions? Accessibility? |
| **Simplicity** | Minimalism | YAGNI? Over-engineering? Dead code? Magic numbers? Unnecessary abstractions? |

### Lens Application Order
1. **Logic** first — correctness is non-negotiable
2. **Security** second — vulnerabilities cannot ship
3. **Performance** third — users feel slowness immediately
4. **Architecture** fourth — maintainability affects long-term velocity
5. **UI/Design** fifth — premium quality is the standard
6. **Simplicity** last — simplify only what is already correct and secure

### Per-Lens Reference Files
Each lens has a dedicated reference file in `references/` with detailed checklists, patterns, and examples:
- `references/logic-review.md` — Bug patterns, edge cases, async issues, data integrity
- `references/security-review.md` — OWASP, CWE, Electron security, IPC validation
- `references/performance-review.md` — React re-renders, bundle, memory, sync ops, DB
- `references/architecture-review.md` — SOLID, coupling, cohesion, abstraction, patterns
- `references/simplicity-review.md` — YAGNI, over-engineering, dead code, simplification

UI/Design lens uses the premium-design skill and the project's UI standards (glassmorphism, dual theme, Framer Motion).

---

## 3. Severity Scoring

### CRITICAL (Must fix before merge)
- **Will crash the application** — unhandled null, missing import, infinite loop, stack overflow
- **Will lose user data** — silent write failure, overwrite without backup, truncation
- **Creates security vulnerability** — SQL injection, XSS, command injection, credential leak, broken auth
- **Breaks existing functionality** — regression in untouched code paths

### HIGH (Should fix before merge)
- **Incorrect behavior users will encounter** — wrong calculation, incorrect filter, bad sort order
- **Missing error handling for common cases** — network failure, invalid input, file not found
- **Accessibility blocker** — keyboard trap, missing labels, no focus management
- **Race condition** — state update ordering, concurrent writes, stale closures

### MEDIUM (Fix if easy, otherwise track)
- **Code quality / maintainability** — inconsistent naming, unclear variable names, complex conditionals
- **Missing tests for new logic** — untested branches, edge cases not covered
- **Suboptimal performance** — unnecessary re-renders, missing memoization, large bundle import
- **Minor architectural deviation** — pattern not followed, abstraction level mismatch

### LOW (Optional, don't block merge)
- **Style preference** — formatting, comment placement, import ordering
- **Nitpick** — variable naming alternatives, slight refactoring opportunity
- **Documentation** — missing JSDoc for internal function, unclear comment
- **Micro-optimization** — marginal performance gain with added complexity

### Severity Calibration Rules
- Not everything is CRITICAL. If more than 20% of findings are CRITICAL, re-evaluate.
- CRITICAL means "this will hurt users TODAY" — not "this could theoretically cause issues"
- When in doubt between two severities, choose the lower one. False alarms erode trust.
- Pre-existing issues inherit MEDIUM at most — they existed before this change

---

## 4. Confidence Threshold

**Only report findings with >= 80% confidence.**

| Confidence | Meaning | Action |
|------------|---------|--------|
| 95-100% | Certain — code evidence is conclusive | Report as definite finding |
| 80-94% | High confidence — strong evidence, minor ambiguity | Report as finding, note any assumptions |
| 60-79% | Moderate — plausible but not certain | Do NOT report. Note internally for pattern tracking |
| Below 60% | Speculative — no strong evidence | Discard. Do not mention |

### Language Matters
- **>= 80% confidence**: "This WILL cause..." / "This IS a vulnerability"
- **60-79% confidence**: Do not report, but if context demands mention: "This COULD potentially..." / "Worth verifying whether..."
- Never use definitive language for uncertain findings

### False Positive Prevention
- Before reporting, ask: "Am I certain this is actually a problem, or am I pattern-matching?"
- Check if the "issue" is actually handled elsewhere (error boundary, middleware, wrapper)
- Verify type information — TypeScript may already prevent the issue you are flagging
- Check runtime context — what runs in renderer vs main process matters

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
