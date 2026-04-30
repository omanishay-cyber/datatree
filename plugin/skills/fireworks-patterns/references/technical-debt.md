# Technical Debt Management — Complete Reference

## What Is Technical Debt?

Technical debt is the implied cost of future rework caused by choosing an
expedient solution now instead of a better approach that would take longer.
Like financial debt, it accrues **interest** — the longer it remains unpaid,
the more it costs to maintain, extend, and debug the affected code.

The critical insight: **debt is not inherently bad.** Strategic debt — taken
deliberately with a repayment plan — is a valid engineering tool. Reckless
debt — taken through ignorance or negligence — is what destroys codebases.

---

## The Debt Quadrant (Martin Fowler's Model)

### Deliberate + Reckless: "We don't have time for tests"

**Characteristics:**
- Team knows best practices but deliberately ignores them
- Driven by deadline pressure with no repayment plan
- Often repeated across multiple deadlines

**Examples:**
- Shipping without any test coverage
- Hardcoding values instead of using configuration
- Copy-pasting code instead of extracting shared logic
- Ignoring security best practices

**Danger Level:** EXTREME. This debt compounds rapidly and often leads to
catastrophic failures (data loss, security breaches, production outages).

**Action:** Stop immediately. This is not technical debt — this is negligence.
Add tests, fix security issues, extract duplicated code. No excuses.

### Deliberate + Prudent: "Ship now, refactor in sprint 2"

**Characteristics:**
- Team understands the trade-off being made
- Debt is documented with a specific repayment plan
- Decision is conscious and time-boxed

**Examples:**
- Using a simple data structure knowing it won't scale past 10K records
- Inline styles instead of a design system (for a prototype)
- Monolithic service that will be split later
- Simplified error handling that will be enhanced

**Danger Level:** MANAGEABLE if the repayment plan is followed.

**Action:** Document with a TD-XXX identifier. Calculate interest. Schedule
repayment. Follow through.

### Inadvertent + Reckless: "What's a design pattern?"

**Characteristics:**
- Team lacks knowledge to recognize the debt
- Poor practices applied without awareness
- No code review or mentorship process

**Examples:**
- God classes with thousands of lines
- No separation of concerns
- Global mutable state everywhere
- No error handling at all

**Danger Level:** HIGH. The team doesn't even know the debt exists, so it
will never be repaid voluntarily.

**Action:** Invest in training, code reviews, and pair programming. Bring in
experienced developers for architecture review.

### Inadvertent + Prudent: "Now we know how we should have done it"

**Characteristics:**
- Team did their best with available knowledge
- Learning occurred through building the system
- The "better way" was only obvious in hindsight

**Examples:**
- Choosing a framework that turned out to be wrong for the use case
- Data model that doesn't match real-world usage patterns
- Architecture that doesn't scale as expected
- API design that doesn't serve actual client needs

**Danger Level:** NORMAL. This is how engineering works — you learn by doing.

**Action:** Refactor when the affected code is next modified (Boy Scout Rule).
If the debt is significant, plan a dedicated refactoring sprint.

---

## Debt Lifecycle — Detailed

### Phase 1: Identify

**How to find debt:**

| Method | What It Finds |
|---|---|
| Code review | Design issues, style violations, missing tests |
| Static analysis | Complexity, duplication, dependency cycles |
| Bug patterns | Areas with repeated bugs indicate structural problems |
| Build metrics | Slow builds, flaky tests, long CI pipelines |
| Developer feedback | "This area is scary to change" |
| Onboarding friction | "This took me 3 days to understand" |
| Performance monitoring | Slow queries, memory leaks, CPU spikes |

**Common debt types:**

| Type | Symptoms |
|---|---|
| Architecture debt | Tight coupling, circular dependencies, wrong boundaries |
| Code debt | Duplication, complexity, dead code, poor naming |
| Test debt | Missing tests, flaky tests, slow test suite |
| Documentation debt | Missing docs, outdated docs, tribal knowledge |
| Infrastructure debt | Manual deployments, no monitoring, legacy tooling |
| Dependency debt | Outdated packages, unmaintained dependencies |

### Phase 2: Document (TD-XXX)

Every identified debt item gets a formal entry:

