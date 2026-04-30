# fireworks-taskmaster — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 3. Task Lifecycle

```
┌──────────┐    ┌───────┐    ┌─────────────┐    ┌────────┐    ┌──────┐
│ BACKLOG  │───>│ READY │───>│ IN-PROGRESS │───>│ REVIEW │───>│ DONE │
└──────────┘    └───────┘    └─────────────┘    └────────┘    └──────┘
     │               │              │                │
     │               │              │                └──> Back to IN-PROGRESS
     │               │              └──> BLOCKED ──> READY (when unblocked)
     │               └──> BACKLOG (deprioritized)
     └──> WONT-DO (cancelled)
```

**Transition Rules:**

| From | To | Condition |
|---|---|---|
| backlog | ready | All dependencies met, acceptance criteria defined |
| ready | in-progress | Task selected for current work block |
| in-progress | review | Implementation complete, self-check passed |
| in-progress | blocked | External dependency or question blocks progress |
| review | done | All Definition of Done criteria met |
| review | in-progress | Review found issues to address |
| blocked | ready | Blocker resolved |
| any | wont-do | Task cancelled or no longer needed |

**Status Tracking Format:**

```
Sprint: Feature-X Implementation
Goal: Users can toggle dark mode from settings

[X] T-001 Create CSS custom properties for theme colors (done)
[X] T-002 Add ThemeProvider context wrapper (done)
[>] T-003 Implement toggle switch component (in-progress)
[ ] T-004 Wire toggle to ThemeProvider (ready)
[ ] T-005 Add localStorage persistence (ready)
[!] T-006 Test both themes end-to-end (blocked: T-003, T-004)
[-] T-007 Add theme transition animation (wont-do: deferred to v2)

Progress: 2/6 done | 1 in-progress | 2 ready | 1 blocked
Burndown: ████████░░░░░░░░ 33%
```

---

## 4. Sprint Planning Quick-Reference

### Capacity Calculation

```
Available Hours = session_hours - (breaks + overhead)
Velocity Factor = 0.6 (first sprint) | rolling_average (subsequent)
Capacity = Available Hours × Velocity Factor

Example:
  4-hour session - 0.5h overhead = 3.5h available
  3.5h × 0.7 velocity = 2.45h = ~150 min of task work
  That fits about 5-6 tasks at 25min average
```

### Sprint Planning Protocol

```
1. DEFINE GOAL
   - One sentence: "By end of session, [outcome]"
   - Must be demonstrable

2. SELECT TASKS
   - Pull from backlog by priority (Must > Should > Could)
   - Only select dependency-free tasks (or tasks whose deps are also selected)
   - Stop when capacity is reached (leave 20% buffer)

3. ORDER TASKS
   - Critical path first
   - Quick wins early (builds momentum)
   - Research tasks before implementation tasks that depend on them

4. COMMIT
   - Read back the sprint goal and task list
   - Confirm with user
   - Start the clock
```

### Sprint Sizing Guide

| Session Length | Effective Capacity | Recommended Tasks |
|---|---|---|
| 30 min | 15-20 min | 1-2 tasks |
| 1 hour | 35-45 min | 2-3 tasks |
| 2 hours | 70-90 min | 4-5 tasks |
| 4 hours | 140-180 min | 6-8 tasks |
| Full day | 300-360 min | 12-15 tasks |

---

## 5. Progress Tracking

### In-Session Tracking

Use TodoWrite for live task tracking during a session. Map task status to TodoWrite status:

```
backlog    → not added (keep in YAML only)
ready      → todo
in-progress → in_progress
review     → in_progress (with note)
done       → completed
blocked    → todo (with [BLOCKED] prefix)
```

Update TodoWrite after every task transition.

### Cross-Session Tracking with GitHub Issues

For tasks that span multiple sessions, create GitHub Issues:

```bash
# Create issue from task
gh issue create \
  --title "T-001: Add dark mode toggle" \
  --body "$(cat <<'EOF'
## Task
Add a toggle switch in the settings panel for dark mode.

## Acceptance Criteria
- [ ] Toggle appears in settings panel
- [ ] Clicking toggles between light and dark
- [ ] Preference persists in localStorage

## Estimate
25 minutes

## Dependencies
None

## Sprint
Feature-X v1
EOF
)" \
  --label "task,feature,sprint:feature-x"
```

### Burndown Tracking

Track progress using ASCII burndown format:

```
Sprint Burndown — Feature-X
Hours remaining vs. ideal burn

3.0h |*
2.5h |  *  .
2.0h |     . *
1.5h |       .  *
1.0h |         .    *
0.5h |           .      *
0.0h |             .        *
     +--+--+--+--+--+--+--+-->
      T1 T2 T3 T4 T5 T6 T7

* = actual    . = ideal
Status: AHEAD of schedule (0.5h buffer)
```

Generate burndown after completing each task:

```
Completed: T-003 (25min estimated, 20min actual)
Remaining: 3 tasks, est. 75min
Elapsed: 65min of 180min capacity
Burndown: ████████████░░░░ 64% done, 36% time remaining — ON TRACK
```

