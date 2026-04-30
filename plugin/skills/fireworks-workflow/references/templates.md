# Templates — Complete Reference

Production-ready templates for every document artifact in the workflow. Copy and fill in the sections. Remove sections not applicable to your project level.

---

## Product Brief Template

```markdown
# Product Brief: [Project Name]

**Author**: [Name]
**Date**: [YYYY-MM-DD]
**Status**: Draft | Under Review | Approved
**Level**: L[2-4]

---

## 1. Vision

[1-2 sentences describing the desired end state. What does the world look like when this project is complete?]

## 2. Problem Statement

[What problem does this solve? Who experiences it? How painful is it? What is the current workaround?]

### Current State
[How things work today — the status quo]

### Desired State
[How things should work after this project — the target]

### Gap
[The difference between current and desired state — this is what the project must bridge]

## 3. Stakeholders

| Stakeholder | Role | Key Interest | Decision Authority |
|-------------|------|--------------|-------------------|
| [Name/Type] | Primary User | [What they care about] | [Approve/Inform/Consult] |
| [Name/Type] | Developer | [What they care about] | [Approve/Inform/Consult] |
| [Name/Type] | Business Owner | [What they care about] | [Approve/Inform/Consult] |

## 4. Scope

### In Scope
- [Feature/capability 1]
- [Feature/capability 2]
- [Feature/capability 3]

### Out of Scope
- [Feature/capability explicitly excluded 1]
- [Feature/capability explicitly excluded 2]
- [Feature/capability explicitly excluded 3]

### Future Considerations
- [Things that may be in scope for a future phase]

## 5. Success Criteria

| # | Metric | Target | Measurement Method |
|---|--------|--------|-------------------|
| 1 | [Metric name] | [Quantifiable target] | [How to measure] |
| 2 | [Metric name] | [Quantifiable target] | [How to measure] |
| 3 | [Metric name] | [Quantifiable target] | [How to measure] |

## 6. Constraints

### Technical Constraints
- [Constraint 1 — e.g., must work with existing database schema]
- [Constraint 2 — e.g., must support offline operation]

### Business Constraints
- [Constraint 1 — e.g., must be completed by Q2]
- [Constraint 2 — e.g., must not require user retraining]

### Compliance Constraints
- [Constraint 1 — e.g., must comply with data protection regulations]

## 7. Assumptions

- [Assumption 1 — e.g., users have modern browsers]
- [Assumption 2 — e.g., network latency is under 100ms]
- [Assumption 3 — e.g., existing APIs will not change during development]

## 8. Risks

| # | Risk | Impact | Likelihood | Mitigation |
|---|------|--------|------------|------------|
| 1 | [Risk description] | High/Med/Low | High/Med/Low | [Mitigation strategy] |
| 2 | [Risk description] | High/Med/Low | High/Med/Low | [Mitigation strategy] |
| 3 | [Risk description] | High/Med/Low | High/Med/Low | [Mitigation strategy] |

## 9. Timeline

| Phase | Date Range | Milestone | Deliverable |
|-------|------------|-----------|-------------|
| Analysis | [Start]-[End] | Problem understood | Analysis summary |
| Planning | [Start]-[End] | Requirements defined | PRD |
| Solutioning | [Start]-[End] | Architecture designed | Architecture doc |
| Implementation | [Start]-[End] | Feature complete | Working software |

## 10. Approval

| Approver | Date | Decision |
|----------|------|----------|
| [Name] | [Date] | Approved / Rejected / Deferred |
```

---

## PRD Template

