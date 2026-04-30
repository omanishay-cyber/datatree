# fireworks-workflow — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## Phase Gates

Phase gates are mandatory checkpoints between phases. No work proceeds to the next phase until the gate passes. Gates prevent the most expensive software error: building the wrong thing.

### Gate 1: Analysis → Planning

| Condition | Evidence |
|-----------|----------|
| Problem is understood | Analysis summary document exists |
| Scope is defined | IN/OUT scope boundaries documented |
| Stakeholder input received | User has confirmed problem statement |
| Constraints identified | Technical and business constraints listed |
| Open questions resolved | No blocking unknowns remain |

### Gate 2: Planning → Solutioning

| Condition | Evidence |
|-----------|----------|
| All requirements have unique IDs | FR-001, NFR-001 format used |
| All requirements have priorities | MoSCoW classification complete |
| All requirements have acceptance criteria | Testable conditions written |
| Dependencies mapped | Upstream/downstream dependencies listed |
| Success metrics defined | Measurable outcomes specified |

### Gate 3: Solutioning → Implementation

| Condition | Evidence |
|-----------|----------|
| Architecture reviewed | Architecture doc complete and approved |
| NFRs mapped to architecture | Every NFR has an architectural strategy |
| Technology justified | Tool/pattern choices explained |
| Risks assessed | Risk register with mitigations |
| Trade-offs documented | Key decisions justified |

### Gate 4: Implementation → Done

| Condition | Evidence |
|-----------|----------|
| All stories complete | Every story marked done |
| Tests pass | All tests green, coverage >=80% |
| Self-review complete | 6-step protocol executed |
| TypeScript clean | `tsc --noEmit` passes |
| Both themes verified | Light and dark mode checked |
| User acceptance | User confirms behavior matches requirements |

See `references/phase-gates.md` for detailed gate criteria and failure resolution.

---

## Workflow Commands

Commands available for orchestrating the workflow. These are invoked as slash commands.

| Command | Level | Purpose |
|---------|-------|---------|
| `/workflow-init` | All | Initialize project workflow, assess level |
| `/workflow-status` | All | Check progress, recommend next step |
| `/product-brief` | L2+ | Create discovery document |
| `/prd` | L2+ | Create Product Requirements Document |
| `/tech-spec` | L0-L1 | Create lightweight technical specification |
| `/architecture` | L2+ | Design system architecture |
| `/sprint-planning` | L2+ | Plan sprint iterations |
| `/create-story` | L1+ | Create user story with estimates |
| `/dev-story` | L1+ | Implement a specific story with TDD |

See `references/workflow-commands.md` for full documentation and example outputs.

---

## YAML Status Persistence

Project workflow state is persisted in `docs/workflow-status.yaml` within the project directory. This file is read on session start and updated on every milestone completion, ensuring work survives session boundaries.

### Status File Location

```
<project-root>/docs/workflow-status.yaml
```

### Status File Structure

```yaml
project:
  name: "Project Name"
  level: L2
  created: "2026-03-25"
  updated: "2026-03-25"

workflow:
  current_phase: PLANNING
  completed_phases:
    - phase: ANALYSIS
      completed: "2026-03-25"
      gate_passed: true

requirements:
  functional:
    - id: FR-001
      title: "Requirement title"
      priority: must-have
      status: defined
      acceptance_criteria:
        - "Condition 1"
        - "Condition 2"
  non_functional:
    - id: NFR-001
      title: "Performance requirement"
      priority: must-have
      status: defined

sprints:
  current: 1
  total: 2
  items:
    - sprint: 1
      status: in-progress
      stories:
        - id: US-001
          title: "Story title"
          status: in-progress
          estimate: 3
```

### Persistence Rules

1. Read `workflow-status.yaml` at the start of every session
2. Update after every phase gate passage
3. Update after every story status change
4. Update after every sprint completion
5. Never delete — only append and update
6. Include timestamps on all status changes

See `references/templates.md` for the complete YAML template.

---

## Self-Verification Protocol

A 6-step protocol executed before declaring any work complete. Derived from the Unicorn Team's "Fresh Eyes" methodology: review your own work as if you were a critical reviewer seeing it for the first time.

### Step 1: Review Staged Changes

Look at every changed line. Ask: "If this were someone else's pull request, would I approve it?"

