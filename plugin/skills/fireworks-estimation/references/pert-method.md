# PERT Method — Complete Reference

Program Evaluation and Review Technique (PERT) provides statistically grounded
estimates by combining three-point estimation with weighted averages and standard
deviation calculations.

---

## Core PERT Formula

### Three-Point Inputs

Every task requires three time estimates:

- **O (Optimistic)**: The minimum possible time assuming everything goes right.
  This is NOT zero. It is the fastest you could reasonably complete the work
  if every decision was correct on the first try.

- **R (Realistic / Most Likely)**: The most probable duration given normal conditions.
  This is your experienced gut feel after careful consideration. Minor issues
  will occur, but nothing catastrophic.

- **P (Pessimistic)**: The maximum reasonable time if significant problems arise.
  This is NOT infinite. It is the worst case within the realm of plausibility.
  Hardware failures, natural disasters, and alien invasions are excluded.

### Expected Duration

```
E = (O + 4R + P) / 6
```

The formula assigns 4x weight to the realistic estimate because it represents
the peak of the probability distribution. Optimistic and pessimistic each get
1x weight as they represent the tails.

### Standard Deviation

```
SD = (P - O) / 6
```

Standard deviation measures the spread of uncertainty. Larger SD means more
uncertain estimate.

### Variance

```
Variance = SD^2 = ((P - O) / 6)^2
```

Variance is useful when combining multiple task estimates for a project total,
because variances are additive (standard deviations are not).

---

## Confidence Intervals

Using the normal distribution:

| Confidence | Range | Interpretation |
|---|---|---|
| **68.3%** | E +/- 1 SD | "Likely" — 2 out of 3 times |
| **95.4%** | E +/- 2 SD | "Very likely" — 19 out of 20 times |
| **99.7%** | E +/- 3 SD | "Almost certain" — 369 out of 370 times |

### How to Choose a Confidence Level

- **Informal planning**: Use 68% (1 SD). Quick and usually good enough.
- **Sprint commitments**: Use 85-90%. Missing a sprint commitment has consequences.
- **Client-facing deadlines**: Use 95%. Under-delivering to clients damages trust.
- **Contractual obligations**: Use 99%. Financial penalties require near-certainty.

---

## Worked Examples

### Example 1: Simple Feature (Login Form)

```
Task: Build login form with email/password validation
O = 3 hours (done this many times, straightforward)
R = 5 hours (typical with minor CSS adjustments)
P = 10 hours (design changes, accessibility issues)

E  = (3 + 4*5 + 10) / 6 = 33/6 = 5.5 hours
SD = (10 - 3) / 6 = 1.17 hours

68% confidence: 4.3 - 6.7 hours
95% confidence: 3.2 - 7.8 hours
```

**Communicate**: "5.5 hours expected, 4-7 hours likely, up to 8 hours worst case."

### Example 2: Complex Feature (Real-Time Sync)

```
Task: Implement real-time data sync between Electron app and cloud
O = 16 hours (ideal path, libraries work perfectly)
R = 32 hours (some integration issues, retry logic needed)
P = 80 hours (protocol issues, conflict resolution complexity)

E  = (16 + 4*32 + 80) / 6 = 224/6 = 37.3 hours
SD = (80 - 16) / 6 = 10.67 hours

68% confidence: 26.7 - 48.0 hours
95% confidence: 16.0 - 58.7 hours
```

**Red flag**: P (80) > 3x R (32) = 96, so P < 3R. This passes.
If P were 100 hours, that would exceed 3x32=96, requiring decomposition.

### Example 3: Multi-Task Project Estimate

When combining multiple tasks, sum the expected values and add variances:

```
Task A: E=5.5h, SD=1.17h, Var=1.37
Task B: E=37.3h, SD=10.67h, Var=113.85
Task C: E=8.0h, SD=2.00h, Var=4.00

Project E  = 5.5 + 37.3 + 8.0 = 50.8 hours
Project Var = 1.37 + 113.85 + 4.00 = 119.22
Project SD  = sqrt(119.22) = 10.92 hours

68% confidence: 39.9 - 61.7 hours
95% confidence: 29.0 - 72.6 hours
```

Note: You CANNOT simply add SDs (5.5+37.3+8=50.8 is wrong for SD).
You MUST add variances and then take the square root.