---

## 6. Task Templates (Quick Reference)

These are abbreviated templates for common task types. Full detailed templates with checklists and examples are in `references/task-templates.md`.

### Bug Fix Task

```yaml
type: bug
steps:
  1. Reproduce the bug (capture exact steps)
  2. Write a failing test that demonstrates the bug
  3. Identify root cause (not just symptoms)
  4. Implement fix targeting root cause
  5. Verify fix (test passes, manual check)
  6. Check for regressions
done_when:
  - Bug no longer reproduces
  - Failing test now passes
  - No regressions in related functionality
  - Root cause documented in commit message
```

### Feature Task

```yaml
type: feature
steps:
  1. Clarify acceptance criteria with user
  2. Design approach (component structure, data flow)
  3. Implement with TDD (test first, then code)
  4. Verify in both light and dark themes
  5. Check TypeScript compilation
  6. Manual smoke test
done_when:
  - All acceptance criteria met
  - tsc --noEmit passes
  - Both themes verified
  - No console errors
```

### Refactor Task

```yaml
type: refactor
steps:
  1. Identify code smell or structural issue
  2. Write characterization tests (capture current behavior)
  3. Refactor in small, verifiable steps
  4. Run tests after each step
  5. Verify no behavior change
  6. Clean up any temporary scaffolding
done_when:
  - All existing tests still pass
  - Code smell is eliminated
  - No behavior changes
  - TypeScript compiles cleanly
```

### Research Task

```yaml
type: research
steps:
  1. Define the specific question to answer
  2. Search documentation, source code, and web
  3. Evaluate options against project constraints
  4. Synthesize findings into actionable recommendation
  5. Document sources and reasoning
done_when:
  - Question answered with evidence
  - At least 2 options evaluated
  - Recommendation includes trade-offs
  - Unknowns explicitly listed
```

---

## 7. Prioritization Quick-Reference

### MoSCoW Decision Tree

Use MoSCoW as the default prioritization framework for most development work.

```
Is the feature/fix required for the system to function?
  YES → MUST HAVE
  NO  → Does the user explicitly need this for the current milestone?
          YES → SHOULD HAVE
          NO  → Would this noticeably improve the user experience?
                  YES → COULD HAVE
                  NO  → WON'T HAVE (this time)
```

**Priority Rules:**
- Sprint MUST items: fill to 60% of capacity
- Sprint SHOULD items: fill to 80% of capacity
- Sprint COULD items: fill remaining 20% (buffer)
- WONT items: stay in backlog, revisit next sprint

### Quick RICE Score

When MoSCoW leaves you undecided between tasks of the same category:

```
RICE = (Reach × Impact × Confidence) / Effort

Reach:      How many users/sessions affected? (1-10)
Impact:     How much does it improve things? (0.25=minimal, 3=massive)
Confidence: How sure are you of the estimates? (0.5-1.0)
Effort:     Person-hours to complete (raw number)
```

Higher RICE = do first.

See `references/prioritization-frameworks.md` for full framework details, worked examples, and the Eisenhower Matrix.

---

## 8. Multi-Session Persistence

### Sprint State File

Save sprint state to a YAML file in the project root:

```yaml
# .taskmaster/sprint-current.yaml
sprint:
  name: "Feature-X Dark Mode"
  goal: "Users can toggle dark mode from settings"
  started: 2026-03-26T10:00:00
  capacity_minutes: 180
  velocity_factor: 0.7

tasks:
  - id: T-001
    subject: "Create CSS custom properties for theme colors"
    status: done
    estimate: 25min
    actual: 20min
    completed: 2026-03-26T10:20:00

  - id: T-002
    subject: "Add ThemeProvider context wrapper"
    status: done
    estimate: 25min
    actual: 30min
    completed: 2026-03-26T10:50:00

  - id: T-003
    subject: "Implement toggle switch component"
    status: in-progress
    estimate: 25min
    started: 2026-03-26T10:55:00

  - id: T-004
    subject: "Wire toggle to ThemeProvider"
    status: ready
    estimate: 20min
    dependencies: [T-003]

  - id: T-005
    subject: "Add localStorage persistence"
    status: ready
    estimate: 15min
    dependencies: [T-004]

  - id: T-006
    subject: "Test both themes end-to-end"
    status: blocked
    estimate: 20min
    dependencies: [T-003, T-004, T-005]
    blocker: "Waiting on implementation tasks"

metrics:
  tasks_done: 2
  tasks_total: 6
  time_elapsed: 55min
  time_remaining: 125min
  estimated_remaining: 80min
  status: on_track  # on_track | at_risk | behind | ahead
```

### Session Resume Protocol

When starting a new session on an existing project:

```
1. CHECK for .taskmaster/sprint-current.yaml
2. LOAD sprint state
3. DISPLAY status summary:
   - Sprint goal
   - Progress (X/Y tasks done)
   - Burndown status
   - Next ready task
4. ASK user: "Continue this sprint or re-plan?"
5. RESUME or RE-PLAN based on response
```

