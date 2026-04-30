# Project Levels — Detailed Reference

The level system scales process rigor to match project complexity. Under-process leads to missed requirements and rework. Over-process leads to wasted effort and frustration. The goal is exactly the right amount of structure.

---

## Level Definitions

### L0: Single Change / Fix

**Scope**: A single, isolated change with no architectural implications.

**Characteristics**:
- Touches 1 file (or 2 if updating a corresponding test)
- The change is obvious — no design decision required
- No new data models, no new IPC channels, no new components
- Risk is low — a mistake is easy to spot and easy to revert

**Required Phases**: Implementation only (skip Analysis, Planning, Solutioning)

**Required Artifacts**: None

**Required Gates**: Gate 4 only (tests pass, TypeScript clean, no debug artifacts)

**Time Estimate**: 5-30 minutes

**Examples**:
- Fix a typo in a label or error message
- Update a constant or configuration value
- Fix an off-by-one error in a calculation
- Add a missing null check
- Update a dependency version
- Fix a CSS alignment issue
- Correct a SQL query that returns wrong results

**Not L0 (common mistakes)**:
- "Just add a button" — if the button triggers new logic, it is L1+
- "Quick database fix" — if it changes the schema, it is L2+
- "Simple refactor" — if it touches multiple files, it is L1+

---

### L1: Small Feature

**Scope**: A small, well-defined feature that touches 1-3 files with a clear implementation path.

**Characteristics**:
- Touches 1-3 files
- The implementation path is clear — you know what to do before starting
- May add a small UI element, a new query, or a new IPC handler
- Follows existing patterns in the codebase
- No new subsystems or architectural patterns introduced

**Required Phases**: Analysis (lightweight) + Implementation

**Required Artifacts**: Lightweight tech spec (can be inline, not a separate document)

**Required Gates**: Gate 1 (simplified) + Gate 4

**Time Estimate**: 30 minutes - 2 hours

**Examples**:
- Add a sort option to an existing list
- Add a new column to an existing table with IPC handler
- Add a keyboard shortcut for an existing action
- Create a simple modal dialog using existing patterns
- Add input validation to an existing form
- Add a new menu item that calls existing functionality
- Export existing data in a new format (e.g., CSV)

**Tech Spec Format (L1)**:
```
TECH SPEC: [Feature Name]
- What: [1-2 sentences]
- Files: [list of files to modify]
- Approach: [how to implement, following which existing pattern]
- Tests: [what to test]
- Risks: [anything that could go wrong]
```

---

### L2: Medium Feature

**Scope**: A feature that introduces new UI components, data models, or integration points across 4-10 files.

**Characteristics**:
- Touches 4-10 files
- Requires design decisions (how should data flow? what pattern to use?)
- Introduces at least one new concept (component, model, IPC channel, query)
- May require new database tables or columns
- Needs coordination between UI, logic, and data layers

**Required Phases**: All 4 (Analysis, Planning, Solutioning, Implementation)

**Required Artifacts**: PRD (with requirement IDs), Architecture document

**Required Gates**: All 4 gates

**Time Estimate**: 2-8 hours (1-2 sprints)

**Examples**:
- Add a complete CRUD feature (list, create, edit, delete) for a new entity
- Build a dashboard with multiple data visualizations
- Add a search feature with filters, sorting, and pagination
- Implement an import/export feature with format validation
- Add a notification system with different notification types
- Create a settings page with multiple configuration sections
- Build a report generator with customizable parameters

**PRD Sections Required (L2)**:
- Overview and objectives
- Functional requirements with IDs (FR-001, FR-002...)
- Non-functional requirements with IDs (NFR-001, NFR-002...)
- Acceptance criteria per requirement
- Dependencies
- Success metrics

---

### L3: Large Feature

**Scope**: A feature that spans multiple subsystems and requires coordinated changes across 10-30 files.

**Characteristics**:
- Touches 10-30 files
- Crosses multiple subsystems (e.g., database + IPC + state + UI)
- Requires sprint planning — cannot be done in one sitting
- Has meaningful risk if done wrong (data integrity, performance, UX)
- May require database migrations
- Multiple team members would benefit from the plan

**Required Phases**: All 4 phases + Sprint planning

**Required Artifacts**: PRD, Architecture document, Sprint plan, User stories

**Required Gates**: All 4 gates (full rigor)

**Time Estimate**: 1-3 days (2-4 sprints)

**Examples**:
- Add multi-location support (database schema, queries, UI, sync)
- Implement a customer loyalty program (points, tiers, redemption, reports)
- Build an inventory management module with barcode scanning
- Add user roles and permissions system
- Implement automated ordering based on inventory levels
- Build a  integration with payment processing
- Create a comprehensive analytics dashboard with drill-down

**Sprint Planning Required (L3)**:
- Break into 2-4 sprints
- Each sprint touches distinct files (no file overlap)
- Each sprint produces a testable increment
- Parallel execution where possible via subagents

---