- Are the changes minimal and focused?
- Is there any unrelated code mixed in?
- Are there any debugging artifacts left behind?

### Step 2: Completeness Check

- Does the implementation cover ALL requirements?
- Are edge cases handled (empty input, null, max values, concurrent access)?
- Are failure paths handled gracefully (network errors, disk full, permission denied)?
- Is input validated at every boundary?

### Step 3: Quality Check

- No TODO, FIXME, HACK, or XXX comments remain
- No `console.log`, `debugger`, or test data in production code
- Variable and function names are self-documenting
- Functions are under 50 lines (split if longer)
- No duplicated logic (DRY principle)

### Step 4: Test Verification

- All existing tests still pass
- New tests cover the new functionality
- Test coverage >= 80% for changed files
- Edge cases have dedicated tests
- Error paths are tested (not just happy path)

### Step 5: Security Check

- No secrets, API keys, or credentials in code
- No PII logged or exposed in error messages
- All user input is validated and sanitized
- Authentication and authorization enforced where required
- No SQL injection, XSS, or path traversal vulnerabilities

### Step 6: Documentation Check

- Public APIs have JSDoc/TSDoc comments
- Complex business logic has explanatory comments
- README updated if public interface changed
- Changelog entry added if user-facing change

See `references/self-verification.md` for the complete protocol with checklists.

---

## GATE Protocol

The GATE protocol governs how phase gates are evaluated. It ensures rigor without paralysis.

### Execution Steps

1. **Read** — Gather all gate condition results
2. **Check** — Evaluate each condition against its evidence requirement
3. **Pass/Fail** — Determine gate outcome
   - All conditions pass → proceed to next phase
   - Any condition fails → re-attempt with specific feedback
4. **Record** — Log gate result in `workflow-status.yaml`

### Failure Handling

- **First failure**: Identify the specific condition that failed. Provide actionable feedback on what needs to change.
- **Second failure**: Re-examine the approach. Consider whether the condition is being interpreted correctly.
- **Third failure on the same condition**: **STOP**. Report to the user with:
  - What condition is failing
  - What was attempted three times
  - Why it keeps failing
  - Suggested alternatives

### Rules

- Never skip a gate, even under time pressure
- Never force-pass a gate condition that genuinely fails
- Never modify gate criteria to make them easier to pass
- Gates are checkpoints, not bureaucracy — they exist to prevent costly mistakes
- If a gate seems unreasonable for the project level, reassess the project level

---

## Subagent Dispatch Patterns

Parallel agents accelerate workflow phases. These patterns maximize throughput while avoiding anti-patterns.

### Fan-Out Research (Phase 1: Analysis)

Dispatch 3-4 agents to gather information in parallel:

```
Agent 1 (Explore): Search codebase for existing patterns and conventions
Agent 2 (Explore): Map file dependencies and data flow
Agent 3 (Explore): Check external documentation via Context7
Agent 4 (Explore): Analyze similar implementations in the codebase
```

Merge results into a single Analysis Summary.

### Parallel Section Generation (Phase 2: Planning)

Dispatch N agents to write document sections simultaneously:

```
Agent 1 (general-purpose): Write functional requirements (FR-001 through FR-N)
Agent 2 (general-purpose): Write non-functional requirements (NFR-001 through NFR-N)
Agent 3 (general-purpose): Write user stories and acceptance criteria
Agent 4 (general-purpose): Write success metrics and dependencies
```

Merge sections into a single PRD.

### Component Parallel Design (Phase 3: Solutioning)

Dispatch 1 agent per component for architecture design:

```
Agent 1 (general-purpose): Design component A — interface, responsibilities, data model
Agent 2 (general-purpose): Design component B — interface, responsibilities, data model
Agent 3 (general-purpose): Design component C — interface, responsibilities, data model
```

After all complete, run a final integration agent to verify component interfaces align.

### Sprint Parallel Execution (Phase 4: Implementation)

Dispatch 1 agent per independent sprint:

```
Agent 1 (general-purpose): Execute Sprint 1 — [files A, B]
Agent 2 (general-purpose): Execute Sprint 2 — [files C, D]
Agent 3 (general-purpose): Execute Sprint 3 — [files E, F]
```

Two sprints must NEVER edit the same file. After all complete, run integration verification.

### Anti-Patterns

