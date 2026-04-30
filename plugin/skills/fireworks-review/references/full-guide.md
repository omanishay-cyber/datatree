# fireworks-review — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 5. Pragmatic Filtering Rules

Not every finding deserves airtime. Apply these filters before including a finding in the report.

### Filter 1: Does This Actually Matter?
- Will a real user encounter this issue? If only in extreme edge cases with negligible impact, skip it.
- Does this affect data integrity, security, or core functionality? If not, consider downgrading to LOW.
- Is this a theoretical concern or a practical one? Skip theoretical "what-if" scenarios with no evidence.

### Filter 2: Is This the Right Time?
- During a hotfix: only flag CRITICAL and HIGH issues. Save MEDIUM/LOW for a follow-up.
- During a feature PR: full review across all severities.
- During a refactor: focus on Architecture and Simplicity lenses. Logic/Security only if patterns change.
- Never bikeshed during an emergency.

### Filter 3: Is This Actionable?
- Can the developer understand what is wrong from your description alone?
- Does your finding include a specific fix (not just "this is bad")?
- Is the fix proportional to the severity? Don't request a rewrite for a LOW finding.
- Include code snippets for the fix whenever possible.

### Filter 4: Pre-existing vs. Introduced
- If the issue existed before this changeset, flag it but do NOT count it as a review failure
- Mark pre-existing issues with `[PRE-EXISTING]` prefix
- Pre-existing issues are capped at MEDIUM severity in the review context
- Track them separately for future cleanup sprints

### Filter 5: Context Awareness
- Prototype/POC code: relax Architecture and Simplicity lenses
- Production code: full rigor across all lenses
- Test code: focus on Logic lens, relax Performance and Architecture
- Configuration files: focus on Security lens

---

## 6. Review Output Format

```
## Code Review: [PR/Task Title]

### Summary
[1-2 sentences: overall assessment of the changeset — what it does, how well it does it]

### Statistics
- Files reviewed: X
- Lines changed: +Y / -Z
- Findings: A critical, B high, C medium, D low

### Findings

#### [CRITICAL] Finding Title
**File**: `path/to/file.ts:42`
**Lens**: Security
**Confidence**: 95%
**Issue**: Clear description of what is wrong and why it matters.
```typescript
// Current code (problematic)
const query = `SELECT * FROM users WHERE id = ${userId}`;
```
**Fix**: How to fix it, with code.
```typescript
// Fixed code
const query = `SELECT * FROM users WHERE id = ?`;
db.prepare(query).get(userId);
```
**Impact**: What happens if this is not fixed.

#### [HIGH] Finding Title
**File**: `path/to/file.ts:87`
**Lens**: Logic
**Confidence**: 90%
**Issue**: ...
**Fix**: ...

#### [PRE-EXISTING] [MEDIUM] Finding Title
**File**: `path/to/file.ts:120`
**Lens**: Architecture
**Confidence**: 85%
**Issue**: ...
**Fix**: ...
**Note**: This issue predates this changeset. Flagging for awareness.

### Positive Observations
- [What the author did well — good patterns, clean abstractions, thorough tests]
- [Positive reinforcement is important — it encourages good practices]

### Verdict: [APPROVE / REQUEST CHANGES / NEEDS DISCUSSION]
[1-2 sentence justification for the verdict]
```

### Verdict Decision Matrix
| Findings | Verdict |
|----------|---------|
| 0 CRITICAL, 0 HIGH | **APPROVE** (with optional LOW/MEDIUM notes) |
| 0 CRITICAL, 1+ HIGH | **REQUEST CHANGES** (list required fixes) |
| 1+ CRITICAL | **REQUEST CHANGES** (block until resolved) |
| 3+ uncertain findings | **NEEDS DISCUSSION** (invoke 3-Strike Rule) |

---

## 7. PR Review Workflow

When reviewing a Pull Request, follow this extended workflow:

### Phase 1: Context Gathering
1. Read the PR title and description
2. Read the linked issue or task (if any)
3. Understand the intent: what problem does this solve?
4. Check the branch name for hints about scope

### Phase 2: Diff Analysis
1. Get the full diff: `git diff base...HEAD`
2. List all changed files: `git diff --name-status base...HEAD`
3. Identify the type of change: feature, bugfix, refactor, config, docs
4. Note deleted files — was anything important removed?

### Phase 3: File-by-File Review
1. Review each file applying all 6 lenses
2. For large files, read the entire file (not just the diff) to understand context
3. Check import changes — new dependencies introduced?
4. Check export changes — is the public API affected?

### Phase 4: Cross-Cutting Concerns
1. **Tests**: Are new code paths tested? Are existing tests updated for changed behavior?
2. **Breaking changes**: Does this change the public API, database schema, or IPC channels?
3. **Migration**: Does this require a data migration or version bump?
4. **Documentation**: Do README, JSDoc, or inline comments need updates?
5. **Types**: Are TypeScript types correct and complete? Any `any` introduced?

### Phase 5: Integration Check
1. Does this change interact correctly with existing code?
2. Are there circular dependencies introduced?
3. Does the state management remain consistent?
4. Are IPC channels properly typed end-to-end (renderer <-> preload <-> main)?

### Phase 6: Report
1. Compile findings using the standard output format
2. Assign verdict
3. If REQUEST CHANGES, list exactly what needs to change before approval

---

## 8. Verification Gates (Vinicius Pattern)

Before finalizing any review, pass through all 4 gates. If any gate fails, the review is incomplete.

### Gate 1: Completeness Check
- [ ] Every changed file has been read **completely** (not skimmed, not summarized)
- [ ] Context around changes has been understood (not just the diff lines)
- [ ] The purpose of the change is clear
- **Failure mode**: "I reviewed the important files" — ALL files must be reviewed

### Gate 2: Evidence Check
- [ ] Every finding has a specific `file:line` reference
- [ ] Every finding includes the actual code snippet (not paraphrased)
- [ ] Every finding includes a concrete suggested fix with code
- **Failure mode**: "There might be an issue in the auth module" — WHERE exactly?

### Gate 3: Severity Calibration Check
- [ ] CRITICAL findings are genuinely catastrophic (crash, data loss, security breach)
- [ ] Not more than 20% of findings are CRITICAL
- [ ] Severity justification makes sense (would another reviewer agree?)
- **Failure mode**: Everything marked CRITICAL — recalibrate

### Gate 4: False Positive Check
- [ ] Re-evaluate findings with confidence below 90%
- [ ] Check if TypeScript types already prevent the flagged issue
- [ ] Verify the issue is not handled by an upstream error boundary or middleware
- [ ] Confirm the issue exists in the actual execution context (renderer vs main)
- **Failure mode**: Flagging a SQL injection in code that uses parameterized queries

---

## 9. Anti-Premature-Completion

**"I reviewed the code and it looks good" is NOT a review.**

An actual review means:
1. Every changed line has been read and understood
2. All 6 lenses have been applied to every file
3. Specific findings are documented with evidence
4. Positive observations are noted (what was done well)
5. A justified verdict is provided

### Minimum Review Output
Even for a perfect changeset with zero issues, the review must include:
- Summary of what was reviewed
- Confirmation that all 6 lenses were applied
- At least one positive observation
- The APPROVE verdict with justification

### Red Flags That a Review Was Lazy
- No file:line references anywhere
- No code snippets in findings
- Vague language: "looks good", "seems fine", "should be okay"
- Missing lenses — if Architecture was not mentioned, it was not checked
- Review completed in under 30 seconds for 500+ changed lines

---

## 10. 3-Strike Rule

If you are uncertain about **3 or more findings**, stop the review and discuss with the user.

### What Counts as Uncertain
- Confidence below 80% but the finding feels important
- Severity unclear — could be HIGH or could be LOW depending on context
- The fix is not obvious — you know something is off but not how to fix it
- Domain-specific logic you cannot verify without business context

