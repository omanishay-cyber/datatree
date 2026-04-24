# Sprint Patterns — Fireworks Taskmaster

Patterns, protocols, and anti-patterns for running effective development sprints in AI-assisted coding sessions.

---

## Sprint Setup Protocol

Run this protocol at the start of every sprint (whether a 1-hour session or a multi-day effort).

### Step 1: Define the Sprint Goal

Write one clear sentence that describes the demonstrable outcome.

```
Format: "By the end of this sprint, [who] can [do what]."

Good:  "By the end of this sprint, the user can toggle dark mode from settings."
Bad:   "Work on dark mode stuff."
Bad:   "Implement dark mode toggle, persistence, transitions, and theme provider."
       (That's a task list, not a goal.)
```

**Goal Rules:**
- One sentence maximum
- Must be demonstrable (you can show it working)
- Must be achievable within the sprint capacity
- Written from the user's perspective when possible

### Step 2: Calculate Capacity

```
Session hours:        ___
Minus overhead:       - 0.5h (standup, planning, review)
Available hours:      ___
Times velocity:       x ___ (use 0.6 for first sprint, rolling avg after)
Sprint capacity:      ___ hours = ___ minutes
```

### Step 3: Select Tasks

Pull tasks from the backlog using this algorithm:

```
1. Filter: status == "ready" AND dependencies all met
2. Sort: priority (must > should > could), then RICE score
3. Fill:
   a. Add all MUST items first (target: 60% capacity)
   b. Add SHOULD items (target: 80% capacity)
   c. Add COULD items only if buffer remains (target: 100%)
4. Validate:
   - Total estimate <= capacity
   - No circular dependencies
   - Sprint goal is achievable with selected tasks
```

### Step 4: Commit

```
Read back to user:
  "Sprint goal: [goal]
   Tasks: [count] tasks, [total estimate] estimated
   Capacity: [capacity] available

   [List each task with ID, subject, and estimate]

   Ready to start?"
```

Only begin work after user confirms.

---

## Daily Standup Format

For multi-session sprints, start each session with a standup. Keep it to 3 lines maximum.

```
STANDUP — [Date]
Done:    [What was completed since last session]
Doing:   [What will be worked on this session]
Blocked: [Any blockers, or "None"]
```

### Example

```
STANDUP — 2026-03-26
Done:    T-001 theme CSS properties, T-002 ThemeProvider wrapper
Doing:   T-003 toggle component, T-004 wire to provider
Blocked: None
```

### Standup Rules

- Do this BEFORE opening any code files
- Check .taskmaster/sprint-current.yaml for state
- If blocked, identify an alternative task immediately
- Update TodoWrite with current task statuses

---

## Sprint Review Format

Run this protocol at the end of every sprint.

### Part 1: Demo (2 minutes)

Show the sprint goal outcome working. This is a binary check:

```
Sprint Goal: "User can toggle dark mode from settings"
Demo Result: [ ] ACHIEVED  /  [ ] NOT ACHIEVED

If not achieved, explain:
  - What is missing?
  - What blocked completion?
  - How much more work remains?
```

### Part 2: Metrics (1 minute)

```
SPRINT METRICS
  Tasks planned:     [N]
  Tasks completed:   [N]
  Tasks carried over: [N]

  Time estimated:    [X] min
  Time actual:       [X] min

  Velocity:          [completed / planned] = [ratio]
  Accuracy:          [actual / estimated] = [ratio]

  Update rolling velocity: ([v1] + [v2] + [v3]) / 3 = [new avg]
```

### Part 3: Retrospective (2 minutes)

Answer three questions:

```
RETROSPECTIVE
  What went well?
    - [1-2 specific things]

  What could improve?
    - [1-2 specific things]

  What will we change next sprint?
    - [1 concrete action item]
```

### Sprint Review Output

Combine all three parts into a review block and save to the sprint state file:

```yaml
review:
  goal_achieved: true
  tasks_planned: 6
  tasks_completed: 5
  tasks_carried: 1
  estimated_minutes: 150
  actual_minutes: 140
  velocity: 0.83
  accuracy: 0.93
  went_well:
    - "TDD approach caught a bug before it reached UI"
    - "Task decomposition was the right granularity"
  improve:
    - "Underestimated T-003 by 10min — toggle had edge cases"
  change_next:
    - "Add 5min buffer to any task involving UI state"
```

---

## Velocity Tracking

Velocity measures how much of your planned work you actually complete. Track it across sprints to improve estimation accuracy.

### Calculation

```
Sprint Velocity = Tasks Completed / Tasks Planned

3-Sprint Rolling Average:
  Sprint 1: 4/6 = 0.67
  Sprint 2: 5/6 = 0.83
  Sprint 3: 5/5 = 1.00

  Rolling Average = (0.67 + 0.83 + 1.00) / 3 = 0.83
```

### Using Velocity for Planning

```
Next sprint capacity = Available Hours x Velocity Factor

If rolling average velocity = 0.83:
  4 hours available x 0.83 = 3.32 hours of task capacity
  Plan for ~200 minutes of tasks (not 240)
```

### Velocity Trends

