# Risk Assessment — Complete Reference

Comprehensive guide to risk identification, probability-impact analysis, mitigation
strategies, risk registers, Monte Carlo concepts, and historical accuracy tracking.

---

## Risk Identification Checklist

Evaluate every project against these 20 common software risks. Score each as
Not Applicable (0), Low (1), Medium (2), or High (3).

### Technical Risks (T1-T5)

**T1: Technology Maturity**
- Is the team using any technology for the first time?
- Is any core dependency < 1 year old or < v1.0?
- Are there known stability issues with any library?

**T2: Architecture Complexity**
- Does the solution require a new architectural pattern?
- Are there more than 3 integration points between systems?
- Does the system need to handle concurrent writes or real-time data?

**T3: Performance Requirements**
- Are there specific latency or throughput targets?
- Does the system handle large datasets (>100K records)?
- Are there real-time processing requirements?

**T4: Security Requirements**
- Does the system handle PII or financial data?
- Are there encryption or compliance requirements (PCI, HIPAA, GDPR)?
- Is authentication/authorization a core feature?

**T5: Infrastructure / Deployment**
- Is the deployment environment new or unfamiliar?
- Are there specific uptime or availability requirements?
- Does the release require data migration or schema changes?

### Domain Risks (D1-D5)

**D6: Requirements Clarity**
- Are all user stories defined with acceptance criteria?
- Are there any TBD items in the requirements?
- Have edge cases been discussed and documented?

**D7: Business Logic Complexity**
- Are there complex calculation rules (tax, pricing, inventory)?
- Do business rules vary by region, user type, or context?
- Are there complex state machines or workflow logic?

**D8: Data Model Stability**
- Is the data model finalized?
- Are there data migration requirements from legacy systems?
- Could the schema change during development?

**D9: User Experience Ambiguity**
- Are wireframes/designs finalized?
- Are interaction patterns well-defined?
- Are accessibility requirements clear?

**D10: Regulatory / Compliance**
- Are there legal requirements affecting the feature?
- Do outputs need to comply with specific standards?
- Are audit trails or reporting mandated?

### External Risks (E1-E5)

**E11: Third-Party API Dependencies**
- Does the feature depend on external APIs?
- Are SLAs defined for those APIs?
- What happens when the external API is down?

**E12: Vendor / Partner Dependencies**
- Are you waiting on deliverables from an external vendor?
- Is the vendor's timeline aligned with yours?
- Is there a fallback if the vendor fails to deliver?

**E13: Hardware / Device Dependencies**
- Does the feature depend on specific hardware (printers, scanners,  terminals)?
- Is the hardware available for testing?
- Are driver compatibility issues possible?

**E14: Data Source Reliability**
- Is the input data reliable and well-formatted?
- Are there data quality issues that need handling?
- Could the data source change schema or format?

**E15: Environmental Factors**
- Are there network connectivity assumptions?
- Does the system need to work offline?
- Are there time zone or locale considerations?

### Resource Risks (R1-R5)

**R16: Team Availability**
- Are key team members available for the full project?
- Is anyone on the team at risk of leaving or being reassigned?
- Are there vacation or holiday conflicts?

**R17: Skill Gaps**
- Does the team have expertise in all required technologies?
- Is training needed before work can begin?
- Are there specialized skills that only one person has (bus factor = 1)?

**R18: Stakeholder Availability**
- Is the product owner available for timely decisions?
- Can domain experts answer questions within 24 hours?
- Are design reviews and approvals timely?

**R19: Competing Priorities**
- Is the team working on multiple projects simultaneously?
- Are there other deadlines that could steal focus?
- Is there risk of the team being pulled for emergency work?

**R20: Knowledge Transfer**
- Is there adequate documentation of existing systems?
- Are there tribal knowledge dependencies?
- Can new team members ramp up without excessive hand-holding?

---

## Risk Probability x Impact Matrix

### Probability Scale

| Level | Probability | Description |
|---|---|---|
| 1 — Rare | < 10% | Very unlikely, has never happened before |
| 2 — Unlikely | 10-25% | Could happen but not expected |
| 3 — Possible | 25-50% | Has happened before, could happen again |
| 4 — Likely | 50-75% | Expected to happen at some point |
| 5 — Almost Certain | > 75% | Will almost definitely happen |

### Impact Scale

