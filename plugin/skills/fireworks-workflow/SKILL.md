---
name: fireworks-workflow
description: Project lifecycle orchestration — phase gates, level-based rigor scaling, workflow commands, YAML status persistence, sprint planning, and self-verification protocols
version: 2.0.0
author: mneme
tags: [workflow, lifecycle, phase-gate, sprint, project, status, plan, PRD]
triggers: [workflow, lifecycle, phase gate, sprint, project, status, plan, PRD, how to start, project setup]
---

# Fireworks Workflow — Project Lifecycle Orchestration

## Core Principle

**Every project deserves exactly the right amount of process — no more, no less.**

A one-line fix does not need a PRD. A 30-file system redesign cannot survive on "just start coding." This skill scales rigor to match complexity: lightweight for small changes, heavyweight for system-level work. It consolidates project lifecycle management from BMAD Method v6 and Unicorn Team methodology into a single orchestration framework.

The workflow follows four phases — Analysis, Planning, Solutioning, Implementation — with mandatory gates between each. The level system (L0-L4) determines which phases and artifacts are required. Status is persisted in YAML so work survives session boundaries.

---

## Project Level Assessment

Every task begins with a level assessment. The level determines required phases, artifacts, and rigor.

### Level Definitions

| Level | Scope | Files | Phases Required | Artifacts |
|-------|-------|-------|-----------------|-----------|
| **L0** | Single change or fix | 1 | Implement only | None — just implement + test |
| **L1** | Small feature | 1-3 | Analysis + Implement | Lightweight tech spec |
| **L2** | Medium feature | 4-10 | All 4 phases | PRD, architecture doc |
| **L3** | Large feature | 10-30 | All 4 phases + sprints | PRD, architecture, sprint plan |
| **L4** | System-level | 30+ | Full SDLC, multi-sprint | All artifacts, phase gates mandatory |

### Level Assessment Decision Tree

1. Is this a bug fix, typo, or config change? → **L0**
2. Does it touch 1-3 files with a clear path? → **L1**
3. Does it require new data models, IPC channels, or UI components? → **L2**
4. Does it span multiple subsystems (DB + IPC + UI + state)? → **L3**
5. Is it a rewrite, migration, or new product? → **L4**

### Escalation Rules

- If any file touches security (auth, encryption, tokens): minimum **L2**
- If changes cross process boundaries (main/renderer): add one level
- If data migration is involved: minimum **L3**
- If uncertain about level: start one level higher, descale if warranted

See `references/project-levels.md` for detailed definitions and examples.

---

## 4-Phase Workflow

### Phase 1: ANALYSIS

**Goal**: Understand the problem completely before proposing solutions.

Activities:
1. **Requirements Elicitation** — What does the user actually need? Ask clarifying questions.
2. **Domain Research** — Understand the business context and terminology.
3. **Codebase Reconnaissance** — Map existing code, patterns, and conventions.
4. **Constraint Discovery** — Performance, compatibility, security, timeline.
5. **Stakeholder Input** — Get user priorities: what matters most?
6. **Competitive Analysis** — How do similar tools solve this? (L3+)

Output:
```
ANALYSIS SUMMARY:
- Problem Statement: [1-2 sentences]
- User Need: [what the user is trying to accomplish]
- Current State: [how things work today]
- Constraints: [technical, timeline, business]
- Open Questions: [anything still unclear]
- Scope Boundary: [what is IN scope, what is OUT]
```

### Phase 2: PLANNING

**Goal**: Define what to build with traceable, testable requirements.

Activities:
1. **Product Brief** — Discovery document capturing vision and scope (L2+)
2. **PRD Creation** — Formal requirements with unique IDs (L2+)
3. **Tech Spec** — Lightweight technical specification (L0-L1)
4. **Requirement Classification** — Functional (FR-001) vs Non-Functional (NFR-001)
5. **Priority Assignment** — Must-have, Should-have, Could-have, Won't-have (MoSCoW)
6. **Acceptance Criteria** — Testable conditions for every requirement

Output:
```
PLANNING DELIVERABLES:
- [ ] Product Brief (L2+)
- [ ] PRD with requirement IDs (L2+) or Tech Spec (L0-L1)
- [ ] All requirements have acceptance criteria
- [ ] Priorities assigned (MoSCoW)
- [ ] Dependencies identified
- [ ] Success metrics defined
```

### Phase 3: SOLUTIONING

**Goal**: Design how to build it — architecture, components, trade-offs.

Activities:
1. **Architecture Design** — System structure, component boundaries, data flow
2. **Component Design** — Individual module responsibilities and interfaces
3. **NFR Mapping** — How each non-functional requirement is satisfied architecturally
4. **Trade-Off Analysis** — Compare approaches, justify decisions
5. **Risk Assessment** — What could go wrong, mitigation strategies
6. **Technology Justification** — Why these tools/patterns over alternatives

Output:
```
SOLUTIONING DELIVERABLES:
- [ ] Architecture document (L2+)
- [ ] Component interaction diagram (L3+)
- [ ] NFR-to-architecture mapping
- [ ] Trade-off analysis for key decisions
- [ ] Risk register with mitigations
- [ ] Technology justification
```

### Phase 4: IMPLEMENTATION

**Goal**: Build it with discipline, test it thoroughly, verify it works.

Activities:
1. **Sprint Planning** — Break work into 1-2 week sprints (L2+)
2. **Story Creation** — User stories with estimates and acceptance criteria
3. **TDD Execution** — Write tests first, then implementation
4. **Code Review** — Self-review using the 6-step protocol
5. **Integration Testing** — Verify components work together
6. **Verification** — Run the app, check both themes, confirm behavior

Output:
```
IMPLEMENTATION DELIVERABLES:
- [ ] Sprint plan (L2+)
- [ ] Stories with estimates
- [ ] Code implemented per plan
- [ ] Tests passing (>=80% coverage)
- [ ] Self-review complete (6-step protocol)
- [ ] User verification
```

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
