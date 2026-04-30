# Workflow Commands — Detailed Reference

Each workflow command orchestrates a specific phase or artifact of the project lifecycle. Commands are invoked as slash commands and produce structured outputs.

---

## /workflow-init

**Purpose**: Initialize the project workflow by assessing the project level and setting up tracking.

**Available at**: All levels

**Input**: Description of the task, feature, or project

**Process**:
1. Read the task description
2. Assess the project level using the decision tree (see `project-levels.md`)
3. Create `docs/workflow-status.yaml` with initial state
4. Determine required phases, artifacts, and gates
5. Present the assessment to the user for confirmation

**Output**:
```
WORKFLOW INITIALIZED
━━━━━━━━━━━━━━━━━━━
Project: [Name]
Level: L[0-4] — [Level Name]
Reason: [Why this level was chosen]

Required Phases:
  [x] Analysis     [required/skip]
  [x] Planning     [required/skip]
  [x] Solutioning  [required/skip]
  [x] Implementation [required]

Required Artifacts:
  - [List of required documents]

Required Gates:
  - [List of mandatory gates]

Estimated Time: [Range]

Next Step: [What to do first]
```

**Example**:
```
WORKFLOW INITIALIZED
━━━━━━━━━━━━━━━━━━━
Project: Supplier Filter for Inventory
Level: L2 — Medium Feature
Reason: New UI component + new IPC handler + new SQL query = 6 files affected

Required Phases:
  [x] Analysis     required
  [x] Planning     required
  [x] Solutioning  required
  [x] Implementation required

Required Artifacts:
  - PRD with requirement IDs
  - Architecture document

Required Gates:
  - Gate 1: Analysis → Planning
  - Gate 2: Planning → Solutioning
  - Gate 3: Solutioning → Implementation
  - Gate 4: Implementation → Done

Estimated Time: 3-6 hours

Next Step: Begin Analysis phase — understand current inventory page structure
```

---

## /workflow-status

**Purpose**: Check current project progress and recommend the next action.

**Available at**: All levels

**Input**: None (reads from `docs/workflow-status.yaml`)

**Process**:
1. Read `docs/workflow-status.yaml`
2. Determine current phase and progress
3. Check for incomplete items
4. Recommend the next action

**Output**:
```
WORKFLOW STATUS
━━━━━━━━━━━━━━
Project: [Name]
Level: L[0-4]
Current Phase: [Phase name]

Phase Progress:
  [x] Analysis     ✓ completed [date]
  [>] Planning     in progress
  [ ] Solutioning  pending
  [ ] Implementation pending

Current Phase Details:
  - Requirements defined: 5/8
  - Acceptance criteria written: 3/5
  - Gate 2 readiness: 60%

Blockers: [Any blocking issues]

Recommended Next Step: [Specific action to take]
```

---

## /product-brief

**Purpose**: Create a discovery document that captures the project vision, stakeholders, and high-level scope before detailed requirements.

**Available at**: L2+

**Input**: Project context from Analysis phase

**Process**:
1. Verify Analysis phase is complete (Gate 1 passed)
2. Gather information from analysis summary
3. Generate product brief using template
4. Present to user for review

**Output**: A structured document with these sections:

```markdown
# Product Brief: [Project Name]

## Vision
[1-2 sentences describing the end state]

## Problem Statement
[What problem this solves and for whom]

## Stakeholders
| Stakeholder | Role | Interest |
|-------------|------|----------|
| [Name/Type] | [Role] | [What they care about] |

## Scope
### In Scope
- [Item 1]
- [Item 2]

### Out of Scope
- [Item 1]
- [Item 2]

## Success Criteria
- [Measurable outcome 1]
- [Measurable outcome 2]

## Constraints
- [Technical constraint]
- [Business constraint]
- [Timeline constraint]

## Assumptions
- [Assumption 1]
- [Assumption 2]

## Risks
| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| [Risk 1] | High/Med/Low | High/Med/Low | [Strategy] |

## Timeline
- Phase 1: [Date range] — [Milestone]
- Phase 2: [Date range] — [Milestone]
```

---

## /prd

**Purpose**: Create a Product Requirements Document with traceable, testable requirements.

**Available at**: L2+

**Input**: Product brief (if exists) + Analysis findings