```markdown
# Product Requirements Document: [Project Name]

**Author**: [Name]
**Date**: [YYYY-MM-DD]
**Version**: 1.0
**Status**: Draft | Under Review | Approved
**Level**: L[2-4]

---

## 1. Overview

### 1.1 Purpose
[Why this document exists and what it defines]

### 1.2 Background
[Context for the project — what led to this work]

### 1.3 Objectives
- [Objective 1]
- [Objective 2]
- [Objective 3]

### 1.4 Target Users
[Who will use this feature and how]

---

## 2. Functional Requirements

### FR-001: [Requirement Title]
**Priority**: Must-have | Should-have | Could-have | Won't-have
**Description**: [Detailed description of the requirement]
**Rationale**: [Why this requirement exists]

**Acceptance Criteria**:
1. Given [precondition], when [action], then [expected result]
2. Given [precondition], when [action], then [expected result]
3. Given [precondition], when [action], then [expected result]

### FR-002: [Requirement Title]
**Priority**: Must-have | Should-have | Could-have | Won't-have
**Description**: [Detailed description]
**Rationale**: [Why this requirement exists]

**Acceptance Criteria**:
1. Given [precondition], when [action], then [expected result]
2. Given [precondition], when [action], then [expected result]

[Continue for all functional requirements...]

---

## 3. Non-Functional Requirements

### NFR-001: [Requirement Title]
**Priority**: Must-have | Should-have | Could-have | Won't-have
**Category**: Performance | Security | Accessibility | Usability | Reliability
**Description**: [Detailed description]

**Acceptance Criteria**:
1. [Measurable condition]
2. [Measurable condition]

### NFR-002: [Requirement Title]
**Priority**: Must-have | Should-have | Could-have | Won't-have
**Category**: Performance | Security | Accessibility | Usability | Reliability
**Description**: [Detailed description]

**Acceptance Criteria**:
1. [Measurable condition]

[Continue for all non-functional requirements...]

---

## 4. Epics and User Stories

### Epic 1: [Epic Title]
**Description**: [What this epic delivers]

| Story ID | Title | Priority | Estimate | Sprint |
|----------|-------|----------|----------|--------|
| US-001 | [Story title] | Must-have | 3 pts | Sprint 1 |
| US-002 | [Story title] | Must-have | 2 pts | Sprint 1 |
| US-003 | [Story title] | Should-have | 5 pts | Sprint 2 |

### Epic 2: [Epic Title]
**Description**: [What this epic delivers]

| Story ID | Title | Priority | Estimate | Sprint |
|----------|-------|----------|----------|--------|
| US-004 | [Story title] | Must-have | 3 pts | Sprint 2 |
| US-005 | [Story title] | Could-have | 2 pts | Sprint 3 |

---

## 5. Dependencies

### Internal Dependencies
| Dependency | Type | Impact if Unavailable |
|------------|------|----------------------|
| [Module/component] | Blocks [FR-XXX] | [What happens] |
| [Module/component] | Required by [FR-XXX] | [What happens] |

### External Dependencies
| Dependency | Type | Owner | Status |
|------------|------|-------|--------|
| [API/Service] | Required | [Team/Vendor] | Available / Pending |
| [Library] | Required | [Open source] | [Version] |

---

## 6. Success Metrics

| # | Metric | Baseline | Target | Measurement |
|---|--------|----------|--------|-------------|
| 1 | [Metric] | [Current value] | [Target value] | [How measured] |
| 2 | [Metric] | [Current value] | [Target value] | [How measured] |
| 3 | [Metric] | [Current value] | [Target value] | [How measured] |

---

## 7. Out of Scope

- [Feature/behavior explicitly excluded]
- [Feature/behavior explicitly excluded]

---

## 8. Open Questions

| # | Question | Owner | Due Date | Resolution |
|---|----------|-------|----------|------------|
| 1 | [Question] | [Who answers] | [Date] | [Answer once resolved] |

---

## 9. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | [Date] | [Name] | Initial draft |
```

---

## Tech Spec Template (Lightweight, L0-L1)

```markdown
# Tech Spec: [Feature Name]

**Date**: [YYYY-MM-DD]
**Level**: L[0-1]

## Summary
[1-2 sentences: what will be built and why]

## Files Affected
| File | Change Type | Description |
|------|-------------|-------------|
| [path/to/file.ts] | modify | [what changes] |
| [path/to/file.tsx] | modify | [what changes] |
| [path/to/file.test.ts] | create | [what tests are added] |

## Approach
[How to implement. Reference existing patterns in the codebase.]

## Implementation Steps
1. [File: path/to/file.ts] — [specific change]
2. [File: path/to/file.tsx] — [specific change]
3. [File: path/to/file.test.ts] — [write tests]
4. Verify: [how to confirm it works]

## Test Plan
- [ ] [Test case 1]
- [ ] [Test case 2]
- [ ] [Edge case]

## Risks
- [Risk]: [mitigation]
```

---

## Architecture Document Template (10 Sections)

