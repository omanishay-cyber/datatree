# Phase Gates — Detailed Reference

Phase gates are mandatory quality checkpoints between workflow phases. They prevent the most expensive class of software error: building the wrong thing, or building the right thing on a broken foundation.

---

## Gate Philosophy

Gates exist because errors compound across phases. A misunderstood requirement in Analysis becomes a wrong specification in Planning, a flawed architecture in Solutioning, and wasted implementation effort. Catching errors at the gate is 10x cheaper than catching them in the next phase.

Gates are not bureaucracy. They are the engineering equivalent of "measure twice, cut once."

---

## Gate 1: Analysis → Planning

**Purpose**: Confirm the problem is understood before defining solutions.

### Conditions

| # | Condition | Evidence Required | Verification Method |
|---|-----------|-------------------|---------------------|
| 1.1 | Problem statement is clear | Written problem statement exists | Read and confirm it answers "what" and "why" |
| 1.2 | Scope boundaries defined | IN/OUT scope list exists | Confirm at least 3 items in each list |
| 1.3 | Stakeholder input received | User has reviewed and confirmed problem statement | Explicit user confirmation in conversation |
| 1.4 | Constraints identified | Technical and business constraints listed | At least 2 constraints documented |
| 1.5 | Open questions resolved | No blocking unknowns | All questions marked resolved or descoped |
| 1.6 | Existing code understood | Codebase reconnaissance complete | Files involved and patterns documented |

### Passing Examples

**PASS**: "Problem: Users cannot filter inventory by supplier. Scope: Add supplier filter to inventory list page (IN), do not modify supplier management (OUT). Constraints: Must work with existing sql.js queries, must support both themes. No open questions."

**PASS**: "Problem: Report generation takes 30+ seconds for large datasets. Scope: Optimize the sales report query (IN), do not change report layout (OUT). Constraints: Cannot change database schema, must maintain backward compatibility."

### Failing Examples

**FAIL**: "We need to improve the inventory page." — No specific problem statement, no scope, no constraints.

**FAIL**: "Users want better reports." — Vague. What is "better"? Which reports? What does the user actually need?

**FAIL**: Problem is clear but "I'm not sure if this needs a new database table" — open question is blocking.

---

## Gate 2: Planning → Solutioning

**Purpose**: Confirm all requirements are defined, traceable, and testable before designing architecture.

### Conditions

| # | Condition | Evidence Required | Verification Method |
|---|-----------|-------------------|---------------------|
| 2.1 | All requirements have unique IDs | FR-001, NFR-001 format | Scan document for ID format |
| 2.2 | All requirements have priorities | MoSCoW classification | Every requirement tagged Must/Should/Could/Won't |
| 2.3 | All requirements have acceptance criteria | Testable conditions per requirement | Each requirement has at least 1 acceptance criterion |
| 2.4 | Dependencies identified | Upstream/downstream deps listed | Dependencies section exists and is non-empty |
| 2.5 | Success metrics defined | Measurable outcomes | At least 2 quantifiable metrics |
| 2.6 | No conflicting requirements | Requirements are internally consistent | No two requirements contradict each other |

### Passing Examples

**PASS**: PRD contains FR-001 through FR-008, each with MoSCoW priority, each with 2-3 acceptance criteria. NFR-001 through NFR-003 define performance and accessibility targets. Dependencies on existing inventory module documented.

### Failing Examples

**FAIL**: Requirements exist but none have IDs — untraceable through architecture and implementation.

**FAIL**: Requirements have IDs and priorities but FR-003 says "the system should be fast" with no acceptance criterion — untestable.

**FAIL**: FR-002 says "all data is encrypted at rest" and FR-005 says "export raw data to CSV" — potential conflict (is exported CSV encrypted?).

---

## Gate 3: Solutioning → Implementation

**Purpose**: Confirm the architecture is sound before writing code.

### Conditions

| # | Condition | Evidence Required | Verification Method |
|---|-----------|-------------------|---------------------|
| 3.1 | Architecture document complete | All 10 sections present | Section checklist verification |
| 3.2 | NFRs mapped to architecture | Every NFR has architectural strategy | Cross-reference NFR IDs to architecture doc |
| 3.3 | Technology choices justified | Rationale for each tool/pattern | Justification section exists with trade-offs |
| 3.4 | Risks assessed | Risk register with mitigations | At least 3 risks identified with mitigations |
| 3.5 | Trade-offs documented | Key decisions have alternatives analysis | Trade-off table exists for major decisions |
| 3.6 | Component interfaces defined | API/contract between components | Interface definitions exist |
| 3.7 | Data flow documented | End-to-end data path traced | Data flow section shows input-to-output path |
| 3.8 | User approved architecture | Explicit approval | User confirmation in conversation |

### Passing Examples

**PASS**: Architecture doc defines three components (SupplierFilter, InventoryQuery, FilterPanel) with typed interfaces. NFR-001 (response time <200ms) mapped to indexed SQL query strategy. Risk: large supplier lists may slow rendering — mitigation: virtualized list. User has reviewed and approved.

### Failing Examples

**FAIL**: Architecture doc exists but NFR-002 (accessibility) has no architectural strategy — will be discovered too late in implementation.

**FAIL**: Two components have undefined interfaces — integration will be ad-hoc and fragile.

**FAIL**: No trade-off analysis — how do we know this approach is better than alternatives?