```
ID:          TD-042
Title:       Inventory sync uses O(n^2) comparison algorithm
Location:    src/services/sync.ts:145-210
Type:        Code debt (performance)
Quadrant:    Inadvertent + Prudent
Created:     2024-06-15
Author:      the maintainer
Description: The inventory sync compares each remote item against all local
             items using nested loops. Works fine for <500 items but becomes
             unacceptably slow beyond 2000 items.
Impact:      Sync takes 45 seconds for 3000 items. Users report timeout errors.
Root Cause:  Original implementation assumed max 500 items per store.
Solution:    Build a Map of local items by ID for O(1) lookup. Expected
             reduction from 45s to <1s for 3000 items.
```

### Phase 3: Calculate Interest

**Formula:** `Annual Interest = Cost per Incident x Incidents per Year`

**Cost per incident includes:**
- Developer time investigating / working around the issue
- Lost productivity from other developers blocked
- Customer impact (support tickets, lost sales, reputation)
- Opportunity cost (time NOT spent on features)

**Worked examples:**

```
TD-042: Slow Inventory Sync
  Cost per incident: 2 hours developer time + 30 min user waiting = 2.5 hours
  Incidents per year: 365 days x 1 sync/day = 365 incidents
  Annual Interest: 2.5 x 365 = 912.5 hours/year
  (This is an extreme case — should be fixed immediately)

TD-043: Duplicated Validation Logic
  Cost per incident: 0.5 hours per bug fix (must fix in 3 places)
  Incidents per year: ~12 validation bugs per year
  Annual Interest: 0.5 x 12 = 6 hours/year

TD-044: Missing TypeScript Strict Mode
  Cost per incident: 1 hour per type-related bug
  Incidents per year: ~24 type bugs per year
  Annual Interest: 1 x 24 = 24 hours/year
```

### Phase 4: Prioritize

**Formula:** `Priority Score = Annual Interest / Payoff Effort`

**Payoff effort** = estimated hours to fully resolve the debt.

```
TD-042: Priority = 912.5 / 16 = 57.0 → HIGH (pay ASAP)
TD-043: Priority = 6 / 4 = 1.5 → LOW (pay when convenient)
TD-044: Priority = 24 / 40 = 0.6 → LOW (but consider long-term benefits)
```

**Priority thresholds:**

| Score | Priority | Timeline | Action |
|---|---|---|---|
| > 10 | HIGH | Pay within 30 days | Schedule in next sprint |
| 3-10 | MEDIUM | Pay within 1-4 months | Add to backlog with deadline |
| < 3 | LOW | Pay when convenient | Address when touching that code |

**Override rules:**
- Security debt is always HIGH regardless of score
- Debt blocking new features gets bumped one level
- Debt causing data loss is always immediate

### Phase 5: Plan Payment

**Payment strategies:**

#### The Sprint 0 Rule
Reserve the first sprint of each quarter for debt payment only. No new
features. Focus entirely on paying down HIGH and MEDIUM debt.

#### The 20% Rule
Allocate 20% of every sprint's capacity to debt payment. This prevents
debt from accumulating while maintaining feature velocity.

#### The Boy Scout Rule
"Leave the code better than you found it." When touching code for a feature
or bug fix, improve nearby debt. Small, continuous improvements.

#### The Strangler Fig
For large architectural debt, build the new system alongside the old one.
Gradually route traffic/usage to the new system. Remove the old system
when it has zero usage. Named after the strangler fig tree that grows
around its host tree.

#### The Mikado Method
For complex refactoring with many dependencies:
1. Try the change
2. If it breaks, revert and note what broke
3. Fix the dependencies first
4. Try the change again
5. Repeat until it works cleanly

### Phase 6: Verify

After paying debt:
- [ ] All existing tests still pass
- [ ] New tests cover the refactored code
- [ ] Performance metrics are equal or better
- [ ] No new debt was introduced
- [ ] Debt register is updated (status: PAID, date, who)
- [ ] Team is notified of the change

---

## Debt Register Template

Maintain a living document (Markdown, spreadsheet, or issue tracker):