```markdown
# Architecture Document: [Project Name]

**Author**: [Name]
**Date**: [YYYY-MM-DD]
**Version**: 1.0
**Status**: Draft | Under Review | Approved
**Level**: L[2-4]
**PRD Reference**: [Link or file path to PRD]

---

## 1. Overview and Context

### 1.1 Purpose
[What this architecture document covers]

### 1.2 Scope
[System boundaries — what is designed here vs. what is assumed to exist]

### 1.3 Context Diagram
[High-level view of the system in its environment — what systems does it interact with?]

```
[External System A] <---> [This System] <---> [Database]
                              ^
                              |
                         [User/Client]
```

---

## 2. Architecture Principles

| # | Principle | Rationale |
|---|-----------|-----------|
| 1 | [Principle — e.g., "Separation of concerns"] | [Why this principle matters for this project] |
| 2 | [Principle — e.g., "Fail fast, recover gracefully"] | [Why] |
| 3 | [Principle — e.g., "No shared mutable state"] | [Why] |
| 4 | [Principle — e.g., "Convention over configuration"] | [Why] |

---

## 3. System Structure

### 3.1 Component Diagram

```
┌─────────────────────────────────────────────┐
│                  Renderer                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │Component A│  │Component B│  │Component C│  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  │
│       │              │              │        │
│  ┌────┴──────────────┴──────────────┴────┐  │
│  │              State Store               │  │
│  └───────────────────┬───────────────────┘  │
└──────────────────────┼──────────────────────┘
                       │ IPC
┌──────────────────────┼──────────────────────┐
│                  Main Process                │
│  ┌──────────┐  ┌────┴─────┐  ┌──────────┐  │
│  │  Handler  │  │  Service  │  │ Database  │  │
│  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────┘
```

### 3.2 Component Responsibility Matrix

| Component | Responsibility | Owns | Depends On |
|-----------|---------------|------|------------|
| [Component A] | [What it does] | [What data/state] | [What it needs] |
| [Component B] | [What it does] | [What data/state] | [What it needs] |
| [Component C] | [What it does] | [What data/state] | [What it needs] |

---

## 4. Component Details

### 4.1 [Component A]
**Responsibility**: [Single-sentence description]
**Files**: [List of files that comprise this component]

**Interface**:
```typescript
interface ComponentAProps {
  // Input interface
}