| Level | Impact | Schedule | Cost | Quality |
|---|---|---|---|---|
| 1 — Negligible | Minimal | < 1 day delay | < 5% budget | Cosmetic only |
| 2 — Minor | Small | 1-3 day delay | 5-10% budget | Minor functionality |
| 3 — Moderate | Significant | 3-7 day delay | 10-20% budget | Major feature degraded |
| 4 — Major | Large | 1-3 week delay | 20-40% budget | Critical feature affected |
| 5 — Catastrophic | Severe | > 3 week delay | > 40% budget | Project viability at risk |

### Risk Score Matrix

```
                    IMPACT
                1     2     3     4     5
           ┌─────┬─────┬─────┬─────┬─────┐
     5     │  5  │ 10  │ 15  │ 20  │ 25  │  Almost Certain
           ├─────┼─────┼─────┼─────┼─────┤
P    4     │  4  │  8  │ 12  │ 16  │ 20  │  Likely
R          ├─────┼─────┼─────┼─────┼─────┤
O    3     │  3  │  6  │  9  │ 12  │ 15  │  Possible
B          ├─────┼─────┼─────┼─────┼─────┤
     2     │  2  │  4  │  6  │  8  │ 10  │  Unlikely
           ├─────┼─────┼─────┼─────┼─────┤
     1     │  1  │  2  │  3  │  4  │  5  │  Rare
           └─────┴─────┴─────┴─────┴─────┘
```

### Risk Score Interpretation

| Score Range | Risk Level | Response |
|---|---|---|
| 1-4 | **Low** | Accept. Monitor passively. No buffer needed. |
| 5-9 | **Medium** | Mitigate. Add 1.2-1.5x buffer. Monitor weekly. |
| 10-15 | **High** | Active mitigation required. Add 1.5-2.0x buffer. Monitor daily. |
| 16-25 | **Critical** | Escalate immediately. Consider descoping. Add 2.0-3.0x buffer. |

---

## Mitigation Strategies per Risk Category

### Technical Risk Mitigation

| Strategy | When to Use | Cost |
|---|---|---|
| **Spike / Proof of Concept** | New technology, unproven approach | 1-3 days |
| **Incremental Architecture** | Complex system, many unknowns | 20% more time |
| **Fallback Technology** | Risky library, uncertain API | Keep Plan B ready |
| **Pair Programming** | Complex logic, single expert | 2x dev time on critical path |
| **Architecture Review** | Major design decisions | 2-4 hours per review |
| **Performance Budget** | Latency/throughput targets | Continuous monitoring |

### Domain Risk Mitigation

| Strategy | When to Use | Cost |
|---|---|---|
| **Requirements Workshop** | Ambiguous requirements | 0.5-1 day |
| **Prototype / Clickable Mock** | UX uncertainty | 1-3 days |
| **Edge Case Discovery Session** | Complex business rules | 2-4 hours |
| **Domain Expert Pairing** | Complex calculations | Ongoing availability |
| **Phased Delivery** | Large scope, evolving requirements | Smaller increments |
| **Acceptance Criteria Review** | Before sprint commitment | 1 hour per epic |

### External Risk Mitigation

| Strategy | When to Use | Cost |
|---|---|---|
| **API Contract Testing** | Third-party API dependency | 1-2 days setup |
| **Mock Services** | External service unreliable | 0.5-1 day per service |
| **Vendor SLA Agreement** | Vendor dependency | Contract negotiation |
| **Offline Capability** | Network unreliability | Significant (plan for it) |
| **Data Validation Layer** | Untrusted data source | 1-2 days |
| **Feature Flags** | Risky rollout | 0.5 day setup |

### Resource Risk Mitigation

| Strategy | When to Use | Cost |
|---|---|---|
| **Knowledge Sharing Sessions** | Bus factor = 1 | 2 hours/week |
| **Documentation Sprint** | Tribal knowledge risk | 1-2 days |
| **Cross-Training** | Single point of failure | Ongoing |
| **Buffer for Onboarding** | New team member | 50% capacity for 2 sprints |
| **Stakeholder Alignment Meeting** | Competing priorities | 1 hour/week |
| **Contractor Backup Plan** | Key person departure risk | Pre-sourced contacts |

---

## Risk Register Template

Maintain a living risk register for every project:

```
RISK REGISTER — [Project Name]
Last Updated: [Date]
Owner: [Project Lead]

| ID | Risk Description | Category | Prob | Impact | Score | Buffer | Mitigation | Status | Owner |
|----|-----------------|----------|------|--------|-------|--------|------------|--------|-------|
| R1 | New WebSocket library may have stability issues | Technical | 3 | 3 | 9 | 1.5x | Spike in Sprint 1, fallback to polling | Open | Dev A |
| R2 | Tax calculation rules not finalized | Domain | 4 | 4 | 16 | 2.0x | Workshop with accountant, phase delivery | Open | PM |
| R3 | Payment gateway API migration in Q2 | External | 3 | 4 | 12 | 1.8x | Abstract payment layer, mock testing | Monitoring | Dev B |
| R4 | Senior dev on PTO for 2 weeks in Sprint 3 | Resource | 5 | 2 | 10 | 1.3x | Knowledge transfer before PTO, pair programming | Accepted | Lead |
| R5 | Electron auto-update may break on Windows | Technical | 2 | 5 | 10 | 1.5x | Staged rollout, manual update fallback | Open | Dev C |

TOTAL RISK EXPOSURE: Sum of all (Score x Estimated Impact in Hours)
  R1: 9 x 8h = 72h
  R2: 16 x 16h = 256h
  R3: 12 x 12h = 144h
  R4: 10 x 6h = 60h
  R5: 10 x 10h = 100h
  TOTAL: 632 risk-hours

RECOMMENDED PROJECT BUFFER: 632 x 0.3 (30% of risk exposure) = ~190 hours
```

### Risk Register Maintenance

- **Review frequency**: Every sprint planning session
- **Update triggers**: New risk identified, risk materialized, risk mitigated, probability changed
- **Close a risk**: When mitigation is complete and risk is no longer relevant
- **Escalation rule**: Any risk with score > 15 must be escalated to project sponsor within 24 hours

---

## Monte Carlo Simulation Concepts

For L3-L4 projects with many tasks and significant uncertainty, Monte Carlo simulation
provides more robust estimates than simple PERT summation.

### Core Concept

Instead of calculating a single expected duration, Monte Carlo runs thousands of
simulated project schedules, each time randomly sampling from each task's probability
distribution. The result is a distribution of possible project durations.

### Simplified Process

1. For each task, define O, R, P (same as PERT inputs)
2. Assume a triangular or beta distribution for each task
3. Run 10,000 simulated schedules:
   - For each simulation, randomly pick a duration for each task from its distribution
   - Sum the durations along the critical path
   - Record the total project duration
4. Analyze the distribution of 10,000 totals

### Reading Results

```
Monte Carlo Results (10,000 simulations):

Percentile | Duration | Interpretation
-----------|----------|---------------
P10        | 85 hours | 10% chance of finishing this fast or faster
P25        | 95 hours | 25% chance
P50        | 110 hours| Median — 50% chance of finishing by this time
P75        | 128 hours| 75% chance
P90        | 145 hours| 90% chance (recommended for commitments)
P95        | 160 hours| 95% chance (recommended for contracts)

Mean: 112 hours
Std Dev: 22 hours
```

### When to Use Monte Carlo

- Projects with 20+ tasks on the critical path
- High-stakes deadlines with financial penalties
- Multiple dependent work streams
- When PERT alone feels insufficient due to correlated risks

### Practical Alternative

For most software projects, Monte Carlo is overkill. Instead, use this simplified approach:

```
Simple Range Estimation:
  Best Case  = Sum of all O values (never commit to this)
  Expected   = Sum of all PERT values
  Worst Case = Sum of all P values (plan for this to be safe)

  Commitment = Expected x Integration Buffer x Compound Risk Buffer
```

This gives you a range that captures most of Monte Carlo's value without the complexity.

---

## Historical Accuracy Tracking

Track estimation accuracy over time to improve future estimates.

### Tracking Template

```
ESTIMATION ACCURACY LOG

| Sprint | Story | Points | Estimated Hours | Actual Hours | Ratio | Category |
|--------|-------|--------|----------------|--------------|-------|----------|
| S10 | S-201 | 3 | 6 | 7 | 1.17 | Frontend |
| S10 | S-202 | 5 | 12 | 10 | 0.83 | Full-stack |
| S10 | S-203 | 8 | 18 | 24 | 1.33 | Integration |
| S10 | S-204 | 2 | 4 | 3 | 0.75 | Config |
| S10 | S-205 | 3 | 6 | 8 | 1.33 | Backend |
| S11 | S-206 | 5 | 12 | 14 | 1.17 | Frontend |
| S11 | S-207 | 3 | 6 | 5 | 0.83 | Testing |
| S11 | S-208 | 8 | 18 | 22 | 1.22 | Full-stack |
```