```markdown
# Technical Debt Register

| ID | Title | Priority | Interest (h/yr) | Effort (h) | Score | Status | Due |
|---|---|---|---|---|---|---|---|
| TD-042 | Slow sync algorithm | HIGH | 912 | 16 | 57.0 | IN PROGRESS | 2024-07-01 |
| TD-044 | Missing strict mode | MEDIUM | 24 | 40 | 0.6 | BACKLOG | 2024-Q4 |
| TD-043 | Duplicate validation | LOW | 6 | 4 | 1.5 | BACKLOG | — |
| TD-041 | Hardcoded API URL | LOW | 2 | 1 | 2.0 | PAID | 2024-06-10 |
```

---

## Communicating Debt to Non-Technical Stakeholders

### Analogy: The House

"Technical debt is like deferred maintenance on a house. You can skip painting
for a year — that saves time and money now. But after 5 years, the wood starts
rotting, and now you need to replace the siding, not just repaint it. The
longer you wait, the more expensive the fix."

### Speak in Business Terms

Instead of: "We need to refactor the sync module"
Say: "The sync system slows down as inventory grows. At current growth rate,
it will become unusable in 6 months. Fixing it now takes 2 days. Fixing it
after it breaks will take 2 weeks and cause store downtime."

### Show the Numbers

```
Current cost: 912 hours/year = $91,200 at $100/hour
Fix cost: 16 hours = $1,600
ROI: $91,200 / $1,600 = 57x return on investment
Payback period: 6.4 days
```

### Use the Quadrant

Show stakeholders which quadrant their debt falls in. Deliberate+Prudent is
acceptable. Deliberate+Reckless is not. This reframes the conversation from
"should we pay debt?" to "what kind of debt is this?"

---

## Budget Enforcement Rules

### Per-Module Limits
- Maximum 10 open debt items per module
- If a module hits 10, no new features until debt count drops below 7
- This creates natural pressure to pay debt as it accumulates

### Per-Priority Limits
- Maximum 3 HIGH priority items at any time
- If a 4th HIGH appears, the oldest HIGH must be resolved first
- HIGH items that age past 30 days escalate to team lead

### Sprint Allocation
- Minimum 20% of sprint capacity for debt payment (non-negotiable)
- Carry-over: unused debt capacity does NOT roll to next sprint
- Overtime: debt payment does NOT count toward feature velocity

### PR Requirements
- Any PR that adds debt must include a TD-XXX entry in the description
- Reviewer must verify the debt is documented and interest is estimated
- PRs that reduce debt should be celebrated and fast-tracked for review

---

## Metrics

### Debt Ratio
```
Debt Ratio = Total Payoff Effort / Total Codebase Size (in developer-weeks)
Target: < 15%
Warning: 15-30%
Critical: > 30%
```

### Interest Rate
```
Interest Rate = Total Annual Interest / Total Development Capacity
Target: < 10%
Warning: 10-25%
Critical: > 25% (you're spending more time on debt interest than features)
```

### Payback Period
```
Payback Period = Total Payoff Effort / Allocated Debt Capacity per Sprint
Target: < 6 months to pay all current debt
Warning: 6-12 months
Critical: > 12 months (debt is growing faster than you can pay it)
```

### Trend Tracking
- Track total debt items, total interest, and debt ratio over time
- Healthy: all three metrics stable or declining
- Warning: any metric increasing for 3+ sprints
- Critical: all three increasing simultaneously

---

## Anti-Patterns in Debt Management

### "We'll fix it later" (and never do)
Deliberate debt without a repayment plan is not strategic — it's negligence.
Always attach a deadline to deliberate debt.

### Debt bankruptcy
Declaring "we'll rewrite everything" is almost always wrong. Rewrites take
3-5x longer than estimated and introduce new bugs. Incremental improvement
(Strangler Fig, Mikado Method) is almost always better.

### Invisible debt
Debt that isn't tracked doesn't get paid. Every debt item needs an ID, an
interest estimate, and a priority score. "Everyone knows the auth module is
bad" is not debt management.

### Gold plating as debt payment
Rewriting working code to make it "cleaner" without measurable benefit is
not debt payment — it's gold plating. Debt payment must reduce measurable
cost (bugs, developer time, performance).

### Debt as punishment
Never frame debt payment as undesirable work. Paying debt is professional
engineering practice. Frame it as investment, not punishment.
