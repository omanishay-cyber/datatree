# Prioritization Frameworks — Fireworks Taskmaster

Decision frameworks for ordering tasks, features, and backlog items. Use these when you have more work than capacity and need to decide what to do first.

**Quick rule: If you can only pick one framework, use MoSCoW.**

---

## MoSCoW Framework

MoSCoW categorizes work into four buckets based on necessity. It is the default framework for sprint planning and feature prioritization.

### Categories

| Category | Meaning | Sprint Allocation |
|---|---|---|
| **Must Have** | System does not work without this. Non-negotiable. | 60% of capacity |
| **Should Have** | Important but not critical. Workarounds exist. | 20% of capacity |
| **Could Have** | Nice to have. Improves experience but not essential. | 20% of capacity (buffer) |
| **Won't Have** | Explicitly out of scope for this sprint. Parked. | 0% — backlog only |

### Decision Tree

```
START: Is this item blocking the system from functioning?
  │
  ├── YES → MUST HAVE
  │
  └── NO → Has the user explicitly requested this for the current milestone?
            │
            ├── YES → SHOULD HAVE
            │
            └── NO → Would this noticeably improve the user experience?
                      │
                      ├── YES → COULD HAVE
                      │
                      └── NO → WON'T HAVE (this time)
```

### Tie-Breaking Within a Category

When multiple items share the same MoSCoW category, break ties using:

1. **Dependencies** — Items that unblock other items go first
2. **User pain** — Items that fix the most painful user experience go first
3. **Effort** — Among equally painful items, do the quickest one first (momentum)
4. **Risk** — Items with technical uncertainty go first (fail fast)

### MoSCoW Worked Example

```
Feature: Inventory Management v2

MUST HAVE (60% = 108 min):
  [M] T-001 Product CRUD operations (30min)
  [M] T-002 Search and filter products (25min)
  [M] T-003 Stock level tracking (25min)
  [M] T-004 Low stock alerts (25min)
  Subtotal: 105 min — fits within 108 min allocation

SHOULD HAVE (20% = 36 min):
  [S] T-005 Barcode scanner integration (30min)
  Subtotal: 30 min — fits within 36 min allocation

COULD HAVE (20% = 36 min buffer):
  [C] T-006 Bulk import from CSV (25min)
  [C] T-007 Product image thumbnails (20min)
  Subtotal: 45 min — over by 9 min, pick one: T-006 (more value)

WON'T HAVE (backlog):
  [W] T-008 AI-powered reorder suggestions
  [W] T-009 Supplier portal integration
```

---

## RICE Score Framework

RICE quantifies priority with a numeric score. Use it when MoSCoW alone does not resolve ordering within a category, or when stakeholders need data-driven prioritization.

### Formula

```
RICE Score = (Reach x Impact x Confidence) / Effort
```

### Factor Definitions

| Factor | Description | Scale |
|---|---|---|
| **Reach** | How many users or sessions will this affect per month? | 1-10 (1=few, 10=everyone) |
| **Impact** | How much will this improve things for those users? | 0.25=minimal, 0.5=low, 1=medium, 2=high, 3=massive |
| **Confidence** | How sure are you about the Reach and Impact estimates? | 0.5=low, 0.8=medium, 1.0=high |
| **Effort** | How many person-hours to implement? | Raw number (hours) |

### RICE Calculator

```
Task: Add barcode scanner integration
  Reach:      8   (most daily users scan barcodes)
  Impact:     2   (high — saves significant time)
  Confidence: 0.8 (medium — haven't tested hardware yet)
  Effort:     4   (4 hours of work)

  RICE = (8 × 2 × 0.8) / 4 = 12.8 / 4 = 3.2

Task: Add product image thumbnails
  Reach:      6   (most users, but not critical path)
  Impact:     0.5 (low — cosmetic improvement)
  Confidence: 1.0 (high — straightforward)
  Effort:     3   (3 hours of work)

  RICE = (6 × 0.5 × 1.0) / 3 = 3.0 / 3 = 1.0

Winner: Barcode scanner (3.2) beats thumbnails (1.0)
```

### RICE Comparison Table Format

