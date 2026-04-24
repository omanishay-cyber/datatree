# Story Sizing Guide — Complete Reference

Comprehensive guide to Fibonacci story points, anchor stories, planning poker protocol,
T-shirt sizing, epic estimation, and practical examples for Electron + React + TypeScript
projects.

---

## Fibonacci Story Points — Detailed Guide

### Why Fibonacci?

The Fibonacci sequence (1, 2, 3, 5, 8, 13, 21) forces estimation into discrete buckets
that grow proportionally. This reflects a fundamental truth: the larger the task, the
less precise your estimate. The gaps between numbers grow, preventing false precision
on large items.

**Key principle**: Story points measure RELATIVE COMPLEXITY, not hours. A 5-point story
is roughly 2.5x more complex than a 2-point story. It does NOT mean "5 hours."

### Point Definitions

#### 1 Point — Trivial
- **Effort**: Less than 2 hours
- **Scope**: Single file change, no logic changes
- **Risk**: Zero unknowns
- **Examples**:
  - Fix a typo in UI text
  - Update a color constant in theme config
  - Add a new entry to an existing config object
  - Update a dependency version (non-breaking)
  - Change a button label

#### 2 Points — Simple
- **Effort**: 2-4 hours
- **Scope**: 1-2 files, minor logic
- **Risk**: No unknowns, well-understood pattern
- **Examples**:
  - Add a new field to an existing form
  - Create a simple display-only component
  - Add input validation to an existing form field
  - Write a utility function with clear requirements
  - Add a new column to an existing table component

#### 3 Points — Moderate
- **Effort**: 4-8 hours (1 day)
- **Scope**: 2-3 files, standard business logic
- **Risk**: Minimal unknowns, may need minor research
- **Examples**:
  - Build a complete CRUD form component
  - Implement a search/filter feature on a list
  - Add a new IPC channel with handler and renderer call
  - Create a new Zustand store slice with actions
  - Implement a data export feature (CSV/Excel)

#### 5 Points — Complex
- **Effort**: 1-2 days
- **Scope**: 3-5 files, crosses component boundaries
- **Risk**: Some unknowns, requires design decisions
- **Examples**:
  - Build a multi-step wizard component
  - Implement drag-and-drop reordering
  - Add real-time data sync between two views
  - Create a charting/visualization component with dynamic data
  - Implement keyboard shortcut system across the app

#### 8 Points — Very Complex
- **Effort**: 2-3 days
- **Scope**: 5+ files, cross-cutting concern
- **Risk**: Significant unknowns, architectural decisions
- **Examples**:
  - Build a complete notification system (UI + state + persistence)
  - Implement offline-first data sync with conflict resolution
  - Create a plugin/extension architecture
  - Build a comprehensive reporting module
  - Implement role-based access control across the app

#### 13+ Points — MUST DECOMPOSE
- **Any story at 13 or above is too large to estimate accurately**
- **Action**: Break into multiple stories of 8 or fewer points
- **Expect**: Sum of sub-stories will exceed the original number (this is correct)
- **The extra points represent hidden complexity you exposed by decomposing**

---

## Anchor Stories (Reference Stories for Calibration)

Anchor stories are completed stories that serve as reference points for future estimation.
Every team should maintain 5-6 anchor stories across the point spectrum.

### How to Select Anchor Stories

1. Choose stories that the whole team worked on or reviewed
2. Pick stories where the final effort matched the estimate
3. Ensure at least one anchor per point value (1, 2, 3, 5, 8)
4. Update anchors every 5-10 sprints as the team evolves

### Example Anchor Stories (Electron + React + TypeScript)

```
ANCHOR STORIES

1 Point — "Update theme color variables"
  Changed 3 CSS custom properties in theme.ts.
  No logic changes. No testing needed beyond visual check.
  Completed in 45 minutes.

2 Points — "Add phone number field to customer form"
  Added input to CustomerForm.tsx with mask formatting.
  Added validation in customer-schema.ts.
  Completed in 3 hours including tests.

3 Points — "Implement product search with debounced filtering"
  Created SearchInput component with useDebounce hook.
  Added filter logic in product-store.ts.
  Updated ProductList to consume filtered results.
  Completed in 6 hours including tests.

5 Points — "Build invoice PDF generation"
  Created InvoiceTemplate component for PDF layout.
  Added pdf-generation.ts service using jsPDF.
  Wired IPC channel for main process file save dialog.
  Added Zustand action for invoice state.
  Updated InvoiceView with "Export PDF" button.
  Completed in 1.5 days including tests.

8 Points — "Implement data backup and restore system"
  Created BackupService in main process with sql.js dump.
  Added encryption layer for backup files.
  Built restore flow with validation and rollback.
  Created BackupSettings UI with schedule configuration.
  Added IPC channels for backup/restore/schedule operations.
  Created progress tracking with Electron notifications.
  Completed in 2.5 days including tests.
```