### L4: System-Level

**Scope**: A system-level change, rewrite, or new product that touches 30+ files.

**Characteristics**:
- Touches 30+ files (or creates a new system entirely)
- Full SDLC required — no shortcuts
- Multi-sprint, possibly multi-week effort
- Architectural decisions have long-term consequences
- Failure risk is significant (data loss, security breach, complete breakage)
- Requires detailed documentation for future maintenance

**Required Phases**: Full SDLC — all phases with maximum rigor

**Required Artifacts**: Product brief, PRD, Architecture document, Sprint plans, User stories, Integration test plan

**Required Gates**: All 4 gates (mandatory, no conditions skippable)

**Time Estimate**: 1-4 weeks (4+ sprints)

**Examples**:
- Build a new application from scratch
- Migrate from one database to another
- Rewrite the authentication/authorization system
- Major version upgrade of a core framework (e.g., React 17 → 18)
- Implement end-to-end encryption across the application
- Build a plugin/extension system
- Create a multi-tenant architecture

---

## Level Assessment Decision Tree

```
START
  │
  ├─ Is it a bug fix, typo, or config change?
  │   └─ YES → L0
  │
  ├─ Does it touch ≤3 files with a clear path?
  │   └─ YES → L1
  │
  ├─ Does it introduce new data models or components?
  │   ├─ YES, touching 4-10 files → L2
  │   ├─ YES, touching 10-30 files → L3
  │   └─ YES, touching 30+ files → L4
  │
  ├─ Does it cross process boundaries (main/renderer)?
  │   └─ YES → Add 1 level (minimum L2)
  │
  ├─ Does it involve data migration?
  │   └─ YES → Minimum L3
  │
  ├─ Does it touch security (auth, encryption, tokens)?
  │   └─ YES → Minimum L2
  │
  └─ Uncertain?
      └─ Start 1 level higher, descale if warranted
```

---

## Required vs Optional Matrix

| Artifact/Activity | L0 | L1 | L2 | L3 | L4 |
|-------------------|:--:|:--:|:--:|:--:|:--:|
| Analysis phase | - | lite | full | full | full |
| Planning phase | - | - | full | full | full |
| Solutioning phase | - | - | full | full | full |
| Implementation phase | full | full | full | full | full |
| Product Brief | - | - | optional | required | required |
| PRD | - | - | required | required | required |
| Tech Spec | - | inline | - | - | - |
| Architecture Doc | - | - | required | required | required |
| Sprint Plan | - | - | optional | required | required |
| User Stories | - | optional | required | required | required |
| Integration Test Plan | - | - | optional | optional | required |
| Gate 1 (Analysis→Planning) | skip | lite | full | full | full |
| Gate 2 (Planning→Solutioning) | skip | skip | full | full | full |
| Gate 3 (Solutioning→Implementation) | skip | skip | full | full | full |
| Gate 4 (Implementation→Done) | lite | full | full | full | full |
| Self-Verification Protocol | optional | required | required | required | required |
| YAML Status Persistence | - | optional | required | required | required |

---

## Time Estimation Guide

These are estimates for a single developer with AI assistance. Actual time varies by domain familiarity, codebase complexity, and requirement clarity.

| Level | Planning Time | Implementation Time | Testing Time | Total |
|-------|---------------|---------------------|--------------|-------|
| L0 | 0 min | 5-20 min | 5-10 min | 10-30 min |
| L1 | 10-20 min | 20-60 min | 10-30 min | 30 min - 2 hr |
| L2 | 30-90 min | 1-4 hr | 30-90 min | 2-8 hr |
| L3 | 1-3 hr | 4-16 hr | 2-6 hr | 1-3 days |
| L4 | 3-8 hr | 16-80 hr | 8-24 hr | 1-4 weeks |

### Estimation Heuristics

- **File count is the best complexity proxy.** Count affected files during analysis.
- **New concepts multiply time.** Each new pattern, library, or subsystem adds 30-50% to the estimate.
- **Cross-boundary work is slower.** Changes spanning main/renderer/preload take 2x longer than single-process changes.
- **Data migrations are inherently risky.** Budget 2x your estimate for data-touching work.
- **Unfamiliar code is slower.** If you have never touched the module before, add 50% for learning time.

---

## Level Reassessment

Levels can change during the project. Common triggers:

### Escalation (Level Goes Up)

- Discovery reveals more files are affected than initially estimated
- A hidden dependency is found (e.g., changing a shared utility)
- Security implications are discovered
- Data migration is required
- User adds new requirements

### De-escalation (Level Goes Down)

- Analysis reveals the change is simpler than expected
- An existing pattern can be reused directly (no new architecture needed)
- Requirements are narrowed during planning
- A library solves most of the problem

### Process

1. Document the reason for reassessment
2. Update `workflow-status.yaml` with the new level
3. Add or remove required artifacts accordingly
4. Inform the user of the level change and its implications
5. Do not restart completed phases — only adjust future phases