```
| Task | Reach | Impact | Confidence | Effort | RICE |
|---|---|---|---|---|---|
| Barcode scanner | 8 | 2.0 | 0.8 | 4h | 3.2 |
| Low stock alerts | 9 | 2.0 | 1.0 | 2h | 9.0 |
| Product thumbnails | 6 | 0.5 | 1.0 | 3h | 1.0 |
| CSV import | 4 | 1.0 | 0.8 | 3h | 1.1 |

Priority order: Low stock alerts > Barcode scanner > CSV import > Thumbnails
```

### When RICE Works Best

- Comparing features from different domains (apples to oranges)
- Presenting priority decisions to stakeholders who want numbers
- Large backlogs (20+ items) where gut feel is unreliable
- When the team disagrees on priorities and needs a neutral framework

### RICE Pitfalls

- Garbage in, garbage out — if estimates are wild guesses, the score is meaningless
- Confidence factor is often ignored — always include it
- Effort creep — re-estimate effort as you learn more
- Do not over-optimize — if two items are within 20% RICE score, treat them as equal

---

## Eisenhower Matrix

The Eisenhower Matrix sorts work by urgency and importance. Use it for triaging incoming requests, managing interruptions, and deciding what to do RIGHT NOW versus what to schedule.

### Quadrants

```
                    URGENT                    NOT URGENT
            ┌──────────────────────┬──────────────────────┐
            │                      │                      │
 IMPORTANT  │   Q1: DO FIRST       │   Q2: SCHEDULE       │
            │                      │                      │
            │   - Production bugs  │   - Feature work     │
            │   - Data loss risks  │   - Refactoring      │
            │   - Broken builds    │   - Documentation    │
            │   - Security issues  │   - Test coverage    │
            │                      │   - Architecture     │
            │   Action: Fix NOW    │   Action: Plan it    │
            │                      │                      │
            ├──────────────────────┼──────────────────────┤
            │                      │                      │
 NOT        │   Q3: DELEGATE       │   Q4: ELIMINATE      │
 IMPORTANT  │                      │                      │
            │   - Minor UI tweaks  │   - Gold plating     │
            │   - Config changes   │   - Premature optim. │
            │   - Routine updates  │   - Features nobody  │
            │                      │     asked for        │
            │   Action: Quick do   │   Action: Drop it    │
            │   or delegate        │                      │
            │                      │                      │
            └──────────────────────┴──────────────────────┘
```

### Classification Questions

```
Is it URGENT?
  - Will something break or degrade if we don't do this today?
  - Is someone actively blocked by this?
  - Is there a hard deadline within this sprint?

Is it IMPORTANT?
  - Does this move us toward the sprint/project goal?
  - Does the user care about this directly?
  - Does this reduce risk or technical debt significantly?
```

### Eisenhower for Development Sessions

| Quadrant | Session Action |
|---|---|
| Q1 (Urgent + Important) | Drop everything, fix this first |
| Q2 (Important + Not Urgent) | This IS your sprint work — schedule and execute |
| Q3 (Urgent + Not Important) | Spend max 5 minutes, then move on |
| Q4 (Not Urgent + Not Important) | Do not work on this. Add to backlog or delete. |

---

## Choosing the Right Framework

```
What kind of decision are you making?

├── "What do I work on in this sprint?"
│   └── Use MoSCoW
│       - Fast, intuitive, capacity-aligned
│       - Tie-break with RICE if needed
│
├── "Which of these 20 features should we build first?"
│   └── Use RICE
│       - Quantitative comparison at scale
│       - Good for stakeholder communication
│
├── "Something just broke — what do I do right now?"
│   └── Use Eisenhower
│       - Triage urgency vs. importance
│       - Prevents reactive whiplash
│
└── "I'm overwhelmed with requests"
    └── Use Eisenhower first (triage), then MoSCoW (plan)
        - Eliminate Q4, quick-fix Q3, then plan Q1+Q2
```

### Combining Frameworks

For maximum effectiveness, use frameworks in layers:

1. **Eisenhower** to triage incoming work (5 minutes)
2. **MoSCoW** to categorize the important work into sprint buckets (10 minutes)
3. **RICE** to order items within the same MoSCoW category (if needed)

This three-layer approach handles everything from daily interruptions to quarterly planning.
