---
name: fireworks-architect
description: Ultimate system design superbrain — Research-Plan-Implement protocol, INVARIANTS contracts, sprint decomposition. Use for new features, refactors, and any non-trivial architectural decision.
version: 2.0.0
author: mneme
tags: [architecture, rpi, invariants, sprint, electron, trade-off, system-design]
triggers: [architect, plan, design, system design, new feature, RPI, how should I build, architecture]
---

# Fireworks Architect — Ultimate System Design Superbrain

## Core Principle

**Understanding precedes planning. Planning precedes implementation. Skipping phases is the #1 cause of rework.**

Every software change, from a one-line fix to a system redesign, benefits from the same discipline: understand the problem fully, design the solution completely, then execute precisely. The temptation to "just start coding" is the root of architectural debt, missed edge cases, and the dreaded "fixed X but broke Y" cycle.

This skill consolidates knowledge from 5 agents (super-architect, deep-think-partner, roadmap-analyst, project-planner, requirement-validator) and 2 skills (rpi-workflow, spec-driven-dev). It integrates the INVARIANTS.md pattern for machine-verifiable contracts and sprint decomposition for parallel execution.

---

## Research-Plan-Implement (RPI) Protocol

Every non-trivial change follows three mandatory phases. No phase may be skipped.

### Phase 1: RESEARCH (Mandatory)

Before writing a single line of code, build complete understanding:

1. **Read the target file** — understand its current structure, conventions, and responsibilities
2. **Read its callers** — who depends on this code? What will break if the interface changes?
3. **Read its dependencies** — what does this code depend on? Are those stable or changing?
4. **Read its tests** — what behavior is contractually guaranteed? What test patterns are used?
5. **Map data flow** — trace the path from user input to database/output and back
6. **Find similar implementations** — does the codebase already solve a similar problem? Follow that pattern
7. **Check Context7 for library APIs** — never guess at external API signatures; verify them
8. **Understand constraints** — types, performance requirements, compatibility, security boundaries

**Research Output Format:**
```
RESEARCH FINDINGS:
- Files involved: [list with full paths]
- Data flow: [path from input to output, through each layer]
- Existing patterns: [conventions this codebase follows]
- Dependencies: [libraries, modules, services consumed]
- Test coverage: [what is tested, testing patterns used]
- Constraints: [type safety, performance, compatibility, security]
- Similar implementations: [existing code doing similar things]
- Risk areas: [fragile code, areas of poor understanding]
```

**Parallel Research Dispatch:**
```
Agent 1 (Explore): Map files to be modified, read callers and dependencies
Agent 2 (Explore): Search for similar patterns in codebase
Agent 3 (Explore): Check external docs via Context7 for library APIs
```

See `references/rpi-methodology.md` for complete protocol details.

### Phase 2: PLAN (Mandatory)

Synthesize research findings into a concrete implementation plan:

1. **Design the simplest solution** that satisfies all requirements
2. **Create a numbered roadmap** — every step specifies a file and a change
3. **Identify risks** — what could go wrong? What is the mitigation?
4. **Identify INVARIANTS** — which architectural contracts apply?
5. **Present to user for approval** — never proceed without explicit sign-off

**Plan Output Format:**
```
IMPLEMENTATION PLAN:
## Summary
[1-2 sentences describing what will be built and why]

## Approach
[Why this approach over alternatives — reference trade-off analysis if multiple options]

## Steps
1. [File: /path/to/file.ts] — [What change and why]
2. [File: /path/to/file.ts] — [What change and why]
3. [File: /path/to/test.ts] — [What test and why]
...

## Risks
- Risk: [description] — Mitigation: [how to handle]
- Risk: [description] — Mitigation: [how to handle]

## INVARIANTS Check
- [ ] [Contract 1]: Will not be violated because [reason]
- [ ] [Contract 2]: Will not be violated because [reason]

## Files Modified
[Complete list of every file that will be created, modified, or deleted]
```

### Phase 3: IMPLEMENT (Mandatory)

Execute the plan with discipline:

1. **Follow the plan exactly** — do not deviate without re-planning
2. **One step at a time** — complete and verify each step before moving to the next
3. **Read before write** — always read the current state of a file before modifying it
4. **Verify after each change** — run `tsc --noEmit` after TypeScript changes
5. **Run tests** — execute relevant tests after each logical change
6. **Report completion** — summarize what was done, what was verified, and any remaining work

---

## INVARIANTS.md Pattern

Architecture contracts that are machine-verifiable. Prevents "fixed X but broke Y" by automatically checking rules after every code change.

### What Are INVARIANTS?

INVARIANTS are non-negotiable rules about the codebase that can be verified by running a command. They encode architectural decisions, security requirements, and code quality standards as executable checks.

### Format

```markdown
# INVARIANTS — [Project Name]

## [Category]
- [ ] [Contract description]: `[verification command]` = [expected result]
```

### Example INVARIANTS.md

```markdown
# INVARIANTS — your Electron project

## Type Safety
- [ ] No `any` types: `grep -rn ": any" src/ --include="*.ts" --include="*.tsx" | grep -v "node_modules" | wc -l` = 0
- [ ] tsc clean: `npx tsc --noEmit 2>&1 | grep "error" | wc -l` = 0

## Security
- [ ] No nodeIntegration: `grep -rn "nodeIntegration: true" src/ | wc -l` = 0
- [ ] contextIsolation on: `grep -rn "contextIsolation: false" src/ | wc -l` = 0

## IPC
- [ ] All handlers validate: every `ipcMain.handle` has a `.parse()` or validation call
- [ ] No sendSync: `grep -rn "sendSync" src/ | wc -l` = 0

## Architecture
- [ ] No circular deps: `npx madge --circular src/`
- [ ] No direct store access from main: `grep -rn "useStore" src/main/ | wc -l` = 0
```

### How INVARIANTS Work

1. The `check-invariants.sh` hook reads the project's INVARIANTS.md
2. After every Write/Edit operation, each verification command is executed
3. If any invariant fails (output does not match expected result), the change is flagged
4. Violations BLOCK the change until resolved

### Creating INVARIANTS for a Project

- Start with 5-10 core rules covering type safety, security, and architecture
- Add new rules when bugs reveal violations that should have been caught
- Remove rules that are no longer relevant to the project
- Keep verification commands fast (under 5 seconds each)

See `references/invariants.md` for the complete specification.

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
