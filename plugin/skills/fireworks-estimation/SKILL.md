---
name: fireworks-estimation
description: PERT estimation, 3-point estimates, risk buffers, sprint velocity tracking, story sizing, complexity assessment, and project timeline prediction
version: 2.0.0
author: mneme
tags: [estimation, PERT, sprint, velocity, sizing, complexity, timeline]
triggers: [estimate, how long, timeline, sprint, velocity, points, sizing, PERT, capacity, when will it be done]
---

# Fireworks Estimation Skill

Consolidated estimation, sprint planning, and project metrics framework derived from
BMAD Method and Unicorn Team methodology. This skill provides structured protocols for
accurate software estimation, sprint capacity planning, complexity assessment, risk
quantification, and delivery tracking.

---

## 1. Estimation Protocol

Every estimate MUST follow this six-step protocol. Skipping steps produces garbage numbers.

### Step 1: DECOMPOSE Exhaustively

Break the work into atomic tasks. Each task MUST be estimable independently.

- **Maximum granularity**: No single task exceeds 8 hours of effort
- **Decomposition test**: Can you describe exactly what "done" looks like for this task?
- **If uncertain**: The task is too large. Break it further.
- **Hierarchy**: Epic > Feature > Story > Task > Subtask
- **Rule of thumb**: If you cannot list the files you will touch, decompose more

```
BAD:  "Build user authentication" (too vague, too large)
GOOD: "Create login form component" (4h)
      "Implement JWT token generation" (3h)
      "Add password hashing with bcrypt" (2h)
      "Write login API endpoint" (3h)
      "Add token refresh logic" (4h)
      "Create auth middleware" (2h)
      "Write auth integration tests" (4h)
```

### Step 2: IDENTIFY Unknowns

Unknowns are the primary source of estimation error. Categorize every unknown:

| Category | Examples | Impact |
|---|---|---|
| **Technical** | New library, unfamiliar API, performance constraints | 1.3-2.0x multiplier |
| **Domain** | Unclear business rules, edge cases undefined | 1.4-2.5x multiplier |
| **External** | Third-party API reliability, vendor response time | 1.5-3.0x multiplier |
| **Resource** | Team availability, skill gaps, onboarding time | 1.2-2.0x multiplier |

For each unknown, decide: **Can it be resolved before estimation?** If yes, resolve it first.
If no, apply the appropriate risk buffer.

### Step 3: THREE-POINT Estimate

For every decomposed task, provide three numbers:

- **O (Optimistic)**: Everything goes perfectly. No surprises. You have done this exact thing before.
- **R (Realistic)**: Normal conditions. Some minor hiccups. Typical day.
- **P (Pessimistic)**: Significant obstacles. Unforeseen complexity. Not catastrophic, but bad.

Rules:
- O must be achievable, not fantasy (minimum possible, not zero)
- P must be plausible, not apocalyptic (worst reasonable, not heat death of universe)
- R should be your gut feel after careful thought
- **If P > 3x R, the task MUST be decomposed further** — your uncertainty is too high

### Step 4: CALCULATE PERT Expected Duration

```
Expected = (O + 4*R + P) / 6
Standard Deviation = (P - O) / 6
```

The PERT formula weights the realistic estimate 4x because it is the most likely outcome.

**Confidence intervals:**
- 68% confidence: Expected +/- 1 SD
- 95% confidence: Expected +/- 2 SD
- 99.7% confidence: Expected +/- 3 SD

Example:
```
Task: "Implement JWT token generation"
O = 2 hours, R = 3 hours, P = 6 hours
Expected = (2 + 4*3 + 6) / 6 = 20/6 = 3.33 hours
SD = (6 - 2) / 6 = 0.67 hours
68% range: 2.67 - 4.00 hours
95% range: 2.00 - 4.67 hours
```

### Step 5: APPLY Risk Buffers

Based on the unknowns identified in Step 2, apply multiplicative buffers:

| Risk Level | Multiplier | When to Apply |
|---|---|---|
| **Low** | 1.0 - 1.2x | Well-understood work, proven patterns, experienced team |
| **Medium** | 1.2 - 1.5x | Some unknowns, familiar domain but new implementation |
| **High** | 1.5 - 2.0x | Significant unknowns, new technology, complex integration |
| **Critical** | 2.0 - 3.0x | Novel domain + new tech, regulatory requirements, external dependencies |

**Compound risk**: When multiple risk categories apply, MULTIPLY the factors:
```
Technical risk (1.3) x Domain risk (1.4) = 1.82x total buffer
```

### Step 6: ADD Integration Buffer

**"Integration is where 50% of bugs live."**

After summing all task estimates with their risk buffers, add an integration buffer:

- **Simple integration** (few touchpoints, well-defined interfaces): 20%
- **Moderate integration** (multiple systems, some shared state): 25%
- **Complex integration** (many systems, shared state, real-time): 30%

```
Final Estimate = Sum(Task PERT Estimates x Risk Buffers) x Integration Buffer
```

---

## 2. Story Sizing (Fibonacci Points)

Story points measure **relative complexity**, not hours. Use Fibonacci numbers to force
honest assessment of uncertainty at larger sizes.

| Points | Complexity | Time Estimate | Scope | Files Touched |
|---|---|---|---|---|
| **1** | Trivial | < 2 hours | Config change, typo fix, simple CSS | 1 file |
| **2** | Simple | 2-4 hours | Small feature, add field, simple validation | 1-2 files |
| **3** | Moderate | 4-8 hours | Standard feature, CRUD operation, new component | 2-3 files |
| **5** | Complex | 1-2 days | Multi-component feature, state management, API integration | 3-5 files |
| **8** | Very Complex | 2-3 days | Cross-cutting concern, new subsystem, complex algorithm | 5+ files |
| **13+** | **MUST BREAK DOWN** | N/A | Too large to estimate accurately | N/A |

### Decision Tree for Sizing

```
Is this a config/copy change only?
  YES -> 1 point
  NO  -> Does it require new business logic?
           NO  -> 2 points (simple wiring/plumbing)
           YES -> Is the logic well-understood?
                    YES -> Does it cross component boundaries?
                             NO  -> 3 points
                             YES -> 5 points
                    NO  -> Does it require research/spikes?
                             NO  -> 5 points
                             YES -> 8 points (or break down if > 8)
```

### Sizing Rules

1. Compare against **anchor stories** — reference stories the team has already completed
2. When in doubt, round UP to the next Fibonacci number
3. Stories > 8 points MUST be decomposed before sprint commitment
4. Bugs default to 3 points unless proven simpler or harder
5. Spikes (research tasks) are always timeboxed: 1 point = 2 hours, max 8 points

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