- **Do not spawn agents for tasks under 1,000 tokens** — the overhead exceeds the benefit
- **Do not pass full conversation history** to subagents — pass only the relevant context
- **Do not spawn more than 5 agents simultaneously** — diminishing returns and context fragmentation
- **Do not use subagents for sequential tasks** — only for genuinely parallel work
- **Do not skip the merge step** — parallel outputs must be reconciled

---

## Verification Gates

Four verification gates ensure quality throughout the lifecycle.

### Gate 1: Document Existence

Phase-appropriate documents exist and are complete:
- L0: None required
- L1: Tech spec exists
- L2+: PRD exists with requirement IDs
- L3+: Architecture document exists
- L4: All documents exist plus sprint plan

### Gate 2: Requirement Traceability

Every requirement can be traced from definition to implementation:
- Each requirement has a unique ID (FR-001, NFR-001)
- Each requirement maps to at least one user story
- Each user story maps to at least one test
- No orphan requirements (defined but never implemented)
- No orphan code (implemented but not required)

### Gate 3: Test Proof

Tests prove the implementation works:
- All tests pass (`npm test` or equivalent)
- Coverage >= 80% for new/changed code
- Edge cases tested (empty, null, max, concurrent)
- Error paths tested (not just happy path)
- Integration tests verify component interaction (L3+)

### Gate 4: Self-Review Complete

The 6-step self-verification protocol has been executed:
- Staged changes reviewed
- Completeness verified
- Quality standards met
- Tests verified
- Security checked
- Documentation checked

---

## State Machine — EMIT Phase

The workflow state machine includes an EMIT phase between EXECUTE and VERIFY to prevent half-written code when new unknowns surface mid-write.

### State Flow

```
PLAN → EXECUTE → EMIT → VERIFY → UPDATE-DOCS → COMPLETE
```

| Phase | Purpose |
|-------|---------|
| **PLAN** | Understand requirements, create roadmap, identify unknowns |
| **EXECUTE** | Resolve unknowns, run code, prototype — but do NOT write final files yet |
| **EMIT** | Write all resolved changes to disk atomically — only after all unknowns are resolved |
| **VERIFY** | Test, type-check, visual verify, run self-verification protocol |
| **UPDATE-DOCS** | Update workflow-status.yaml, session files, memory files |
| **COMPLETE** | Only after all verification passes and docs are current |

### Why EMIT Exists

Without EMIT, the EXECUTE phase writes files as it goes. If a new unknown is discovered halfway through writing, you end up with:
- Half the files updated to the new approach
- Half the files still on the old approach
- A broken codebase that requires manual reconciliation

With EMIT, EXECUTE resolves all unknowns in memory/scratch first. Only when every unknown is resolved does EMIT write everything at once. If a new unknown surfaces during EXECUTE, you snake-back to PLAN without having polluted the codebase.

### Snake-Back Rules

- If a new unknown is discovered during EXECUTE → return to PLAN (no files written yet, so no cleanup needed)
- If a new unknown is discovered during EMIT → STOP the write, return to EXECUTE to resolve it, then restart EMIT
- If verification fails during VERIFY → return to EXECUTE (EMIT will re-write after fixes)

---

## Mutables — Named Unknowns

Mutables are explicitly tracked unknowns that must be resolved before code is written. They formalize the "snake-back" pattern by giving every unknown a name and a resolution status.

### Lifecycle

1. **Before EXECUTE**: List all mutables as `name=UNKNOWN`
2. **During EXECUTE**: Resolve each mutable to `name=VALUE` through research, prototyping, or user input
3. **Before EMIT**: ALL mutables must be `name=VALUE` — no UNKNOWN remaining
4. **If stuck**: After 2 unresolved passes through EXECUTE, restart from PLAN with new approach

### Syntax

```
MUTABLES:
- apiShape=UNKNOWN          → apiShape=REST+JSON (resolved via Context7)
- dbSchema=UNKNOWN          → dbSchema=3 tables, see arch doc (resolved via research)
- authFlow=UNKNOWN          → authFlow=token-based, 30min expiry (resolved via user input)
- stateShape=UNKNOWN        → stateShape=Zustand slice per domain (resolved via codebase patterns)
```

### Rules

- Every mutable MUST have a name — unnamed unknowns are invisible and dangerous
- Resolution must include HOW it was resolved (research, user input, prototyping, Context7)
- A mutable resolved as `VALUE` cannot revert to `UNKNOWN` — if new info invalidates it, create a NEW mutable
- Mutables are logged in the session file for cross-session continuity