### How to Invoke the 3-Strike Rule
```
### Review Paused: 3-Strike Rule

I identified 3+ findings where I need your input before finalizing:

1. **[file.ts:42]** Is `calculateMargin()` supposed to include tax?
   If yes: no issue. If no: HIGH severity logic bug.

2. **[store.ts:88]** The state update pattern differs from other stores.
   Is this intentional (optimized) or accidental (should follow pattern)?

3. **[ipc.ts:15]** The channel name `sync:pull` — is this expected to be
   called from renderer? If yes: needs input validation. If no: no issue.

Please clarify so I can complete the review with accurate findings.
```

---

## 11. Meta-Judge Pattern

Before dispatching reviewers, generate task-specific evaluation criteria:
1. Analyze the code change type (feature, bugfix, refactor, security)
2. Generate weighted rubric YAML specific to THIS change
3. Pass rubric UNCHANGED to all reviewer agents
4. Reviewers score against rubric without knowing pass/fail thresholds
5. Orchestrator reads ONLY VERDICT/SCORE/ISSUES — never implementation details

---

## 12. Two New Review Lenses

Add to existing 6-lens system:
- **Lens 7: Contracts Reviewer** — Validates API contracts, IPC channel signatures, Zustand store shapes, exported types. Catches breaking changes before they cascade.
- **Lens 8: Historical Context Reviewer** — Uses `git log` and `git blame` to understand WHY code exists before suggesting changes. Prevents removing "unnecessary" code that handles a real edge case.

---

## 13. Judge-with-Debate Protocol

For critical reviews (security, architecture, data integrity):
1. 3 judges independently evaluate
2. Enter debate rounds (max 3)
3. Consensus requires: overall scores within 0.5 points, no criterion with >1-point disagreement
4. If no consensus after 3 rounds: declare "no consensus" and escalate to user

---

## 14. Anti-Performative Review

When receiving review feedback: do NOT say "Great point!" or "You're absolutely right!" — verify the suggestion technically, push back if wrong, and just fix it if right. Technical correctness over social comfort.

---

## 15. Reference Links

### Internal References
- `references/logic-review.md` — Bug patterns, edge cases, async, data integrity
- `references/security-review.md` — OWASP, CWE, Electron, IPC, credentials, dependencies
- `references/performance-review.md` — React, bundle, memory, sync ops, database
- `references/architecture-review.md` — SOLID, coupling, cohesion, abstraction, patterns
- `references/simplicity-review.md` — YAGNI, over-engineering, dead code, simplification

### Related Skills
- `premium-design` — UI/Design lens detailed standards
- `electron-patterns` — Electron-specific security and architecture patterns
- `security-pipeline` — Deep security analysis workflow
- `tdd-workflow` — Test coverage verification
- `quality-gate` — Final quality verification before completion
- `fireworks-security` — security lens
- `fireworks-test` — test coverage check
- `fireworks-patterns` — pattern recognition in review

---

## Scope Boundaries

- **MINIMUM**: Always apply all 6 review lenses.
- **MAXIMUM**: Budget ~5 minutes per 100 LOC for thorough review.

### Related Agents
- `code-reviewer` — General code review agent
- `pr-reviewer` — Pull request review agent
- `pragmatic-code-reviewer` — Pragmatic filtering agent
- `multi-perspective-reviewer` — Multi-lens analysis agent
- `architect-reviewer` — Architecture review agent
- `security-reviewer` — Security-focused review agent
- `code-simplifier` — Simplification and dead code agent

### External Standards
- OWASP Top 10: https://owasp.org/www-project-top-ten/
- CWE Top 25: https://cwe.mitre.org/top25/
- Electron Security: https://www.electronjs.org/docs/latest/tutorial/security
- React Performance: https://react.dev/reference/react/memo
- TypeScript Strict: https://www.typescriptlang.org/tsconfig#strict