---

## Gate 4: Implementation → Done

**Purpose**: Confirm the implementation is complete, correct, and production-ready.

### Conditions

| # | Condition | Evidence Required | Verification Method |
|---|-----------|-------------------|---------------------|
| 4.1 | All stories complete | Every story status is "done" | Check workflow-status.yaml |
| 4.2 | All tests pass | Test suite green | Run `npm test` or equivalent |
| 4.3 | Coverage adequate | >= 80% for changed files | Run coverage report |
| 4.4 | TypeScript clean | No type errors | Run `tsc --noEmit` |
| 4.5 | Self-review complete | 6-step protocol executed | Checklist documented |
| 4.6 | Both themes verified | Light and dark mode checked | Visual inspection or screenshot |
| 4.7 | No debug artifacts | No console.log, debugger, TODO | Code search for artifacts |
| 4.8 | User acceptance | User confirms behavior | Explicit user confirmation |

### Passing Examples

**PASS**: All 5 stories marked done. 47 tests pass, coverage at 84%. `tsc --noEmit` clean. Self-review checklist complete with no issues. Screenshots of both themes provided. User confirms behavior matches requirements.

### Failing Examples

**FAIL**: Tests pass but coverage is 62% — insufficient confidence in correctness.

**FAIL**: `tsc --noEmit` reports 3 errors — type safety compromised.

**FAIL**: Dark mode not checked — visual regressions may exist.

---

## Gate Failure Resolution Process

When a gate condition fails, follow this process:

### Step 1: Identify the Failure

Be specific. "Gate failed" is not actionable. "Condition 2.3 failed: FR-004 has no acceptance criteria" is actionable.

### Step 2: Determine Root Cause

- **Missing work**: The required artifact was never created → create it
- **Insufficient quality**: The artifact exists but does not meet the standard → improve it
- **Misunderstanding**: The condition is being interpreted incorrectly → clarify with the user
- **Infeasible condition**: The condition cannot be satisfied for this project → escalate

### Step 3: Re-Attempt

Fix the specific issue and re-evaluate only the failed conditions. Do not re-evaluate conditions that already passed.

### Step 4: Escalation (After 3 Failures)

If the same condition fails three times:

1. **STOP** — do not attempt a fourth time
2. **Document** the three attempts and why each failed
3. **Report** to the user with:
   - Which condition is failing
   - What was attempted
   - Why it keeps failing
   - Whether the condition should be modified for this project
4. **Wait** for user direction before proceeding

### Step 5: Record

Log the gate evaluation in `workflow-status.yaml`:

```yaml
gates:
  - gate: "Analysis → Planning"
    evaluated: "2026-03-25"
    result: passed
    conditions:
      - id: "1.1"
        result: passed
      - id: "1.2"
        result: passed
    notes: "All conditions met on first attempt"
```

---

## Level-Specific Gate Requirements

Not all gate conditions apply at every level. The matrix below shows which conditions are mandatory per level.

### Gate 1 (Analysis → Planning)

| Condition | L0 | L1 | L2 | L3 | L4 |
|-----------|----|----|----|----|-----|
| 1.1 Problem statement | skip | required | required | required | required |
| 1.2 Scope boundaries | skip | optional | required | required | required |
| 1.3 Stakeholder input | skip | optional | required | required | required |
| 1.4 Constraints | skip | optional | required | required | required |
| 1.5 Open questions | skip | required | required | required | required |
| 1.6 Existing code understood | skip | required | required | required | required |

### Gate 2 (Planning → Solutioning)

| Condition | L0 | L1 | L2 | L3 | L4 |
|-----------|----|----|----|----|-----|
| 2.1 Unique IDs | skip | skip | required | required | required |
| 2.2 Priorities | skip | skip | required | required | required |
| 2.3 Acceptance criteria | skip | optional | required | required | required |
| 2.4 Dependencies | skip | skip | required | required | required |
| 2.5 Success metrics | skip | skip | optional | required | required |
| 2.6 No conflicts | skip | skip | required | required | required |

### Gate 3 (Solutioning → Implementation)

| Condition | L0 | L1 | L2 | L3 | L4 |
|-----------|----|----|----|----|-----|
| 3.1 Architecture doc | skip | skip | required | required | required |
| 3.2 NFR mapping | skip | skip | optional | required | required |
| 3.3 Tech justification | skip | skip | optional | required | required |
| 3.4 Risk assessment | skip | skip | required | required | required |
| 3.5 Trade-offs | skip | skip | optional | required | required |
| 3.6 Component interfaces | skip | skip | required | required | required |
| 3.7 Data flow | skip | skip | required | required | required |
| 3.8 User approval | skip | skip | required | required | required |

### Gate 4 (Implementation → Done)

| Condition | L0 | L1 | L2 | L3 | L4 |
|-----------|----|----|----|----|-----|
| 4.1 Stories complete | skip | required | required | required | required |
| 4.2 Tests pass | required | required | required | required | required |
| 4.3 Coverage adequate | optional | optional | required | required | required |
| 4.4 TypeScript clean | required | required | required | required | required |
| 4.5 Self-review | optional | required | required | required | required |
| 4.6 Both themes | required | required | required | required | required |
| 4.7 No debug artifacts | required | required | required | required | required |
| 4.8 User acceptance | optional | optional | required | required | required |
