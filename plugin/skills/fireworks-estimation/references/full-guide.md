# fireworks-estimation — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 3. Sprint Planning

### Velocity Calculation

```
Velocity = Rolling average of last 3 sprints (completed points)
```

- Sprint 1: 24 points, Sprint 2: 28 points, Sprint 3: 22 points
- Velocity = (24 + 28 + 22) / 3 = **24.7 points/sprint**
- Round DOWN for planning: commit to **24 points**

New teams with no history: Start with **60% of theoretical capacity** for first 3 sprints.

### Capacity Planning

```
Capacity = Developer-Days x 6 productive hours/day
```

Why 6 hours, not 8?
- Meetings, standups, code reviews: ~1 hour
- Context switching, breaks, admin: ~1 hour
- 6 hours of focused coding is realistic

Adjustments:
- Holidays/PTO: subtract those days entirely
- On-call rotation: subtract 50% of on-call developer's capacity
- New team member: 50% capacity for first 2 sprints (onboarding)

### Sprint Length Selection

| Sprint Length | Best For | Project Level |
|---|---|---|
| **1 week** | Rapid prototyping, MVPs, hotfix cycles | L0-L1 |
| **2 weeks** | Standard development, most teams, balanced cadence | L2-L3 |
| **3 weeks** | Complex features, heavy integration, enterprise | L4 |

### Sprint Buffer

**Always reserve 20% capacity for unplanned work:**
- Bug fixes from production
- Urgent stakeholder requests
- Technical debt that blocks progress
- Infrastructure incidents

```
Committable Points = Velocity x 0.80
```

### Sprint Goal

Every sprint MUST have a single-sentence goal that describes the value delivered:

```
BAD:  "Complete stories 101-115"
GOOD: "Users can sign up, log in, and reset their password"
```

The sprint goal is the North Star. If a story does not contribute to the goal,
question whether it belongs in this sprint.

---

## 4. Complexity Assessment Matrix

Rate each factor 1-3, then sum for total complexity score:

| Factor | Low (1) | Medium (2) | High (3) |
|---|---|---|---|
| **Technical novelty** | Known tech, proven patterns | Some unknowns, partial experience | New technology, no team experience |
| **Dependencies** | None or internal only | 1-2 external dependencies | 3+ external dependencies |
| **Domain complexity** | Well-understood rules | Some ambiguity, edge cases | Complex business rules, regulations |
| **Integration points** | 0-1 systems | 2-3 systems | 4+ systems |
| **Data migration** | None required | Simple schema changes | Complex transformation, legacy data |

### Scoring

| Total Score | Complexity Level | Recommended Action |
|---|---|---|
| **5-7** | Low | Standard estimation, minimal buffer |
| **8-11** | Medium | Add spike tasks, increase risk buffer |
| **12-15** | High | Require proof-of-concept, phased delivery, senior review |

### Usage

Run the complexity assessment BEFORE estimation begins. High-complexity items need:
1. A spike (research task) before committing to estimates
2. Phased delivery plan (deliver incrementally, not big-bang)
3. Senior engineer review of estimates
4. Explicit risk register entries

---

## 5. Risk Categories and Buffers

### Risk Identification

For every project or epic, evaluate these four risk dimensions:

**Technical Risk (1.3-2.0x buffer)**
- New framework or language the team has not used
- Performance requirements not yet validated
- Complex integration with external systems
- Architecture changes or migrations
- Concurrency, caching, or distributed system challenges

**Domain Risk (1.4-2.5x buffer)**
- Requirements are incomplete or ambiguous
- Business rules are complex with many edge cases
- Regulatory or compliance requirements
- Domain experts are unavailable or inconsistent
- Data model is not finalized

**External Risk (1.5-3.0x buffer)**
- Third-party API availability or reliability unknown
- Vendor contracts or SLAs not finalized
- Hardware or infrastructure procurement delays
- Regulatory approval timelines
- Open-source dependency stability

**Resource Risk (1.2-2.0x buffer)**
- Key team members may become unavailable
- Skill gaps require training or hiring
- Team is distributed across time zones
- Competing priorities from other projects
- Onboarding new team members during the project

### Compound Risk Calculation

When multiple risk categories apply, multiply the factors:

```
Total Risk Buffer = Technical x Domain x External x Resource

Example:
  Technical: 1.3 (some new tech)
  Domain: 1.5 (ambiguous requirements)
  External: 1.0 (no external deps)
  Resource: 1.2 (one team member on PTO)

  Total: 1.3 x 1.5 x 1.0 x 1.2 = 2.34x

  If base estimate = 40 hours
  Buffered estimate = 40 x 2.34 = 93.6 hours
```

---

## 6. Communication Format

### Estimate Delivery Template

Always communicate estimates in this format:

```
[ESTIMATE] 47 hours (+/- 12 hours) assuming stable requirements and API availability

BREAKDOWN:
| Task                        | O    | R    | P    | PERT  | Risk  | Buffered |
|-----------------------------|------|------|------|-------|-------|----------|
| Login form component        | 2h   | 4h   | 8h   | 4.3h  | 1.0x  | 4.3h     |
| JWT token generation        | 2h   | 3h   | 6h   | 3.3h  | 1.3x  | 4.3h     |
| Password hashing            | 1h   | 2h   | 4h   | 2.2h  | 1.0x  | 2.2h     |
| Auth API endpoint           | 2h   | 3h   | 5h   | 3.2h  | 1.2x  | 3.8h     |
| Token refresh logic         | 3h   | 4h   | 8h   | 4.5h  | 1.3x  | 5.9h     |
| Auth middleware              | 1h   | 2h   | 5h   | 2.3h  | 1.0x  | 2.3h     |
| Integration testing         | 3h   | 5h   | 10h  | 5.5h  | 1.2x  | 6.6h     |
| SUBTOTAL                    |      |      |      |       |       | 29.4h    |
| Integration buffer (25%)    |      |      |      |       |       | 7.4h     |
| TOTAL                       |      |      |      |       |       | 36.8h    |

CONFIDENCE: 75% (Medium) — some unknowns around token refresh edge cases
ASSUMPTIONS:
  - Requirements for password policy are finalized
  - Third-party OAuth not in scope (Phase 2)
  - Existing user table schema is sufficient
RISKS:
  - Token refresh may require WebSocket for real-time invalidation
  - Password policy requirements may change
DEPENDENCIES:
  - User service API must be deployed to staging
  - Database migration for sessions table
UNKNOWNS:
  - Multi-device session management approach TBD
```

### Confidence Levels

| Level | Percentage | Meaning |
|---|---|---|
| Very High | 90-100% | Done this exact thing before, no unknowns |
| High | 75-89% | Similar work done, few minor unknowns |
| Medium | 50-74% | Some unknowns, reasonable assumptions |
| Low | 25-49% | Significant unknowns, needs spike first |
| Very Low | < 25% | Too many unknowns — do NOT commit to this estimate |

### Re-Estimate Triggers

An estimate becomes stale and MUST be re-evaluated when:

1. **Requirements change** — any scope addition, removal, or modification
2. **Unknown resolved** — discovery changes assumptions (up or down)
3. **Overrun > 20%** — actual effort exceeds estimate by more than 20%
4. **Dependency change** — upstream or downstream system changes
5. **Team change** — different person will do the work
6. **Technology change** — different tool, library, or approach chosen

---

## 7. Anti-Patterns

These are the most common estimation failures. Watch for them and call them out.

### "2 Hours" Syndrome
- **What**: Developer gives a quick number without actual estimation
- **Why it fails**: No decomposition, no risk assessment, anchoring on best-case
- **Fix**: Require the 6-step protocol. No shortcuts.

### Secret Padding
- **What**: Adding hidden buffer without transparency
- **Why it fails**: Destroys trust, makes tracking impossible, compounds across tasks
- **Fix**: All buffers explicitly stated and justified

### Ignoring Integration Time
- **What**: Estimating components in isolation, forgetting the glue
- **Why it fails**: Integration is where 50% of bugs live
- **Fix**: Always add 20-30% integration buffer as a separate line item

### Forgetting Testing Time
- **What**: Estimating only development, not testing
- **Why it fails**: Testing typically takes 30-50% of development time
- **Fix**: Include unit tests, integration tests, and manual QA in every estimate

### No Confidence Level
- **What**: Giving a number without stating how sure you are
- **Why it fails**: Stakeholders assume 100% confidence
- **Fix**: Always state confidence percentage with every estimate

### Single-Point Estimates
- **What**: "It will take 5 days"
- **Why it fails**: No range communicates false precision
- **Fix**: Always use 3-point (O/R/P) estimates

### Anchoring Bias
- **What**: First number mentioned dominates all subsequent estimates
- **Why it fails**: Cognitive bias prevents objective re-assessment
- **Fix**: Estimate independently before discussing. Use planning poker.

---

## 8. Burndown Tracking

### Daily Tracking

Track points completed vs points planned each day of the sprint:

```
Day  | Planned Remaining | Actual Remaining | Delta
-----|-------------------|------------------|------
  1  | 22                | 24               | +2 (behind)
  2  | 20                | 21               | +1
  3  | 18                | 18               |  0 (on track)
  4  | 16                | 14               | -2 (ahead)
  5  | 14                | 15               | +1
```

**Warning signals:**
- Delta growing for 3+ consecutive days: escalate immediately
- Actual remaining increases (scope added mid-sprint): flag scope creep
- No movement for 2+ days: developer may be blocked

### Sprint-Level Metrics

Track over 3-5 sprints for trend analysis:

| Metric | How to Calculate | Healthy Range |
|---|---|---|
| **Velocity** | Points completed per sprint | Stable +/- 15% |
| **Commitment ratio** | Completed / Committed | > 85% |
| **Scope change** | Points added after sprint start | < 10% of commitment |
| **Carry-over** | Stories not completed, moved to next sprint | < 2 stories |
| **Blocked time** | Total days stories were blocked | < 10% of sprint |
| **Bug ratio** | Bug points / Total points | < 20% |

### Release-Level Tracking

For multi-sprint releases (epics, milestones):

```
Epic: User Authentication System
Total Points: 89
Completed: 52 (58%)
Remaining: 37
Sprints Elapsed: 3
Velocity: 18 pts/sprint
Projected Completion: Sprint 5 (2 more sprints)
Confidence: 70% — token refresh spike may add scope
```

---

## 9. Verification Gates

Before finalizing ANY estimate, pass these gates:

### Gate 1: Three-Point Values
- [ ] Every task has Optimistic, Realistic, and Pessimistic values
- [ ] No O value is zero (nothing takes zero time)
- [ ] No P value exceeds 3x R value (decompose if it does)
- [ ] PERT formula applied correctly: (O + 4R + P) / 6

### Gate 2: Story Size Limit
- [ ] No story exceeds 8 points
- [ ] Stories > 8 have been broken into smaller stories
- [ ] Each sub-story is independently deliverable
- [ ] Sum of sub-stories may exceed original (that is expected and correct)

### Gate 3: Risk Buffers
- [ ] All four risk categories evaluated (technical, domain, external, resource)
- [ ] Buffer multipliers explicitly stated per task or per epic
- [ ] Compound risk calculated where multiple categories apply
- [ ] Buffers are transparent, not hidden

### Gate 4: Integration Buffer
- [ ] Integration buffer added as separate line item (20-30%)
- [ ] Buffer percentage justified based on integration complexity
- [ ] Testing time included in estimates (not just coding time)
- [ ] Deployment and environment setup time considered

---

## 10. INVARIANTS

These rules are absolute. They cannot be overridden by context, urgency, or authority.

1. **Never give single-point estimates** — always provide O/R/P and the PERT calculation.
   "It will take 5 days" is NEVER acceptable. "3-7 days, expected 5" is the minimum.

2. **Never size a story > 8 points without breaking it down** — large stories hide
   complexity. If you cannot break it down, you do not understand it well enough to estimate.

3. **Always include integration buffer (20-30%)** — this is not optional padding.
   Integration is where components meet, and components never meet cleanly.

4. **Always state confidence level with every estimate** — stakeholders deserve to know
   how certain you are. A 50% confidence estimate requires different planning than 90%.

5. **Re-estimate when unknowns are resolved** — estimates are living documents. When you
   learn something new that changes your assumptions, update the estimate immediately.
   Going silent on stale estimates is professional malpractice.

---

## Quick Reference Card

```
ESTIMATION FORMULA:
  PERT = (O + 4R + P) / 6
  SD   = (P - O) / 6
  68% confidence = PERT +/- 1 SD
  95% confidence = PERT +/- 2 SD

FIBONACCI POINTS:
  1 = trivial (<2h)
  2 = simple (2-4h)
  3 = moderate (4-8h)
  5 = complex (1-2d)
  8 = very complex (2-3d)
  13+ = BREAK IT DOWN

RISK MULTIPLIERS:
  Low      = 1.0-1.2x
  Medium   = 1.2-1.5x
  High     = 1.5-2.0x
  Critical = 2.0-3.0x

SPRINT RULES:
  Velocity = 3-sprint rolling average
  Capacity = dev-days x 6h/day
  Buffer   = 20% for unplanned work
  Goal     = 1 sentence of value

INTEGRATION BUFFER:
  Simple   = 20%
  Moderate = 25%
  Complex  = 30%

COMMUNICATION:
  [ESTIMATE] (+/- [UNCERTAINTY]) assuming [ASSUMPTIONS]
```

---

### Context Clear Between Phases
For L3/L4 estimates:
- Estimation phase produces sizing doc
- CLEAR context
- Verification phase re-reads sizing doc with fresh eyes
- This prevents anchoring bias from conversation history

---

## References

For detailed methodology, see:

- [PERT Method](references/pert-method.md) -- three-point estimation formulas, weighted averages, confidence intervals
- [Risk Assessment](references/risk-assessment.md) -- risk matrices, probability scoring, mitigation planning
- [Sizing Guide](references/sizing-guide.md) -- story point calibration, T-shirt sizing, complexity factors
- [Sprint Metrics](references/sprint-metrics.md) -- velocity tracking, burndown analysis, capacity planning

---

## Related Skills

- `fireworks-workflow` — L0-L4 levels map to complexity
- `fireworks-architect` — architecture drives estimates
- `fireworks-taskmaster` — task decomposition for accurate sizing

---

## Scope Boundaries

- **MINIMUM**: Always use 3-point estimate for tasks > 2 hours.
- **MAXIMUM**: Do not estimate beyond current quarter.