### Common Mutables by Project Type

#### Electron Projects
| Mutable | What It Resolves |
|---------|-----------------|
| `ipcShape` | Channel names, payload types, direction (invoke vs send) |
| `processBoundary` | Which logic runs in main vs renderer vs preload |
| `dbSchema` | Tables, columns, indexes, migrations |
| `stateShape` | Zustand store structure, slices, selectors |
| `windowLifecycle` | Window creation, persistence, multi-window coordination |
| `securityModel` | CSP, sandbox, contextIsolation, allowed APIs |

#### Flutter Projects
| Mutable | What It Resolves |
|---------|-----------------|
| `stateArch` | Riverpod/Bloc/Provider structure |
| `navShape` | GoRouter routes, guards, deep links |
| `apiContract` | REST/GraphQL endpoints, request/response shapes |
| `platformChannels` | Native method channels and their payloads |
| `storageStrategy` | Hive/SQLite/SharedPreferences and what goes where |
| `buildFlavors` | Dev/staging/prod configurations |

---

## PRD Completion Gate

When a numbered roadmap is created, every item becomes a trackable commitment. The PRD Completion Gate ensures nothing is skipped or forgotten before claiming COMPLETE.

### Tracking Format

When a roadmap is created, write tasks in this format:

```
ROADMAP TRACKER:
- [1] Define IPC channels              → STATUS: PENDING
- [2] Create database schema            → STATUS: PENDING
- [3] Build UI components               → STATUS: PENDING
- [4] Wire state management             → STATUS: PENDING
- [5] Add tests                         → STATUS: PENDING
- [6] Run verification protocol         → STATUS: PENDING
- [7] Update docs                       → STATUS: PENDING
```

### Completion Rules

1. **Every numbered item must reach STATUS: DONE** — no exceptions
2. **STATUS values**: PENDING → IN-PROGRESS → DONE | BLOCKED | DESCOPED
3. **BLOCKED items** require a reason and an unblock plan
4. **DESCOPED items** require explicit user approval — you cannot descope on your own
5. **Before claiming COMPLETE**: count DONE items vs total items — they must match
6. **Pattern**: "If roadmap has 7 steps, all 7 must be verified DONE before COMPLETE"

### Gate Check

```
PRD COMPLETION GATE:
Total items: 7
Done: 7 | Blocked: 0 | Descoped: 0 | Pending: 0
Gate: PASS ✓
```

If the gate does not PASS, you are NOT done. Return to the first non-DONE item and continue.

---

## Orchestration Modes

Four modes govern how work is dispatched based on task characteristics. Choose the right mode before starting execution.

| Mode | When | Pattern |
|---|---|---|
| **Solo Sprint** | Simple task, 1 file, <30 min | Single agent, fast iteration |
| **Domain Deep-Dive** | Complex investigation | Specialist agent with full context |
| **Multi-Agent Handoff** | Sequential pipeline | Agent A → Agent B → Agent C |
| **Skill Chain** | Composable workflow | Skill invokes skill invokes skill |

### Solo Sprint

For L0-L1 tasks where overhead exceeds benefit. One agent, one file, fast cycle.

- No subagent dispatch — work directly
- Skip formal PRD — use inline tech spec
- Execute → Verify → Done
- Example: fix a CSS bug, add a config option, update a constant

### Domain Deep-Dive

For tasks requiring deep expertise in one area. A single specialist agent gets full context and autonomy.

- Dispatch ONE specialist agent (e.g., `css-wizard`, `electron-debugger`, `ipc-specialist`)
- Provide full context: files, error messages, expected behavior
- Agent works autonomously and returns findings
- Example: investigate a complex state management bug, optimize a slow query

### Multi-Agent Handoff

For sequential pipelines where each stage depends on the previous. Agent A's output becomes Agent B's input.

- Agent A (Research) → produces findings document
- Agent B (Plan) → consumes findings, produces implementation plan
- Agent C (Implement) → consumes plan, produces code
- Each handoff includes explicit context transfer — no implicit state
- Example: new feature from research through implementation

### Skill Chain

For composable workflows where skills invoke other skills in sequence.

- `fireworks-workflow` → `fireworks-architect` → `fireworks-test`
- Each skill completes its phase and passes artifacts to the next
- Phase gates between skills ensure quality at each boundary
- Example: full SDLC workflow from PRD to deployed feature

