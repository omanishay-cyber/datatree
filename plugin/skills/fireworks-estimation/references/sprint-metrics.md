# Sprint Metrics — Complete Reference

Comprehensive guide to sprint velocity tracking, burndown analysis, capacity planning,
health indicators, and retrospective metrics.

---

## Velocity Calculation

### Definition

Velocity = total story points **completed** (accepted by product owner) in a sprint.

Points from stories that are "in progress" or "in review" at sprint end do NOT count.
Only fully done stories contribute to velocity.

### Rolling Average

```
Velocity = (V_n + V_n-1 + V_n-2) / 3
```

Use the last 3 sprints for a rolling average. This smooths out outliers while
remaining responsive to genuine trend changes.

**Example:**
```
Sprint 7:  22 points completed
Sprint 8:  28 points completed
Sprint 9:  24 points completed

Rolling Avg Velocity = (22 + 28 + 24) / 3 = 24.7

For planning Sprint 10: commit to 24 points (round DOWN, not up)
```

### Velocity Stabilization

- Sprints 1-3: Velocity will fluctuate wildly. Use 60% of theoretical capacity.
- Sprints 4-6: Velocity begins to stabilize. Use rolling average but add 15% buffer.
- Sprints 7+: Velocity is stable. Trust the rolling average.

### Velocity Adjustments

Adjust velocity when team composition changes:

```
Adjusted Velocity = Current Velocity x (New Team Size / Old Team Size) x 0.85
```

The 0.85 factor accounts for:
- New team member ramp-up time
- Changed team dynamics
- Knowledge transfer overhead

---

## Burndown Chart Data Format

### Sprint Burndown

Track daily progress against the ideal burndown line:

```json
{
  "sprint": "Sprint 10",
  "startDate": "2026-03-23",
  "endDate": "2026-04-03",
  "totalPoints": 24,
  "dailyData": [
    { "day": 1, "date": "2026-03-23", "ideal": 21.6, "actual": 24, "completed": 0 },
    { "day": 2, "date": "2026-03-24", "ideal": 19.2, "actual": 21, "completed": 3 },
    { "day": 3, "date": "2026-03-25", "ideal": 16.8, "actual": 18, "completed": 6 },
    { "day": 4, "date": "2026-03-26", "ideal": 14.4, "actual": 15, "completed": 9 },
    { "day": 5, "date": "2026-03-27", "ideal": 12.0, "actual": 13, "completed": 11 },
    { "day": 6, "date": "2026-03-28", "ideal": 9.6,  "actual": 10, "completed": 14 },
    { "day": 7, "date": "2026-03-31", "ideal": 7.2,  "actual": 8,  "completed": 16 },
    { "day": 8, "date": "2026-04-01", "ideal": 4.8,  "actual": 5,  "completed": 19 },
    { "day": 9, "date": "2026-04-02", "ideal": 2.4,  "actual": 3,  "completed": 21 },
    { "day": 10, "date": "2026-04-03", "ideal": 0,   "actual": 0,  "completed": 24 }
  ]
}
```

### Ideal Burndown Line

```
Ideal Remaining = Total Points - (Total Points / Sprint Days) x Day Number
```

The ideal line is straight. Reality is always jagged. The goal is to stay
within +/- 15% of the ideal line throughout the sprint.

---

## Sprint Health Indicators

### Scope Creep Index

```
Scope Creep = (Points Added After Sprint Start) / (Original Commitment) x 100%
```

| Scope Creep % | Status | Action |
|---|---|---|
| 0-5% | Healthy | Normal adjustments |
| 5-10% | Warning | Discuss in standup, consider removing equal points |
| 10-20% | Unhealthy | Escalate to product owner, protect sprint goal |
| > 20% | Critical | Sprint is compromised, consider reset |

### Blocked Ratio

```
Blocked Ratio = (Sum of Days Stories Were Blocked) / (Total Story-Days) x 100%
```

A story-day is one story for one day. If you have 8 stories in a 10-day sprint,
total story-days = 80.

| Blocked Ratio | Status | Action |
|---|---|---|
| 0-5% | Healthy | Normal dependencies |
| 5-10% | Warning | Review blocking issues in standup |
| 10-20% | Unhealthy | Systemic blocker, needs management attention |
| > 20% | Critical | Team is unable to work effectively |

### Carry-Over Rate

```
Carry-Over Rate = (Stories Not Completed) / (Stories Committed) x 100%
```

| Carry-Over % | Status | Action |
|---|---|---|
| 0% | Excellent | Team may be undercommitting |
| 1-15% | Healthy | Normal variation |
| 15-30% | Warning | Overcommitting or poor estimation |
| > 30% | Unhealthy | Fundamental estimation or capacity problem |

### Focus Factor

```
Focus Factor = (Points Completed) / (Available Person-Days x Hours/Day)
```

This shows how many points the team delivers per person-hour of effort.
Track over time to see if efficiency is improving or degrading.

---

## Capacity Planning Formulas

### Basic Capacity

```
Capacity = Team Members x Sprint Days x Productive Hours/Day

Example:
  3 developers x 10 days x 6 hours/day = 180 person-hours
```

### Adjusted Capacity

Factor in known disruptions:

```
Adjusted Capacity = Base Capacity
  - (PTO days x 6 hours)
  - (Meeting overhead x Sprint Days)
  - (On-call reduction)
  - (Sprint ceremonies: planning + review + retro + daily standups)

Example:
  Base: 180 person-hours
  - Dev A: 2 days PTO = -12 hours
  - Sprint ceremonies: 3 devs x 8 hours = -24 hours
  - On-call: Dev B for 5 days at 50% = -15 hours
  Adjusted: 180 - 12 - 24 - 15 = 129 person-hours
```

### Points-Based Capacity

Convert adjusted capacity to points:

```
Committable Points = (Adjusted Person-Hours / Historical Hours-Per-Point) x 0.80

Historical Hours-Per-Point = Sum(Actual Hours) / Sum(Points Delivered)
  over last 3 sprints

The 0.80 multiplier reserves 20% for unplanned work.
```

### Capacity for Mixed Teams

When team members have different skill levels or part-time allocations:

```
| Team Member | Availability | Skill Factor | Effective Days |
|-------------|-------------|--------------|----------------|
| Senior Dev  | 100%        | 1.0          | 10.0           |
| Mid Dev     | 100%        | 0.8          | 8.0            |
| Junior Dev  | 50%         | 0.5          | 2.5            |
| Total       |             |              | 20.5           |

Capacity = 20.5 effective-days x 6 hours = 123 effective person-hours
```

---

## Sprint Retrospective Metrics Template

Collect these metrics at the end of every sprint:

### Quantitative Metrics

```
SPRINT METRICS REPORT — Sprint [N]

VELOCITY
  Committed:       [X] points
  Completed:       [Y] points
  Commitment Ratio: [Y/X * 100]%
  Rolling Velocity: [3-sprint avg] points

SCOPE
  Original Stories:  [N]
  Added Mid-Sprint:  [N] ([P] points)
  Removed Mid-Sprint: [N] ([P] points)
  Scope Creep Index: [%]

QUALITY
  Bugs Found:       [N]
  Bugs Fixed:       [N]
  Bug Carry-Over:   [N]
  Bug Points Ratio: [bug points / total points]%

FLOW
  Avg Cycle Time:   [days from "In Progress" to "Done"]
  Blocked Stories:   [N]
  Blocked Days:      [N total days]
  Blocked Ratio:     [%]
  Carry-Over Stories: [N]

ESTIMATION ACCURACY
  Stories within +/-20%: [N] / [Total] ([%])
  Average overrun:       [%]
  Average underrun:      [%]
  Worst estimate:        Story [ID] — estimated [X], actual [Y]
```

### Qualitative Assessment

Rate each category 1-5 and track trends:

```
| Category              | Score (1-5) | Trend (vs last sprint) |
|-----------------------|-------------|------------------------|
| Sprint goal achieved  |             | up / same / down       |
| Team collaboration    |             |                        |
| Code quality          |             |                        |
| Process efficiency    |             |                        |
| Stakeholder satisfaction |          |                        |
```

### Action Items Format

```
ACTION ITEMS FROM RETRO — Sprint [N]

1. [ACTION] — Owner: [Name] — Due: [Date]
   Context: [Why this action was identified]
   Success criteria: [How we know it is done]

2. [ACTION] — Owner: [Name] — Due: [Date]
   Context: [Why]
   Success criteria: [How]
```

Limit to 3 action items per retro. More than 3 means nothing gets done.
Review previous sprint's action items first — were they completed?

---

## Velocity Trend Analysis

### Interpreting Trends

```
Velocity over 8 sprints: 18, 20, 22, 24, 23, 25, 24, 26

Trend: Gradually increasing (+0.8 points/sprint average)
Status: Healthy growth — team is improving

Velocity over 8 sprints: 24, 22, 20, 18, 19, 17, 16, 15

Trend: Gradually decreasing (-1.0 points/sprint average)
Status: Degrading — investigate causes (tech debt, burnout, scope creep)

Velocity over 8 sprints: 24, 15, 28, 12, 30, 14, 26, 13

Trend: Wild oscillation
Status: Unstable — inconsistent estimation, scope changes, or team disruption
```

### Velocity Ceiling

Every team has a sustainable velocity ceiling. Pushing beyond it leads to:
- Quality degradation (bugs increase)
- Burnout (team satisfaction drops)
- Technical debt accumulation
- Estimation gaming (inflating points)

**Signs you are at the ceiling:**
- Bug ratio exceeds 25% of velocity
- Carry-over rate increases
- Team satisfaction scores decline
- Cycle time increases despite velocity being stable

### Using Velocity for Release Planning

```
Remaining Points in Release: 120
Current Velocity: 24 points/sprint
Sprint Length: 2 weeks

Optimistic: 120 / 28 = 4.3 sprints (use max velocity from last 3)
Expected:   120 / 24 = 5.0 sprints (use average velocity)
Pessimistic: 120 / 20 = 6.0 sprints (use min velocity from last 3)

Expected Release Date: 10 weeks from now (5 x 2 weeks)
Range: 8.6 - 12 weeks
Buffer: Add 20% = 12 weeks (6 sprints)

Communicate: "12 weeks with 85% confidence, could be as early as 9 weeks"
```