### Handoff Format

When ending a session, generate a handoff block:

```
## Session Handoff — [Date]

**Sprint:** Feature-X Dark Mode
**Progress:** 4/6 tasks done (67%)
**Status:** On track

**Completed this session:**
- T-001: CSS custom properties (20min)
- T-002: ThemeProvider wrapper (30min)
- T-003: Toggle component (25min)
- T-004: Wire toggle to provider (18min)

**Remaining:**
- T-005: localStorage persistence (15min est) — READY
- T-006: End-to-end theme test (20min est) — READY (deps met)

**Blockers:** None
**Notes:** Toggle animation feels sluggish, may need requestAnimationFrame.

**Next session: Start with T-005, then T-006. ~35min remaining.**
```

---

## 9. Verification Gates

Before marking any task as "done", run through the appropriate gate:

### Gate: Code Task

```
[ ] TypeScript compiles: tsc --noEmit
[ ] No console errors in dev tools
[ ] Works in light theme
[ ] Works in dark theme
[ ] All acceptance criteria met (check each one)
[ ] No unintended side effects in adjacent features
```

### Gate: Bug Fix Task

```
[ ] Original bug no longer reproduces
[ ] Regression test added and passes
[ ] Root cause documented
[ ] Related areas checked for similar bugs
[ ] tsc --noEmit passes
```

### Gate: Sprint Completion

```
[ ] All MUST tasks are done
[ ] All SHOULD tasks are done (or consciously deferred)
[ ] Sprint goal is demonstrably met
[ ] Burndown and metrics updated
[ ] Handoff document generated
[ ] Sprint state file saved
[ ] Session notes updated in memory files
```

### Gate: Multi-Session Project Milestone

```
[ ] All stories in the epic are done
[ ] Integration test across stories passes
[ ] User acceptance criteria met
[ ] Documentation updated (if applicable)
[ ] GitHub Issues closed with summary comments
[ ] Retrospective notes captured
```

---

### Formal Role Assignment (SDD Pattern)
For L3/L4 tasks, assign named roles to agents:
| Role | Responsibility | When |
|---|---|---|
| Researcher | Gather requirements, scan codebase | Phase 1 |
| Business Analyst | Clarify domain logic, user stories | Phase 1 |
| Software Architect | Design solution, define interfaces | Phase 2 |
| Tech Lead | Review architecture, approve approach | Phase 2 |
| Developer | Implement per spec | Phase 3 |
| QA Engineer | Test with weighted rubrics | Phase 4 |
| Tech Writer | Document decisions and APIs | Phase 5 |

Not all roles needed for every task — scale by complexity level.

### ACE Memory Curation
After completing major tasks, run 4-phase curation:
1. **Harvest**: Extract insights (Domain Knowledge, Solution Patterns, Anti-Patterns, Quality Gates)
2. **Curate**: Check relevance, non-redundancy, atomicity, verifiability
3. **Update**: Write to appropriate memory topic file with confidence level
4. **Validate**: Check coherence, actionability, no overlap with existing entries

---

## 10. Cross-References

- **fireworks-estimation** — Detailed estimation techniques, calibration, and historical tracking
- **fireworks-workflow** — Development workflow patterns, TDD cycles, and review protocols
- **rpi-workflow** — Research-Plan-Implement workflow for new features
- **session-wrap** — End-of-session handoff and memory update procedures
- **handoff** — Cross-session context transfer protocols
- **quality-gate** — Verification and quality assurance checks

---

## Appendix A: Quick Commands

| User Says | Taskmaster Action |
|---|---|
| "plan this feature" | Full decomposition: Feature → Epic → Story → Task |
| "break this down" | Decompose current item into smaller tasks |
| "what should I work on" | Show highest-priority ready task |
| "sprint status" | Display progress, burndown, blockers |
| "start sprint" | Run sprint planning protocol |
| "end sprint" | Run sprint review, generate handoff |
| "prioritize backlog" | Apply MoSCoW to all backlog items |
| "I'm blocked" | Record blocker, suggest alternative task |
| "done with this" | Run verification gate, update status |
| "pick up where we left off" | Load sprint state, show resume summary |

## Appendix B: ASCII Kanban Board

```
┌─ BACKLOG ──┐ ┌── READY ───┐ ┌─ IN PROG ──┐ ┌── DONE ────┐
│             │ │            │ │            │ │            │
│ T-008 Chore│ │ T-005 Feat │ │ T-003 Feat │ │ T-001 Feat │
│ T-009 Rsrch│ │ T-006 Bug  │ │            │ │ T-002 Feat │
│             │ │            │ │            │ │ T-004 Fix  │
│             │ │            │ │            │ │            │
└─────────────┘ └────────────┘ └────────────┘ └────────────┘
  WIP Limit: -    WIP Limit: 3   WIP Limit: 2   No Limit
```

**WIP Rules:**
- Maximum 2 tasks in-progress at any time (1 for claude, 1 for user)
- Maximum 3 tasks in ready state (prevents over-planning)
- If WIP limit hit, finish current work before pulling new tasks