### Using Anchors During Estimation

When sizing a new story:
1. Read the story requirements carefully
2. Compare to each anchor story: "Is this more or less complex than our 3-point anchor?"
3. Find the closest match and assign that point value
4. If between two anchors, round UP (uncertainty is always underestimated)

---

## Planning Poker Protocol

### Setup
- Each participant gets cards: 1, 2, 3, 5, 8, 13, ? (need more info), coffee (break)
- Product owner or scrum master reads the story
- Anchor stories are visible for reference

### Round 1: Silent Estimation
1. Read the story aloud (2 minutes max)
2. Brief Q&A for clarification only, no sizing discussion (3 minutes max)
3. Everyone selects their card face-down
4. All cards revealed simultaneously (prevents anchoring)
5. Note the spread (highest and lowest values)

### Convergence
- **If all cards match**: That is the estimate. Move on.
- **If spread is 1 step** (e.g., 3 and 5): Brief discussion, re-vote, usually converges
- **If spread is 2+ steps** (e.g., 2 and 8):
  - Highest and lowest explain their reasoning (2 minutes each)
  - Group discussion (5 minutes max)
  - Re-vote
  - If still divergent after 2 rounds: take the higher value

### Rules
- No talking during card selection
- Product owner does NOT vote (they explain, not estimate)
- Scrum master facilitates but may vote if they are also developing
- Time-box: 5 minutes per story maximum. If no consensus, take the higher number.
- If anyone plays "?" card: the story needs more definition. Send it back.

### Common Planning Poker Mistakes
- **Discussing before voting**: Creates anchoring bias. Vote first, discuss after.
- **Averaging**: Never average votes. 3 and 8 does NOT equal 5. The disagreement means
  the story is misunderstood — discuss and re-vote.
- **Peer pressure**: Junior developers defer to seniors. Emphasize that all votes are equal.
- **Over-debating**: If 2 rounds do not converge, take the higher number and move on.

---

## T-Shirt Sizing to Points Mapping

T-shirt sizing is useful for quick, rough estimation during backlog grooming. Convert
to points when precision is needed.

| T-Shirt Size | Story Points | Effort | Description |
|---|---|---|---|
| **XS** | 1 | < 2 hours | Trivial change, no risk |
| **S** | 2 | 2-4 hours | Simple change, low risk |
| **M** | 3 | 4-8 hours | Standard work, known patterns |
| **L** | 5 | 1-2 days | Complex, crosses boundaries |
| **XL** | 8 | 2-3 days | Very complex, unknowns |
| **XXL** | 13+ | > 3 days | MUST decompose |

### When to Use T-Shirt Sizing

- Backlog grooming sessions (rapid sorting)
- Roadmap planning (rough capacity estimates)
- New product discovery (too early for precise points)
- Stakeholder communication (non-technical audience)

### When to Convert to Points

- Sprint planning (need precision for commitment)
- Velocity tracking (points enable metrics)
- Release planning (need to calculate sprint count)

---

## Epic Estimation

Epics are large bodies of work spanning multiple sprints. Estimate them differently.

### Bottom-Up Estimation

1. Break the epic into stories
2. Size each story using Fibonacci points
3. Sum all story points
4. Add 30% buffer for undiscovered stories

```
Epic: "User Management System"

Stories:
  - User registration form: 5 points
  - Email verification flow: 5 points
  - Login/logout: 3 points
  - Password reset: 5 points
  - User profile page: 3 points
  - Role management: 8 points
  - User list with search: 5 points
  - User deactivation: 3 points
  - Audit logging: 5 points
  - Integration testing: 8 points

Sum: 50 points
Buffer (30%): 15 points
Epic Estimate: 65 points

At velocity 24 points/sprint: ~2.7 sprints = 3 sprints
```

### Top-Down Estimation (When Stories Are Not Yet Defined)

