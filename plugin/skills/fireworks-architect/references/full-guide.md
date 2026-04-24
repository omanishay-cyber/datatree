# fireworks-architect — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## Sprint Decomposition

Large tasks are broken into sprints for manageable, verifiable, parallelizable execution.

### Sprint Design Rules

1. **Size**: Each sprint contains 2-5 steps
2. **File Boundaries**: Two sprints must NEVER edit the same file — this prevents merge conflicts and enables parallel execution
3. **Self-Contained**: Each sprint produces a working, testable result
4. **Independently Verifiable**: Each sprint can be verified without depending on other sprints completing first
5. **Parallel Execution**: Independent sprints can run as parallel agents

### Sprint Template

```
SPRINT 1: [Name]
  Files: [list of files ONLY this sprint touches]
  Steps:
    1. [specific change]
    2. [specific change]
  Verification: [how to confirm this sprint succeeded]
  Dependencies: [which sprints must complete before this one]

SPRINT 2: [Name]
  Files: [list of files ONLY this sprint touches]
  Steps:
    1. [specific change]
    2. [specific change]
  Verification: [how to confirm this sprint succeeded]
  Dependencies: [none — can run in parallel with Sprint 1]
```

### Parallel Sprint Dispatch

When sprints have no dependencies on each other, dispatch them as parallel agents:

```
Agent 1 (general-purpose): Execute Sprint 1 — [files A, B]
Agent 2 (general-purpose): Execute Sprint 2 — [files C, D]
Agent 3 (general-purpose): Execute Sprint 3 — [files E, F]
```

After all parallel sprints complete, run a final verification sprint that checks integration across all modified files.

---

## Trade-Off Analysis Framework

When multiple architectural approaches exist, use this framework to make disciplined decisions:

| Criterion | Option A | Option B | Option C |
|-----------|----------|----------|----------|
| **Complexity** | How many files? How many new concepts? | | |
| **Performance** | Runtime impact? Memory impact? | | |
| **Maintainability** | Easy to understand? Easy to modify? | | |
| **User Experience** | Visible to user? Better/worse UX? | | |
| **Risk** | What could go wrong? How bad? | | |
| **Alignment** | Follows existing patterns? | | |
| **Timeline** | How long to implement? | | |
| **Recommendation** | **[CHOSEN/NOT CHOSEN]** | **[CHOSEN/NOT CHOSEN]** | **[CHOSEN/NOT CHOSEN]** |

### How to Use

1. Identify 2-3 viable approaches during the Research phase
2. Fill in the table with honest assessments (not advocacy for a preferred option)
3. Weight criteria by project context — a prototype values speed over maintainability; production code values the opposite
4. Present the table to the user with a clear recommendation and rationale
5. Let the user make the final decision

---

## Complexity Estimation

Use this scale to calibrate the depth of planning required:

| Level | Characteristics | Planning Depth | Example |
|-------|----------------|----------------|---------|
| **Trivial** | 1 file, obvious change | 1 step, minimal research | Fix a typo, update a constant |
| **Simple** | 2-3 files, clear path | 3-5 steps, read target files | Add a new UI field with IPC |
| **Medium** | 5+ files, some unknowns | 5-10 steps, full RPI | New feature with DB + UI + IPC |
| **Complex** | System-wide impact | 10+ steps, break into sprints | Refactor state management |
| **Critical** | Data integrity or security | Full spec + review before code | Auth system, encryption, sync |

### Estimation Heuristics

- Count the number of files that need to change — this is the best complexity proxy
- If you cannot list all affected files, the task is at least Medium
- If changes cross process boundaries (main/renderer), add one complexity level
- If changes affect data persistence, add one complexity level
- If changes affect security, treat as Critical regardless of file count

---

## Architecture Verification Gate

Before implementation begins, ALL of these must be true:

- [ ] **All affected files identified** — no "oh, I also need to change X" surprises
- [ ] **Data flow traced end-to-end** — from user action to database to response
- [ ] **No circular dependencies introduced** — check with `npx madge --circular`
- [ ] **Existing patterns followed** — new code matches codebase conventions
- [ ] **INVARIANTS.md contracts not violated** — run all verification commands
- [ ] **Plan reviewed by user** — explicit approval received
- [ ] **Rollback strategy identified** — if this goes wrong, how do we undo it?
- [ ] **Test strategy defined** — how will we verify correctness?

If any gate item is not satisfied, return to the Research or Plan phase.

---

## Anti-Premature-Completion

**"I planned the architecture" is NOT done.**

ACTUAL done means ALL of the following:

1. Research documented with findings format
2. Plan written with specific file:change pairs
3. Risks identified with mitigations
4. User approved the plan
5. INVARIANTS verified (no violations)
6. Code implemented following the plan
7. TypeScript compiles cleanly (`tsc --noEmit`)
8. Tests pass
9. Both light and dark themes verified (if UI changes)
10. Session file updated with completed work

**Never claim completion without verification. Run the app. Check both themes. Confirm the behavior matches the acceptance criteria.**

---

## 3-Strike Rule

If three architectural approaches fail validation:

1. **STOP immediately** — do not try a fourth approach
2. **Document what failed** — capture each approach and why it did not work
3. **Re-examine requirements** — fundamental requirements may be misunderstood
4. **Ask the user** — present the three failures and ask for clarification
5. **Consider constraints** — are there hidden constraints not yet discovered?

The 3-Strike Rule prevents infinite loops of "try something, fail, try something else" that waste context and time. When three approaches fail, the problem is almost always in the requirements, not the solution.

---

## Electron Architecture Patterns

This skill includes deep knowledge of Electron + React + TypeScript architecture. See `references/electron-architecture.md` for:

- Process model (main, preload, renderer)
- IPC architecture with typed channels
- State sync patterns between processes
- Window management lifecycle
- Security model (contextIsolation, sandbox)

## State Management Patterns

Zustand store design, selectors, middleware, and anti-patterns. See `references/state-management.md`.

## Database Design

sql.js/SQLite patterns including schema design, migrations, query optimization, and transactions. See `references/database-design.md`.

## Requirements Validation

Ensuring requirements are complete, clear, testable, consistent, and feasible before implementation begins. See `references/requirements.md`.

---

## Reference Files

| File | Purpose |
|------|---------|
| `references/rpi-methodology.md` | Full RPI protocol with parallel dispatch patterns |
| `references/invariants.md` | Complete INVARIANTS.md specification and hook integration |
| `references/electron-architecture.md` | Electron + React + TypeScript architecture patterns |
| `references/state-management.md` | Zustand architecture and anti-patterns |
| `references/database-design.md` | sql.js/SQLite patterns and optimization |
| `references/requirements.md` | Requirements validation and spec templates |

---

## Quick Reference — When to Invoke This Skill

- **New feature request** — full RPI protocol
- **Architecture decision** — trade-off analysis framework
- **Large refactor** — sprint decomposition
- **Setting up a new project** — INVARIANTS.md + architecture patterns
- **Debugging structural issues** — architecture verification gate
- **Requirements unclear** — requirements validation
- **Multiple approaches possible** — trade-off analysis + 3-strike rule

## Integration with Other Skills and Agents

This skill works best when combined with:

- **fireworks-design** — for UI/UX decisions within the architectural plan
- **fireworks-debug** — when research reveals bugs that need fixing first
- **fireworks-review** — for post-implementation quality verification
- **electron-patterns** — for Electron-specific implementation details
- **tdd-workflow** — for test-first implementation during the Execute phase
- **security-pipeline** — for Critical complexity tasks involving auth or encryption

## Activation

This skill activates automatically when:
- User requests a new feature or enhancement
- User asks for architecture advice or system design
- User wants to plan before implementing
- User says "plan", "design", "architect", "RPI", or "how should I build"
- A task is estimated as Medium complexity or higher

---

## 7-Dimension Decision Evaluation

Before committing to any architecture decision, evaluate it across seven dimensions. This prevents tunnel vision where a technically elegant solution fails on business, operational, or timeline grounds.

### Evaluation Matrix

| Dimension | Question | Weight |
|---|---|---|
| **Business Impact** | Does this serve the business goal? | HIGH |
| **Technical Risk** | Can we build and maintain this? | HIGH |
| **Operational Risk** | Can we deploy and monitor this? | MEDIUM |
| **Financial Risk** | What's the cost (dev time, infra, licenses)? | MEDIUM |
| **Timeline Risk** | Does this fit the schedule? | HIGH |
| **Team Risk** | Does the team have skills for this? | MEDIUM |
| **Market Risk** | Will this still be relevant in 6 months? | LOW |

### Scoring

Score each dimension 1-5:

| Score | Meaning |
|---|---|
| 1 | No risk / strong positive alignment |
| 2 | Minor concerns, easily mitigated |
| 3 | Moderate concerns, need mitigation plan |
| 4 | Significant concerns, may require redesign |
| 5 | Critical risk, likely to cause failure |

### Decision Thresholds

| Total Score | Action |
|---|---|
| 7-19 | **PROCEED** — risks are acceptable, move to implementation |
| 20-28 | **REVIEW** — document mitigations for every dimension scoring 3+, get user approval |
| 29-35 | **REDESIGN** — fundamental issues exist, return to Research phase with new approach |

### Example Evaluation

```
DECISION: Use sql.js (in-memory SQLite) vs PostgreSQL for Electron app

| Dimension        | sql.js | PostgreSQL |
|------------------|--------|------------|
| Business Impact  | 1      | 2          |
| Technical Risk   | 2      | 4          |
| Operational Risk | 1      | 4          |
| Financial Risk   | 1      | 3          |
| Timeline Risk    | 1      | 3          |
| Team Risk        | 1      | 3          |
| Market Risk      | 1      | 1          |
| TOTAL            | 8      | 20         |

Decision: sql.js (PROCEED) — PostgreSQL requires REVIEW due to operational
complexity in Electron desktop deployment.
```

### Integration with Trade-Off Analysis

The 7-Dimension evaluation complements the existing Trade-Off Analysis Framework:
1. Use the Trade-Off Analysis table for **comparing approaches** (Option A vs B vs C)
2. Use the 7-Dimension evaluation for **validating the chosen approach** before committing
3. If the chosen approach scores > 28, go back to the Trade-Off table and pick a different option

### When to Skip

- L0 tasks (single-file fixes) — overhead exceeds benefit
- Decisions already validated in a previous session — reference the prior evaluation
- Decisions dictated by constraints (e.g., "must use Electron" is not a choice to evaluate)

---

## Scope Boundaries

- **MINIMUM**: Always create INVARIANTS.md for L2+ projects (Simple complexity and above). Every project beyond trivial one-file changes must have architectural contracts documented and verifiable.
- **MAXIMUM**: Do not design beyond the current sprint's requirements. Resist the urge to architect for hypothetical future needs. Design for today, leave extension points for tomorrow.

---

## Related Skills

| Skill | Purpose |
|-------|---------|
| **fireworks-estimation** | Sizing — complexity estimation, effort calibration, sprint sizing |
| **fireworks-workflow** | Lifecycle — end-to-end development workflow orchestration |
| **fireworks-research** | Investigation — deep research, Context7 lookups, codebase exploration |
| **fireworks-patterns** | Pattern selection — choosing the right design patterns for the problem |