### Example 4: Adding Risk Buffers to PERT

After PERT calculation, apply risk multipliers:

```
Base PERT estimate: 37.3 hours
Technical risk (new WebSocket library): 1.3x
Domain risk (sync conflict rules unclear): 1.5x
Compound risk: 1.3 x 1.5 = 1.95x

Buffered estimate: 37.3 x 1.95 = 72.7 hours
Integration buffer (25%): 72.7 x 1.25 = 90.9 hours

Final estimate: ~91 hours (68% confidence)
```

---

## When to Use PERT vs Other Methods

### PERT (This Method)
- **Best for**: Individual task estimation, project planning, deadline commitments
- **Strengths**: Statistically grounded, accounts for uncertainty, produces ranges
- **Weaknesses**: Requires thoughtful O/R/P input, can be gamed, time-consuming for many tasks
- **Use when**: Accuracy matters, stakeholders need confidence intervals, contractual deadlines

### T-Shirt Sizing (XS/S/M/L/XL)
- **Best for**: Quick relative sizing, backlog grooming, initial scoping
- **Strengths**: Fast, low overhead, good for comparing items
- **Weaknesses**: Not precise enough for scheduling, no statistical basis
- **Use when**: You need rough ordering, not precise dates

### Planning Poker (Fibonacci Points)
- **Best for**: Team estimation, building consensus, exposing assumptions
- **Strengths**: Reduces anchoring bias, reveals disagreements, builds shared understanding
- **Weaknesses**: Time-consuming with large backlogs, requires team availability
- **Use when**: Team alignment matters more than individual accuracy

### Expert Judgment
- **Best for**: Novel problems, no historical data, proof of concept
- **Strengths**: Leverages deep experience, handles unique situations
- **Weaknesses**: Subject to all cognitive biases, depends on expert availability
- **Use when**: No other method applies, but document assumptions heavily

### Recommended Approach

Combine methods based on project phase:

1. **Discovery**: T-shirt sizing for rough scoping
2. **Planning**: Planning poker for team-wide story sizing
3. **Commitment**: PERT for tasks in the upcoming sprint
4. **Tracking**: Velocity-based projection for release dates

---

## Common PERT Mistakes

### Mistake 1: Anchoring on O
Developers tend to estimate based on the optimistic case. Guard against this
by ALWAYS estimating P first, then O, then R.

### Mistake 2: Compressed P Values
When P is too close to R, you are underestimating risk. Ask: "What could go
wrong?" For each answer, add time to P.

### Mistake 3: Ignoring Correlation
PERT assumes tasks are independent. If Task B depends on Task A's outcome,
the combined uncertainty is higher than the formula suggests. Add 10-20%
correlation buffer for dependent chains.

### Mistake 4: Not Updating Estimates
PERT estimates are snapshots. As you learn more about a task (completing
a spike, resolving an unknown), re-run the calculation with updated O/R/P.

### Mistake 5: Using PERT for Trivial Tasks
Tasks under 2 hours do not benefit from PERT. Use a single point estimate
for trivial work. Reserve PERT for tasks with genuine uncertainty.

---

## PERT for Project-Level Estimation

For entire projects, use the Critical Path Method (CPM) with PERT:

1. Identify all tasks and their dependencies
2. Calculate PERT for each task
3. Find the critical path (longest chain of dependent tasks)
4. Sum PERT values along the critical path for project duration
5. Sum variances along the critical path for project uncertainty

```
Critical Path: A -> C -> E -> G
Project E  = E_A + E_C + E_E + E_G
Project SD = sqrt(Var_A + Var_C + Var_E + Var_G)
```

Non-critical-path tasks have float (slack time). They can slip without
affecting the project deadline, up to their float amount.

---

## Historical Calibration

Track your actual vs estimated values to improve future estimates:

```
Calibration Ratio = Actual Time / Estimated Time

If consistently > 1.0: You are underestimating. Increase R values.
If consistently < 1.0: You are overestimating. Decrease P values.
If wildly variable:    Your decomposition is too coarse. Break down more.
```

Track calibration ratios by:
- Task type (frontend, backend, infrastructure, testing)
- Complexity level (low, medium, high)
- Developer (individual calibration is the most accurate)

After 10+ calibrated estimates, you can apply your personal calibration
factor to future estimates for improved accuracy.