1. Compare to a completed epic of known size
2. Assess relative complexity using the complexity matrix
3. Apply a scaling factor
4. Add 40% buffer (higher than bottom-up because less is known)

```
Reference Epic: "Product Catalog" — completed in 55 points over 3 sprints
New Epic: "Order Management" — assessed as 1.5x more complex

Estimate: 55 x 1.5 = 82.5 points
Buffer (40%): 33 points
Epic Estimate: 115 points = ~5 sprints
```

---

## Feature-Level Estimation

Features are collections of related stories within an epic. They are smaller than epics
but larger than individual stories.

### Feature Estimation Template

```
FEATURE ESTIMATE: [Feature Name]

STORIES:
| # | Story | Points | Risk | Notes |
|---|-------|--------|------|-------|
| 1 | [Story A] | 3 | Low | Known pattern |
| 2 | [Story B] | 5 | Medium | Some unknowns |
| 3 | [Story C] | 8 | High | Needs spike |
| 4 | [Story D] | 3 | Low | Depends on B |

SUBTOTAL: 19 points
INTEGRATION BUFFER (25%): 5 points
TOTAL: 24 points

DEPENDENCIES:
  - Story D depends on Story B
  - Feature requires API v2 endpoint (external team)

RISKS:
  - Story C spike may reveal additional stories
  - API v2 timeline uncertain

CONFIDENCE: 65% — spike results may change total significantly
```

---

## Electron + React + TypeScript Project Examples

Practical sizing examples for the tech stack used in your Electron project and similar apps.

### Frontend-Only Changes

| Task | Points | Rationale |
|---|---|---|
| Add tooltip to existing button | 1 | Single component, CSS only |
| New display card component | 2 | New component, props, styling |
| Form with validation (4-6 fields) | 3 | Component + validation logic + state |
| Data table with sort/filter/pagination | 5 | Complex component + state + performance |
| Drag-and-drop kanban board | 8 | Multi-component + state + edge cases |

### Full-Stack Changes (Renderer + Main Process)

| Task | Points | Rationale |
|---|---|---|
| Read config from file system | 2 | IPC channel + fs.readFile + store update |
| CRUD for single entity | 5 | UI + IPC + sql.js queries + validation |
| File import with parsing | 5 | UI + IPC + parser + error handling |
| Real-time file watcher with UI updates | 8 | Chokidar + IPC + debounce + state sync |
| Database migration system | 8 | Schema versioning + migration runner + rollback |

### Infrastructure / DevOps

| Task | Points | Rationale |
|---|---|---|
| Update Electron to next major | 3 | Config changes + testing for breakage |
| Add auto-update system | 8 | electron-updater + signing + CDN + rollback |
| Set up CI/CD pipeline | 5 | GitHub Actions + build + test + package |
| Implement crash reporting | 5 | Error boundary + main process handler + reporting |

### Testing

| Task | Points | Rationale |
|---|---|---|
| Unit tests for utility module | 2 | Tests only, no production code |
| Integration tests for IPC channel | 3 | Mock setup + test scenarios |
| E2E test suite for critical flow | 8 | Playwright setup + page objects + CI integration |
| Performance benchmark suite | 5 | Benchmark runner + metrics + baseline |

---

## Estimation Calibration Exercise

Run this exercise quarterly to keep estimates sharp:

1. Select 10 stories completed in the last 3 sprints
2. For each story, record: estimated points, actual effort (hours), actual complexity
3. Calculate accuracy: `Accuracy = 1 - |Estimated - Actual| / Actual`
4. Average the accuracy across all 10 stories
5. Identify patterns: Are you consistently over or under for certain types?

```
| Story | Estimated | Actual Hours | Expected Hours (3pts=6h) | Accuracy |
|-------|-----------|-------------|--------------------------|----------|
| S-101 | 3 pts     | 5 hours     | 6 hours                  | 83%      |
| S-102 | 5 pts     | 14 hours    | 12 hours                 | 83%      |
| S-103 | 2 pts     | 2 hours     | 4 hours                  | 50%      |
| S-104 | 8 pts     | 20 hours    | 18 hours                 | 89%      |
| S-105 | 3 pts     | 8 hours     | 6 hours                  | 67%      |

Average accuracy: 74%
Pattern: Consistently under-estimating 3-point stories.
Action: Review 3-point anchor story. May need to re-calibrate.
```