interface ComponentAOutput {
  // Output interface
}
```

**Behavior**:
- [Behavior 1]
- [Behavior 2]
- [Error handling]

### 4.2 [Component B]
[Same structure as 4.1]

### 4.3 [Component C]
[Same structure as 4.1]

---

## 5. Data Model

### 5.1 Entities

```
[Entity A]         [Entity B]         [Entity C]
├── id (PK)        ├── id (PK)        ├── id (PK)
├── field1         ├── field1         ├── field1
├── field2         ├── entityA_id (FK)├── field2
└── timestamps     └── timestamps     └── timestamps
```

### 5.2 Schema Definition

```sql
CREATE TABLE [entity_a] (
  id TEXT PRIMARY KEY,
  field1 TEXT NOT NULL,
  field2 INTEGER DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE [entity_b] (
  id TEXT PRIMARY KEY,
  field1 TEXT NOT NULL,
  entity_a_id TEXT NOT NULL REFERENCES entity_a(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 5.3 Relationships
[Describe relationships between entities — one-to-many, many-to-many, etc.]

---

## 6. Data Flow

### 6.1 Primary Flow: [Main Use Case]

```
User Action
    │
    ▼
[UI Component] ──── validate input
    │
    ▼
[State Store] ──── optimistic update
    │
    ▼
[IPC Channel] ──── invoke('channel-name', data)
    │
    ▼
[Main Handler] ──── validate + authorize
    │
    ▼
[Service Layer] ──── business logic
    │
    ▼
[Database] ──── SQL query (parameterized)
    │
    ▼
[Response] ──── back through IPC to renderer
    │
    ▼
[State Store] ──── confirm or rollback optimistic update
    │
    ▼
[UI Component] ──── re-render with new state
```

### 6.2 Error Flow
[What happens when each step fails]

---

## 7. NFR Strategy Mapping

| NFR ID | Requirement | Architectural Strategy |
|--------|-------------|----------------------|
| NFR-001 | [Response time <200ms] | [Indexed queries + optimistic UI updates] |
| NFR-002 | [Offline capability] | [Local database + sync on reconnect] |
| NFR-003 | [Accessibility AA] | [Semantic HTML + ARIA + keyboard navigation] |
| NFR-004 | [Data encryption] | [Envelope encryption for sensitive fields] |

---

## 8. Technology Choices

| Decision | Chosen | Alternatives Considered | Justification |
|----------|--------|------------------------|---------------|
| [Decision 1] | [Technology] | [Alt A, Alt B] | [Why chosen over alternatives] |
| [Decision 2] | [Technology] | [Alt A, Alt B] | [Why chosen over alternatives] |
| [Decision 3] | [Technology] | [Alt A, Alt B] | [Why chosen over alternatives] |

---

## 9. Trade-Off Analysis

### Decision: [Key Architecture Decision]

| Criterion | Option A: [Name] | Option B: [Name] |
|-----------|-------------------|-------------------|
| Complexity | [Assessment] | [Assessment] |
| Performance | [Assessment] | [Assessment] |
| Maintainability | [Assessment] | [Assessment] |
| User Experience | [Assessment] | [Assessment] |
| Risk | [Assessment] | [Assessment] |
| Alignment | [Assessment] | [Assessment] |
| **Decision** | **Chosen/Not Chosen** | **Chosen/Not Chosen** |

**Rationale**: [Why the chosen option was selected]

---

## 10. Risk Assessment

| # | Risk | Impact | Likelihood | Mitigation | Owner |
|---|------|--------|------------|------------|-------|
| 1 | [Risk description] | High/Med/Low | High/Med/Low | [Mitigation strategy] | [Who monitors] |
| 2 | [Risk description] | High/Med/Low | High/Med/Low | [Mitigation strategy] | [Who monitors] |
| 3 | [Risk description] | High/Med/Low | High/Med/Low | [Mitigation strategy] | [Who monitors] |

### Contingency Plans
- **If [Risk 1] occurs**: [What to do]
- **If [Risk 2] occurs**: [What to do]
```

---

## User Story Template

```markdown
## US-[NNN]: [Story Title]

**Epic**: [Epic name]
**Requirement**: FR-[NNN]
**Sprint**: [Sprint number]
**Estimate**: [1/2/3/5/8] story points
**Status**: To Do | In Progress | Done

### Story
As a [type of user],
I want to [perform an action],
So that [I achieve a benefit/goal].

### Acceptance Criteria
1. **Given** [precondition/context],
   **When** [action is performed],
   **Then** [expected outcome].

2. **Given** [precondition/context],
   **When** [action is performed],
   **Then** [expected outcome].

3. **Given** [precondition/context],
   **When** [action is performed],
   **Then** [expected outcome].

### Technical Notes
- [Implementation detail or constraint]
- [Performance consideration]
- [Security consideration]

### Files Affected
| File | Change Type | Description |
|------|-------------|-------------|
| [path] | modify/create | [what changes] |

### Test Cases
- [ ] [Test case 1 — happy path]
- [ ] [Test case 2 — edge case]
- [ ] [Test case 3 — error case]

### Definition of Done
- [ ] Code implemented
- [ ] Tests written and passing
- [ ] Self-verification protocol complete
- [ ] Acceptance criteria verified
```

---

## Sprint Plan Template

```markdown
# Sprint Plan: [Project Name] — Sprint [N]

**Sprint Goal**: [1 sentence describing what this sprint delivers]
**Start Date**: [YYYY-MM-DD]
**End Date**: [YYYY-MM-DD]
**Total Story Points**: [Sum of estimates]

---

## Stories

| ID | Title | Estimate | Priority | Status |
|----|-------|----------|----------|--------|
| US-[NNN] | [Story title] | [1/2/3/5/8] | Must/Should/Could | To Do |
| US-[NNN] | [Story title] | [1/2/3/5/8] | Must/Should/Could | To Do |
| US-[NNN] | [Story title] | [1/2/3/5/8] | Must/Should/Could | To Do |

## File Ownership

This sprint owns these files exclusively (no other sprint modifies them):
- [path/to/file1.ts]
- [path/to/file2.tsx]
- [path/to/file3.test.ts]

## Dependencies

| Dependency | Type | Status |
|------------|------|--------|
| Sprint [N-1] component X | Blocks US-[NNN] | Complete / Pending |
| External API Y | Required for US-[NNN] | Available / Pending |

## Execution Order

1. US-[NNN] — [why first]
2. US-[NNN] — [depends on story above]
3. US-[NNN] — [can parallelize with above]

## Verification Criteria

- [ ] All stories meet acceptance criteria
- [ ] All tests pass
- [ ] TypeScript compiles cleanly
- [ ] Integration with previous sprints verified
- [ ] Both themes checked (if UI changes)

## Parallel Dispatch Plan

```text
Agent 1: US-[NNN] — [files A, B]
Agent 2: US-[NNN] — [files C, D] (no overlap with Agent 1)
--- wait for Agent 1 ---
Agent 3: US-[NNN] — [files E] (depends on Agent 1 output)
```

## Sprint Retrospective (fill after completion)

**Completed**: [X] of [Y] story points
**Carried Over**: [Stories not completed]
**Lessons Learned**: [What went well, what to improve]
```

---

## workflow-status.yaml Template

```yaml
# Workflow Status — [Project Name]
# Auto-managed by fireworks-workflow skill
# Do not edit manually unless correcting an error

project:
  name: "[Project Name]"
  description: "[1-sentence description]"
  level: L[0-4]
  created: "[YYYY-MM-DD]"
  updated: "[YYYY-MM-DD]"

workflow:
  current_phase: "[ANALYSIS|PLANNING|SOLUTIONING|IMPLEMENTATION|DONE]"
  completed_phases:
    - phase: ANALYSIS
      completed: "[YYYY-MM-DD]"
      gate_passed: true
      notes: "[Any relevant notes]"
    # Add more phases as they complete

gates:
  - gate: "Analysis → Planning"
    evaluated: "[YYYY-MM-DD]"
    result: "[passed|failed]"
    attempt: 1
    conditions:
      - id: "1.1"
        description: "Problem statement is clear"
        result: "[passed|failed]"
      - id: "1.2"
        description: "Scope boundaries defined"
        result: "[passed|failed]"
    notes: "[Gate evaluation notes]"
  # Add more gates as they are evaluated

requirements:
  functional:
    - id: FR-001
      title: "[Requirement title]"
      priority: "[must-have|should-have|could-have|wont-have]"
      status: "[defined|in-progress|implemented|verified]"
      story_id: "US-001"
      acceptance_criteria:
        - "[Criterion 1]"
        - "[Criterion 2]"
    # Add more functional requirements
  non_functional:
    - id: NFR-001
      title: "[Requirement title]"
      priority: "[must-have|should-have|could-have|wont-have]"
      status: "[defined|in-progress|implemented|verified]"
      architectural_strategy: "[How this is satisfied]"
      acceptance_criteria:
        - "[Criterion 1]"
    # Add more non-functional requirements

epics:
  - name: "[Epic name]"
    description: "[What this epic delivers]"
    stories:
      - US-001
      - US-002
  # Add more epics

stories:
  - id: US-001
    title: "[Story title]"
    epic: "[Epic name]"
    requirement: FR-001
    sprint: 1
    estimate: 3
    status: "[to-do|in-progress|done]"
    started: "[YYYY-MM-DD]"
    completed: "[YYYY-MM-DD]"
    acceptance_criteria:
      - criterion: "[AC text]"
        met: false
    files:
      - "[path/to/file.ts]"
  # Add more stories

sprints:
  current: 1
  total: "[estimated total]"
  items:
    - sprint: 1
      goal: "[Sprint goal]"
      status: "[planning|in-progress|complete]"
      started: "[YYYY-MM-DD]"
      completed: "[YYYY-MM-DD]"
      stories:
        - US-001
        - US-002
      files_owned:
        - "[path/to/file1.ts]"
        - "[path/to/file2.tsx]"
      velocity: "[story points completed]"
    # Add more sprints

verification:
  self_review:
    completed: false
    date: "[YYYY-MM-DD]"
    results:
      step_1_staged_changes: "[pass|fail]"
      step_2_completeness: "[pass|fail]"
      step_3_quality: "[pass|fail]"
      step_4_tests: "[pass|fail]"
      step_5_security: "[pass|fail]"
      step_6_documentation: "[pass|fail]"
    issues_found: 0
    issues_resolved: 0

artifacts:
  product_brief: "[path or 'not required']"
  prd: "[path or 'not required']"
  tech_spec: "[path or 'not required']"
  architecture_doc: "[path or 'not required']"
  sprint_plan: "[path or 'not required']"

session_history:
  - session: "[session number]"
    date: "[YYYY-MM-DD]"
    work_done: "[Summary of work completed]"
    next_steps: "[What to do next]"
```