### Accuracy Metrics

```
Overall Accuracy:
  Mean Ratio: 1.08 (average 8% underestimate)
  Median Ratio: 1.17
  Std Dev: 0.22

By Category:
  Frontend:    Mean 1.17 (tend to underestimate by 17%)
  Full-stack:  Mean 1.03 (accurate)
  Integration: Mean 1.33 (significantly underestimate by 33%)
  Backend:     Mean 1.33 (underestimate by 33%)
  Config:      Mean 0.75 (overestimate by 25%)
  Testing:     Mean 0.83 (overestimate by 17%)

By Story Size:
  1-2 points: Mean 0.80 (overestimate small stories)
  3 points:   Mean 1.22 (underestimate moderate stories)
  5 points:   Mean 1.00 (accurate for complex stories)
  8 points:   Mean 1.28 (underestimate very complex stories)
```

### Calibration Actions

Based on accuracy data, apply calibration factors to future estimates:

```
CALIBRATION FACTORS (apply to PERT result):

  Frontend stories:    x 1.15 (add 15%)
  Full-stack stories:  x 1.00 (no adjustment)
  Integration stories: x 1.30 (add 30%)
  Backend stories:     x 1.30 (add 30%)
  Config stories:      x 0.80 (reduce 20%)
  Testing stories:     x 0.85 (reduce 15%)

  Small stories (1-2): x 0.85 (reduce 15%)
  Large stories (8):   x 1.25 (add 25%)
```

### Accuracy Improvement Process

1. **Monthly review**: Analyze accuracy data from last 4 sprints
2. **Identify patterns**: Which categories or sizes are consistently off?
3. **Update calibration factors**: Adjust multipliers based on data
4. **Update anchor stories**: Replace anchors that no longer represent accurate sizing
5. **Team discussion**: Share findings in retrospective, align on adjustments
6. **Track improvement**: Plot accuracy trend over time — should converge toward 1.0

### Accuracy Targets

| Maturity Level | Accuracy Range | Description |
|---|---|---|
| **Novice** | 0.5 - 2.0x | Wild variation, just starting to track |
| **Developing** | 0.7 - 1.5x | Patterns emerging, calibration beginning |
| **Proficient** | 0.8 - 1.3x | Consistent within 30%, calibration applied |
| **Expert** | 0.85 - 1.15x | Within 15% consistently, rare surprises |

Most solo developers or small teams can reach "Proficient" within 10 sprints
of deliberate tracking. "Expert" requires discipline and a stable team.

---

## Risk-Adjusted Timeline Example

Putting it all together for a real project:

```
PROJECT: Inventory Management Module for your Electron project

TASK ESTIMATES (after PERT calculation):
| Task | PERT | Category | Risk Buffer | Calibration | Final |
|------|------|----------|-------------|-------------|-------|
| Product CRUD | 12h | Full-stack | 1.0x | 1.00x | 12.0h |
| Barcode scanning | 8h | Integration | 1.5x | 1.30x | 15.6h |
| Stock level tracking | 10h | Backend | 1.2x | 1.30x | 15.6h |
| Low stock alerts | 6h | Full-stack | 1.0x | 1.00x | 6.0h |
| Purchase orders | 14h | Full-stack | 1.3x | 1.00x | 18.2h |
| Receiving workflow | 10h | Frontend | 1.2x | 1.15x | 13.8h |
| Inventory reports | 8h | Frontend | 1.0x | 1.15x | 9.2h |
| Data import (CSV) | 6h | Integration | 1.3x | 1.30x | 10.1h |

SUBTOTAL: 100.5 hours
INTEGRATION BUFFER (25%): 25.1 hours
TOTAL: 125.6 hours

AT 6 productive hours/day: 20.9 days = ~4.2 weeks
AT velocity 24 points/sprint (2-week sprints): ~3 sprints

CONFIDENCE: 70% (Medium)
  - Barcode scanning library is new (spike recommended)
  - Purchase order workflow requirements still evolving
  - CSV import format not yet finalized

COMMITMENT RECOMMENDATION:
  Target: 3.5 sprints (7 weeks)
  Aggressive: 3 sprints (6 weeks) — 55% confidence
  Safe: 4 sprints (8 weeks) — 85% confidence
```