| Trend | Meaning | Action |
|---|---|---|
| Increasing | Getting faster or better at estimating | Slightly increase sprint load |
| Stable | Predictable delivery | Maintain current planning approach |
| Decreasing | Tasks harder than expected or interruptions increasing | Reduce sprint load, investigate cause |
| Volatile | Estimates are unreliable | Focus on decomposition quality |

### First Sprint

For the very first sprint with no historical data:

```
Use velocity factor = 0.6 (conservative)
This means: plan for 60% of your available time

After 3 sprints, switch to rolling average.
```

---

## Burndown Chart Interpretation

The burndown chart shows remaining work over time. Compare actual burn against the ideal (straight) line.

### Reading the Chart

```
Remaining
Work (hrs)
  3.0 |*
  2.5 |  .  *
  2.0 |    .   *
  1.5 |      .    *
  1.0 |        .     *
  0.5 |          .      *
  0.0 |            .       *
      +--+--+--+--+--+--+--+-->
       T1 T2 T3 T4 T5 T6 T7

  . = ideal line (straight from start to zero)
  * = actual remaining work
```

### Patterns

**On Track** — Actual line follows ideal line closely.
```
Action: Keep going. No changes needed.
```

**Ahead of Schedule** — Actual line is below ideal line.
```
  2.0 |*
  1.5 |  .
  1.0 |  * .
  0.5 |      .
  0.0 |  *     .
Action: Pull in a COULD task from backlog, or use buffer for polish.
```

**Behind Schedule** — Actual line is above ideal line.
```
  2.0 |*
  1.5 |  *  .
  1.0 |    *  .
  0.5 |    *    .
  0.0 |           .
Action:
  1. Check for blockers — remove them
  2. Reduce scope — move a COULD or SHOULD to backlog
  3. If still behind — reduce to MUST items only
  4. Communicate to user: "We may not hit the full goal"
```

**Flat Line (Blocked)** — No progress for 2+ tasks.
```
  2.0 |*
  1.5 |  *  .
  1.0 |  *    .
  0.5 |  *      .
  0.0 |           .
Action:
  1. You are stuck. STOP working on the current task.
  2. Identify the blocker explicitly
  3. Switch to an unblocked task
  4. If all tasks blocked, escalate to user
```

**Scope Creep** — Line goes UP instead of down.
```
  2.0 |*
  1.5 |  .
  2.0 |    * .      <- work INCREASED
  1.5 |        .
Action:
  1. New tasks were added mid-sprint
  2. STOP adding tasks to the current sprint
  3. Put new items in backlog for next sprint
  4. Exception: critical bugs can be added (but remove a COULD to compensate)
```

---

## Sprint Anti-Patterns

Recognize and avoid these common sprint failures.

### Anti-Pattern 1: No Goal

```
Symptom:  Sprint is a grab-bag of unrelated tasks
Problem:  No way to know if the sprint "succeeded"
Fix:      Always write a sprint goal before selecting tasks
          The goal filters which tasks belong in THIS sprint
```

### Anti-Pattern 2: Overcommit

```
Symptom:  Sprint ends with 40%+ tasks carried over
Problem:  Planning ignores velocity factor, or estimates are optimistic
Fix:      Use rolling velocity average, leave 20% buffer
          Better to finish early and pull more than to fail
```

### Anti-Pattern 3: Scope Creep

```
Symptom:  New tasks keep appearing mid-sprint, burndown goes up
Problem:  No discipline around sprint boundary
Fix:      New requests go to BACKLOG, not current sprint
          Only exception: production-breaking bugs
          If adding a task, remove one of equal size
```

### Anti-Pattern 4: No Retrospective

```
Symptom:  Same problems repeat sprint after sprint
Problem:  No feedback loop, no learning
Fix:      Spend 2 minutes at sprint end on the 3 questions
          Pick ONE concrete change for next sprint
          Actually implement that change
```

### Anti-Pattern 5: Task Too Big

```
Symptom:  A single task stays "in progress" for the entire sprint
Problem:  Task was not decomposed enough (>30 min)
Fix:      If a task isn't done in 30 min, STOP
          Split it into smaller tasks right now
          Mark the original as the parent, add subtasks
```

### Anti-Pattern 6: Dependency Chains

```
Symptom:  Most tasks are blocked, only 1-2 can be worked on at a time
Problem:  Tasks were decomposed vertically (by layer) not horizontally (by feature)
Fix:      Decompose by user-facing slice, not by technical layer
          "Add search UI" + "Add search API" + "Wire together"
          NOT "Build all APIs" then "Build all UI" then "Wire all"
```

### Anti-Pattern 7: Ignoring Blockers

```
Symptom:  Working around a problem instead of solving it
Problem:  Blocker will compound and cause bigger issues later
Fix:      When blocked, immediately:
          1. Document the blocker
          2. Timebox 10 min to resolve
          3. If not resolved, switch tasks AND escalate
          Never silently work around a blocker
```

---

## Sprint Cadence Quick Reference

| Session Type | Planning | Standup | Review | Retro |
|---|---|---|---|---|
| Single session (<2h) | 5 min | Skip | 2 min | 1 min |
| Half-day (2-4h) | 10 min | Skip | 5 min | 2 min |
| Full day (4-8h) | 15 min | Midday | 10 min | 5 min |
| Multi-day | 15 min | Each session start | End of last session | End of last session |

**Rule of thumb:** Planning + review overhead should be less than 10% of sprint time.
