---
name: fireworks-taskmaster
description: >-
  Task decomposition, sprint planning, progress tracking, and project management for development sessions. Breaks features into tasks with dependencies, tracks progress with burndown, manages multi-session persistence, and integrates with GitHub Issues. Use when planning features, breaking work into tasks, tracking sprint progress, managing backlogs, or estimating capacity.
version: 1.0.0
author: mneme
tags: [task, project, sprint, backlog, planning, tracking, burndown, kanban]
triggers: [task, plan, sprint, backlog, track, progress, breakdown, decompose, prioritize, kanban]
---

# Fireworks Taskmaster

## 1. Overview

Fireworks Taskmaster is a structured task management system designed for AI-assisted development sessions. It provides a complete workflow for breaking down features into executable tasks, planning sprints, tracking progress, and persisting state across sessions.

This skill bridges the gap between high-level feature requests and concrete development steps. Every task produced by this system is small enough to complete in one focused block, has clear acceptance criteria, and fits into a trackable workflow.

**Core Principles:**
- Every piece of work is a task with a clear definition of done
- Tasks are small (max 30 minutes), concrete, and verifiable
- Dependencies are explicit — no hidden assumptions
- Progress is visible at all times via burndown and status tracking
- State persists across sessions via YAML files and GitHub Issues

**When to Invoke This Skill:**
- User says "plan", "break down", "decompose", or "what should I work on"
- Starting a new feature or fixing a complex bug
- Beginning a new development session and need to pick up where you left off
- User asks about progress, velocity, or remaining work
- Sprint planning, review, or retrospective is needed

---

## 2. Task Decomposition Protocol

### Hierarchy

```
Feature (what the user wants)
  └── Epic (major deliverable, 1-5 days)
        └── Story (user-facing value, 2-8 hours)
              └── Task (single dev action, 15-30 min)
                    └── Subtask (atomic step, 5-15 min)
```

### Task Schema

Every task MUST have these fields:

```yaml
task:
  id: T-001                          # Unique identifier
  subject: "Add dark mode toggle"    # Short, action-oriented title
  description: |                     # What needs to happen
    Implement a toggle switch in the settings panel that
    switches between light and dark theme using CSS custom
    properties.
  status: backlog                    # backlog | ready | in-progress | review | done
  priority: must                     # must | should | could | wont
  estimate: 25min                    # Time estimate (max 30min per task)
  dependencies: [T-000]             # Task IDs that must complete first
  assignee: claude                   # claude | user | pair
  type: feature                      # feature | bug | refactor | research | chore
  acceptance_criteria:               # When is this done?
    - Toggle appears in settings panel
    - Clicking toggles between light and dark
    - Preference persists in localStorage
    - Both themes pass visual check
  created: 2026-03-26
  completed: null
```

### Decomposition Rules

1. **Max 30 minutes per task.** If a task feels bigger, split it.
2. **Action-oriented subjects.** Start with a verb: Add, Fix, Refactor, Research, Update.
3. **No vague tasks.** "Improve performance" is not a task. "Add memoization to ProductList render" is.
4. **Dependencies must be explicit.** If task B needs task A's output, declare it.
5. **Every task has acceptance criteria.** At least one concrete, verifiable criterion.
6. **Estimate before starting.** If you cannot estimate, the task needs more decomposition.

### Decomposition Procedure

```
INPUT: Feature request from user

STEP 1 — Understand
  - Restate the feature in your own words
  - Identify unknowns (mark as research tasks)
  - Identify affected files/modules

STEP 2 — Break into Stories
  - Each story delivers user-facing value
  - Stories are independent when possible
  - Apply INVEST criteria (Independent, Negotiable, Valuable, Estimable, Small, Testable)

STEP 3 — Break Stories into Tasks
  - Each task is one concrete development action
  - Max 30 minutes
  - Include setup, implementation, testing, and cleanup tasks
  - Add verification task at the end of each story

STEP 4 — Identify Dependencies
  - Draw dependency graph
  - Find critical path
  - Parallelize where possible

STEP 5 — Estimate
  - Use T-shirt sizes first (XS=5min, S=15min, M=25min, L=30min)
  - If L, consider splitting further
  - Sum estimates for sprint capacity check

STEP 6 — Validate
  - Every story has at least one task
  - Every task has acceptance criteria
  - No circular dependencies
  - Total estimate fits available capacity
```

### Definition of Done — By Task Type

**Feature Task:**
- [ ] Code implements all acceptance criteria
- [ ] TypeScript compiles (`tsc --noEmit` passes)
- [ ] Works in both light and dark themes
- [ ] No console errors or warnings
- [ ] Tested manually or with unit test

**Bug Fix Task:**
- [ ] Root cause identified and documented
- [ ] Fix addresses root cause (not just symptoms)
- [ ] Regression test added or existing test updated
- [ ] Original bug no longer reproduces
- [ ] No new bugs introduced

**Refactor Task:**
- [ ] Behavior is unchanged (characterization tests pass)
- [ ] Code smell is eliminated
- [ ] TypeScript compiles cleanly
- [ ] No performance regression

**Research Task:**
- [ ] Question is answered with evidence
- [ ] Sources are cited or linked
- [ ] Recommendations are ranked by viability
- [ ] Unknowns are explicitly listed

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
