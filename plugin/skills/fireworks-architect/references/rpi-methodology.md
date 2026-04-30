# Research-Plan-Implement (RPI) Methodology

## Overview

RPI is a three-phase protocol that ensures every code change is understood, designed, and executed with precision. It eliminates the "just start coding" impulse that causes rework, broken interfaces, and architectural debt.

**The RPI guarantee**: If you follow all three phases faithfully, you will never be surprised by unintended side effects.

---

## Phase 1: RESEARCH

### Purpose
Build complete understanding of the problem space before attempting any solution.

### Mandatory Steps

1. **Read the target file(s)** — understand current structure, naming conventions, import patterns, and responsibilities
2. **Read callers** — search for every file that imports/uses the target. These are your "blast radius"
3. **Read dependencies** — what does the target import? Are those interfaces stable?
4. **Read tests** — what behavior is contractually guaranteed? What patterns do tests follow?
5. **Map data flow** — trace from user action through IPC, stores, database, and back to UI
6. **Find similar implementations** — search the codebase for analogous patterns to follow
7. **Check Context7** — verify external library APIs instead of guessing
8. **Document constraints** — types, performance budgets, security requirements, compatibility needs

### Research Output Format

```
RESEARCH FINDINGS:
- Files involved: [list with full paths]
- Data flow: [path from input to output, through each layer]
- Existing patterns: [conventions this codebase follows]
- Dependencies: [libraries, modules, services consumed]
- Test coverage: [what is tested, test framework, test patterns used]
- Constraints: [type safety, performance, compatibility, security]
- Similar implementations: [existing code doing similar things, with file paths]
- Risk areas: [fragile code, areas of poor understanding, coupling]
```

### Research Phase Rules

- NEVER skip research for "simple" changes — even one-line fixes can have cascading effects
- If research reveals the problem is different from what was assumed, STOP and re-scope with the user
- Research findings must be documented before moving to Plan phase
- If you cannot find similar implementations, that is a risk — document it

### Parallel Research Dispatch

For efficiency, launch multiple Explore agents simultaneously:

```
Agent 1 (Explore): "Map all files that import or reference [target]. List each file with the specific lines that reference it."

Agent 2 (Explore): "Search the codebase for similar patterns to [feature]. Look for existing implementations of [analogous functionality]."

Agent 3 (Explore): [Use Context7] "Check the [library] documentation for [specific API]. Verify parameter types, return types, and edge cases."
```

Wait for all agents to complete. Synthesize their findings into the Research Output Format before proceeding.

### Research Depth by Complexity

| Complexity | Research Depth |
|-----------|---------------|
| Trivial | Read target file only. 2 minutes. |
| Simple | Read target + callers. Check for similar patterns. 5 minutes. |
| Medium | Full research protocol. All 8 steps. 10-15 minutes. |
| Complex | Full protocol + parallel agents. Trade-off analysis. 15-30 minutes. |
| Critical | Full protocol + security review + spec document. 30+ minutes. |

---

## Phase 2: PLAN

### Purpose
Transform research findings into a concrete, step-by-step implementation plan that can be reviewed and approved before any code is written.

### Mandatory Steps

1. **Synthesize findings** — identify the key insights from research that shape the solution
2. **Design simplest solution** — prefer the approach with fewest files, fewest new concepts, and highest alignment with existing patterns
3. **Create numbered roadmap** — every step = specific file + specific change + reason for the change
4. **Identify risks** — what could go wrong at each step? What is the mitigation?
5. **Check INVARIANTS** — which architectural contracts apply? Will the plan violate any?
6. **Present for approval** — user must explicitly approve before implementation begins

### Plan Output Format

```
IMPLEMENTATION PLAN:

## Summary
[1-2 sentences describing what will be built and why]

## Approach
[Why this approach over alternatives — reference trade-off analysis if multiple viable options exist]

## Steps
1. [File: /absolute/path/to/file.ts] — [What will change and why]
2. [File: /absolute/path/to/file.ts] — [What will change and why]
3. [File: /absolute/path/to/test.ts] — [What test will verify the change]
...

## Risks
- Risk: [description] — Mitigation: [how to handle if it occurs]
- Risk: [description] — Mitigation: [how to handle if it occurs]

## INVARIANTS Check
- [ ] [Contract]: Will not be violated because [specific reason]
- [ ] [Contract]: Will not be violated because [specific reason]

## Files Modified
- [/path/to/file1.ts] — [created | modified | deleted]
- [/path/to/file2.ts] — [created | modified | deleted]

## Verification
- [ ] tsc --noEmit passes
- [ ] [specific test] passes
- [ ] [manual check] confirmed
```

### Plan Phase Rules

- Every step in the plan must reference a specific file path — no vague "update the component"
- If the plan requires more than 10 steps, break it into sprints (see sprint decomposition)
- If the plan modifies a shared interface, list every consumer that must be updated
- The user must approve the plan before implementation begins — no exceptions
- If the user requests changes to the plan, revise and re-present

---

## Phase 3: IMPLEMENT

### Purpose
Execute the approved plan with precision, verifying each step before moving to the next.

### Mandatory Steps

1. **Follow the plan exactly** — do not add features, skip steps, or change the approach
2. **One step at a time** — complete step N and verify before starting step N+1
3. **Read before write** — always read the current file content before editing
4. **Verify after each change**:
   - Run `tsc --noEmit` after any TypeScript change
   - Run relevant tests after each logical change
   - Check for regressions in related functionality
5. **Report completion** — summarize what was done, what was verified, remaining work

### Implementation Rules

- If a step fails or produces unexpected results, STOP. Do not continue to the next step
- If the plan needs to change mid-implementation, re-enter the Plan phase
- Never modify files that are not listed in the plan without re-planning
- After all steps complete, run the full verification checklist from the plan
- Report any deviations from the plan, no matter how minor

### Implementation Verification Checklist

After all steps are complete:

- [ ] All planned steps executed
- [ ] `tsc --noEmit` passes
- [ ] All relevant tests pass
- [ ] INVARIANTS.md contracts verified
- [ ] Manual verification performed (if applicable)
- [ ] Both light and dark themes checked (if UI changes)
- [ ] No unintended file modifications
- [ ] Session file updated with completed work

---

## Common RPI Anti-Patterns

### Anti-Pattern 1: "I'll just fix this real quick"
Skipping research for "obvious" fixes. The fix works but breaks a caller you did not know about.
**Solution**: Always read callers before modifying a shared interface.

### Anti-Pattern 2: "The plan is in my head"
Not writing down the plan. Steps get forgotten, dependencies get missed.
**Solution**: Always write the plan in the output format. Always.

### Anti-Pattern 3: "I found a better way mid-implementation"
Deviating from the approved plan during implementation. The deviation causes scope creep.
**Solution**: If a better approach is found, STOP implementation. Return to Plan phase. Get approval.

### Anti-Pattern 4: "It compiles, so it works"
Treating `tsc --noEmit` as sufficient verification. Type safety does not guarantee correct behavior.
**Solution**: Always run tests AND perform manual verification.

### Anti-Pattern 5: "Let me just add this one more thing"
Adding features not in the plan. Increases complexity, introduces untested behavior.
**Solution**: Stick to the plan. File new features as separate tasks.