**Process**:
1. Verify Analysis phase complete
2. Generate functional requirements with unique IDs (FR-001, FR-002...)
3. Generate non-functional requirements with unique IDs (NFR-001, NFR-002...)
4. Assign MoSCoW priorities to each requirement
5. Write acceptance criteria for each requirement
6. Define dependencies and success metrics
7. Present to user for review and approval

**Output**: See `templates.md` for the complete PRD template.

**Example Functional Requirement**:
```markdown
### FR-003: Supplier Filter Dropdown
**Priority**: Must-have
**Description**: The inventory list page shall display a dropdown filter
that allows users to filter products by supplier name.

**Acceptance Criteria**:
1. Dropdown displays all unique supplier names from the database
2. Selecting a supplier filters the inventory list to show only that supplier's products
3. Selecting "All Suppliers" removes the filter and shows all products
4. Filter state persists during the session but resets on page reload
5. Dropdown is keyboard-accessible (Tab to focus, Enter to select, Escape to close)
```

**Example Non-Functional Requirement**:
```markdown
### NFR-001: Filter Response Time
**Priority**: Must-have
**Description**: Filtering the inventory list shall complete within 200ms
for datasets up to 10,000 products.

**Acceptance Criteria**:
1. Filter operation completes in <200ms measured from click to render
2. No visible UI jank during filtering
3. Loading indicator shown if operation exceeds 100ms
```

---

## /tech-spec

**Purpose**: Create a lightweight technical specification for small features.

**Available at**: L0-L1

**Input**: Feature description

**Process**:
1. Understand the feature request
2. Identify affected files
3. Determine implementation approach
4. Define test strategy
5. Present spec for user confirmation

**Output**:
```markdown
# Tech Spec: [Feature Name]

## Summary
[1-2 sentences describing what will be built]

## Files Affected
| File | Change Type | Description |
|------|-------------|-------------|
| [path] | modify | [what changes] |
| [path] | create | [what is created] |

## Approach
[How to implement, referencing existing patterns]

## Implementation Steps
1. [Step 1 — specific file and change]
2. [Step 2 — specific file and change]
3. [Step 3 — test]

## Test Plan
- [Test 1: what to verify]
- [Test 2: what to verify]

## Risks
- [Risk and mitigation]
```

---

## /architecture

**Purpose**: Design the system architecture for a feature or project.

**Available at**: L2+

**Input**: PRD (with requirement IDs) + Analysis findings

**Process**:
1. Verify Planning phase complete (Gate 2 passed)
2. Design system structure based on requirements
3. Define component boundaries and interfaces
4. Map NFRs to architectural strategies
5. Analyze trade-offs for key decisions
6. Assess risks
7. Present to user for review

**Output**: See `templates.md` for the complete architecture document template (10 sections).

**Key Sections**:
1. Overview and context
2. Architecture principles
3. System structure (component diagram)
4. Component details (responsibilities, interfaces)
5. Data model
6. Data flow
7. NFR strategy mapping
8. Technology choices with justification
9. Trade-off analysis
10. Risk assessment

---

## /sprint-planning

**Purpose**: Break implementation work into sprint iterations.

**Available at**: L2+

**Input**: Architecture document + User stories

**Process**:
1. Verify Solutioning phase complete (Gate 3 passed)
2. Group stories into logical sprints
3. Ensure no file overlap between sprints
4. Identify parallel execution opportunities
5. Estimate sprint durations
6. Define sprint verification criteria

**Output**:
```markdown
# Sprint Plan: [Project Name]

## Sprint Overview
| Sprint | Focus | Stories | Estimated Time | Dependencies |
|--------|-------|---------|----------------|--------------|
| Sprint 1 | [Focus area] | US-001, US-002 | [Time] | None |
| Sprint 2 | [Focus area] | US-003, US-004 | [Time] | Sprint 1 |
| Sprint 3 | [Focus area] | US-005, US-006 | [Time] | None |

## Sprint 1: [Name]
**Files**: [list of files ONLY this sprint touches]
**Stories**:
- US-001: [title] — [estimate]
- US-002: [title] — [estimate]

**Verification**:
- [ ] [How to confirm sprint 1 succeeded]

**Parallel Execution**: Can run alongside Sprint 3

## Sprint 2: [Name]
**Files**: [list of files ONLY this sprint touches]
**Stories**:
- US-003: [title] — [estimate]
- US-004: [title] — [estimate]

**Dependencies**: Sprint 1 must complete first (depends on [component])

**Verification**:
- [ ] [How to confirm sprint 2 succeeded]

## Parallel Dispatch Plan
```
Agent 1: Sprint 1 — [files A, B]
Agent 3: Sprint 3 — [files E, F]
--- wait for Sprint 1 ---
Agent 2: Sprint 2 — [files C, D]
--- final integration ---
Agent 4: Integration verification
```
```