### Mode Selection Heuristic

1. Count affected files: 1 = Solo Sprint, 2-5 = Domain Deep-Dive, 5+ = Multi-Agent or Skill Chain
2. Check dependencies: sequential = Multi-Agent Handoff, parallel = fan-out within any mode
3. Check complexity: investigation = Domain Deep-Dive, full lifecycle = Skill Chain
4. When in doubt, start with Domain Deep-Dive and escalate if needed

---

## INVARIANTS

These rules are absolute. They cannot be overridden by convenience, time pressure, or "it works on my machine."

1. **Never skip required phases for the project level.** L0 can skip planning; L4 cannot. The level determines the minimum process.

2. **Every requirement must have a unique ID.** FR-001 for functional, NFR-001 for non-functional. No exceptions. IDs enable traceability.

3. **Every story must have acceptance criteria.** "As a user I want X" without acceptance criteria is not a story — it is a wish.

4. **Phase gates are mandatory checkpoints, not optional.** Work does not proceed to the next phase until the gate passes. The GATE protocol governs evaluation.

5. **Status file must be updated after every phase completion.** `docs/workflow-status.yaml` is the single source of truth for project state. If the file does not reflect reality, reality is wrong.

6. **Never claim done without verification.** Run the app. Check both themes. Execute the self-verification protocol. "It compiles" is not "it works."

7. **Three failures on the same gate condition trigger a STOP.** Do not brute-force past gates. Escalate to the user.

8. **Subagent outputs must be merged and reconciled.** Parallel does not mean independent. Integration verification is mandatory.

9. **Sprint boundaries must not overlap files.** Two sprints editing the same file creates merge conflicts and defeats parallelism.

10. **Requirements drive implementation, not the reverse.** Code exists to satisfy requirements. If code exists without a requirement, either the requirement is missing or the code should not exist.

---

## Reference Files

| File | Purpose |
|------|---------|
| `references/phase-gates.md` | Detailed gate criteria, evidence requirements, failure resolution |
| `references/project-levels.md` | L0-L4 definitions, decision tree, time estimates |
| `references/workflow-commands.md` | Full command documentation with examples |
| `references/self-verification.md` | 6-step protocol, fresh eyes techniques, security questions |
| `references/templates.md` | All document templates (PRD, tech spec, architecture, stories, YAML) |

---

## Quick Reference — When to Invoke This Skill

- **Starting a new project or feature** — `/workflow-init` to assess level and begin
- **Resuming work across sessions** — `/workflow-status` to read YAML state and continue
- **Need a PRD** — `/prd` to generate requirements document
- **Need architecture** — `/architecture` to design system structure
- **Planning sprints** — `/sprint-planning` to break work into iterations
- **Before claiming done** — run self-verification protocol
- **Gate is failing** — follow GATE protocol failure handling

## Integration with Other Fireworks Skills

This skill is the orchestration layer. Other Fireworks skills handle execution:

- **fireworks-architect** — deep architecture design during Solutioning phase
- **fireworks-design** — UI/UX design during Solutioning phase
- **fireworks-test** — TDD execution during Implementation phase
- **fireworks-review** — code review during Implementation phase
- **fireworks-debug** — debugging when implementation reveals issues
- **fireworks-security** — security review during Implementation phase
- **fireworks-performance** — performance optimization during Implementation phase

## Activation

This skill activates automatically when:
- User requests project planning, PRD creation, or sprint planning
- User says "workflow", "lifecycle", "phase gate", "PRD", "sprint"
- User starts a new multi-file feature (L2+)
- User asks about project status or next steps
- Session starts and `docs/workflow-status.yaml` exists in the project

---

## Scope Boundaries

- **MINIMUM**: L0 projects still need a 1-line goal statement. Even a one-line fix deserves a sentence describing what it solves.
- **MAXIMUM**: L4 workflow is the ceiling — do not add more process beyond L4. If L4 feels insufficient, the project needs to be split, not given more bureaucracy.

---

## Related Skills

- **fireworks-estimation** — Sizing and estimation within workflow phases, PERT estimates for sprint planning
- **fireworks-devops** — Deployment phase execution, CI/CD pipeline setup during Implementation phase
- **fireworks-taskmaster** — Task management within sprints, backlog grooming, progress tracking