---

## /create-story

**Purpose**: Create a user story with acceptance criteria and estimates.

**Available at**: L1+

**Input**: Requirement ID (e.g., FR-003) or feature description

**Process**:
1. Reference the requirement from the PRD (if exists)
2. Write the user story in standard format
3. Define acceptance criteria
4. Estimate complexity (story points: 1, 2, 3, 5, 8)
5. Identify files affected
6. Update `workflow-status.yaml`

**Output**:
```markdown
## US-[NNN]: [Story Title]

**Requirement**: FR-[NNN]
**Sprint**: [Sprint number]
**Estimate**: [1/2/3/5/8] story points

**Story**:
As a [role],
I want to [action],
So that [benefit].

**Acceptance Criteria**:
1. Given [precondition], when [action], then [result]
2. Given [precondition], when [action], then [result]
3. Given [precondition], when [action], then [result]

**Files Affected**:
- [path/to/file.ts] — [what changes]
- [path/to/file.tsx] — [what changes]

**Test Plan**:
- [ ] [Test case 1]
- [ ] [Test case 2]
- [ ] [Test case 3]

**Notes**:
[Any implementation notes, edge cases, or considerations]
```

**Story Point Reference**:
| Points | Complexity | Typical Scope |
|--------|------------|---------------|
| 1 | Trivial | Single file, obvious change |
| 2 | Simple | 2-3 files, clear path |
| 3 | Medium | 3-5 files, some design needed |
| 5 | Complex | 5-8 files, multiple concerns |
| 8 | Very Complex | 8+ files, significant design |

---

## /dev-story

**Purpose**: Implement a specific story using TDD methodology.

**Available at**: L1+

**Input**: Story ID (e.g., US-001)

**Process**:
1. Read the story from `workflow-status.yaml` or PRD
2. Review acceptance criteria
3. Write failing tests first (Red)
4. Implement the minimum code to pass tests (Green)
5. Refactor for quality (Refactor)
6. Run the 6-step self-verification protocol
7. Update story status in `workflow-status.yaml`

**Execution Flow**:
```
1. READ story and acceptance criteria
2. WRITE test files (tests should fail — Red phase)
3. RUN tests — confirm they fail for the right reason
4. IMPLEMENT code to pass tests (Green phase)
5. RUN tests — confirm they pass
6. REFACTOR — improve code quality without changing behavior
7. RUN tests — confirm they still pass
8. SELF-VERIFY — execute 6-step protocol
9. UPDATE workflow-status.yaml — mark story as done
10. REPORT — summary of what was built and verified
```

**Output**:
```
STORY COMPLETE: US-001
━━━━━━━━━━━━━━━━━━━━
Title: [Story title]
Status: Done

Implementation:
- [File 1]: [What was changed]
- [File 2]: [What was changed]

Tests:
- [X] [Test 1] — passing
- [X] [Test 2] — passing
- [X] [Test 3] — passing
Coverage: [X]%

Self-Verification:
- [X] Staged changes reviewed
- [X] Completeness verified
- [X] Quality standards met
- [X] Tests verified
- [X] Security checked
- [X] Documentation checked

Acceptance Criteria:
- [X] AC1: [met because...]
- [X] AC2: [met because...]
- [X] AC3: [met because...]
```

---

## Command Dependency Chain

Commands must be invoked in order. The dependency chain prevents skipping required steps.

```
/workflow-init
    │
    ├── /product-brief (L2+, optional for L2)
    │
    ├── /prd (L2+) or /tech-spec (L0-L1)
    │
    ├── /architecture (L2+)
    │
    ├── /sprint-planning (L2+)
    │       │
    │       └── /create-story (repeatable)
    │
    └── /dev-story (repeatable)
            │
            └── /workflow-status (anytime)
```

Commands earlier in the chain must complete before later commands can be invoked. Exception: `/workflow-status` can be invoked at any time.
