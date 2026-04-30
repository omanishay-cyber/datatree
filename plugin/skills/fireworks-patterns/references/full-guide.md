# fireworks-patterns — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 3. Pattern Transfer Protocol (Crown Jewel)

The single most powerful engineering meta-skill: recognizing that a problem you
face today is structurally identical to problems solved before in other
languages, frameworks, or domains — and transferring the solution.

### Step 1: IDENTIFY the Problem Class

Surface symptoms map to a small set of core problem classes:

| Surface Symptom | Core Problem Class |
|---|---|
| "State gets out of sync" | State Observation |
| "Race conditions / ordering issues" | Async Orchestration |
| "Too slow, repeated computation" | Caching / Memoization |
| "Too many requests / overload" | Rate Limiting |
| "Intermittent failures" | Retry / Resilience |
| "Data format mismatch" | Data Transformation |
| "Need to extend without modifying" | Open/Closed (Decoration) |
| "Complex conditional logic" | Strategy / State Machine |
| "Tight coupling between modules" | Mediation / Pub-Sub |
| "Need undo/redo or audit trail" | Command / Event Sourcing |

### Step 2: RECALL Prior Implementations

Map all known manifestations of this problem class across domains:

```
Problem: State Observation
├── React: useEffect + dependency array
├── Svelte: $: reactive declarations
├── Zustand: subscribe() + selector
├── Electron: ipcMain.on / ipcRenderer.on
├── Flutter: StreamBuilder / ValueNotifier
├── RxJS: Observable.subscribe()
├── Python: property decorator / descriptor protocol
├── Backend: Webhook / polling / SSE / WebSocket
└── Database: triggers / change streams / CDC
```

### Step 3: EXTRACT Canonical Solution (the ESSENCE)

Strip away all language/framework specifics. Write the core algorithm as
numbered steps:

```
ESSENCE of Observer Pattern:
1. Subject maintains list of observers
2. Observers register interest in specific state
3. When state changes, subject notifies all registered observers
4. Observers receive new state and react accordingly
5. Observers can unregister to stop receiving notifications
```

### Step 4: MAP to Local Idioms

Translate the essence into the target domain's conventions:

```
Target: Zustand store in React
- "Subject" = Zustand store
- "Observers" = React components using useStore(selector)
- "Register" = component mounts and calls useStore
- "Notify" = Zustand triggers re-render on selector change
- "Unregister" = component unmounts (automatic cleanup)
```

### Step 5: REIFY (Implement Using Local Conventions)

Write the actual code using the target framework's best practices and idioms.
This is where framework expertise matters — the ESSENCE tells you WHAT to build,
the IDIOM tells you HOW to build it in this specific context.

```typescript
// Reified: Zustand observer pattern
const useInventoryStore = create<InventoryState>((set, get) => ({
  items: [],
  addItem: (item) => set((s) => ({ items: [...s.items, item] })),
  removeItem: (id) => set((s) => ({ items: s.items.filter(i => i.id !== id) })),
}));

// Component "observes" only what it needs
function ItemCount() {
  const count = useInventoryStore((s) => s.items.length); // selective observer
  return <span>{count} items</span>;
}
```

---

## 4. Quick Reference Pattern Mapping Table

| Pattern | Web/React | Electron | Flutter | Backend |
|---|---|---|---|---|
| Observer | useEffect/events | ipcMain.on | StreamBuilder | EventEmitter |
| Strategy | props/render | handler map | Strategy widget | DI container |
| Decorator | HOC/hooks | middleware | widget wrapping | @decorator |
| Memoization | useMemo/memo | cache Map | compute cache | Redis/LRU |
| Rate Limit | debounce/throttle | IPC throttle | Timer | token bucket |
| Retry | async retry | IPC retry | http retry | circuit breaker |
| Builder | form builder | config builder | widget builder | fluent API |
| Command | action/reducer | IPC invoke | BLoC event | event sourcing |
| Chain | pipe/compose | middleware chain | stream pipe | Express middleware |
| Factory | component factory | window factory | widget factory | service factory |
| Adapter | API wrapper | bridge module | platform channel | gateway |
| Singleton | context/provider | app instance | GetIt/Provider | DI singleton |
| State Machine | useReducer/XState | FSM handler | Bloc/Cubit | state chart |
| Repository | data hooks | database service | repository class | DAO/repository |
| Mediator | event bus | IPC mediator | event channel | message broker |

---

## 5. Technical Debt Management

### Debt Quadrant (Martin Fowler's Model)

```
                    DELIBERATE                    INADVERTENT
            ┌─────────────────────────┬───────────────────────────┐
            │                         │                           │
  RECKLESS  │  "No time for tests"    │  "What's a design        │
            │                         │   pattern?"               │
            │  [DANGEROUS]            │  [INCOMPETENCE]           │
            │  Action: Stop. Add      │  Action: Training +       │
            │  tests NOW.             │  pairing + code review    │
            │                         │                           │
            ├─────────────────────────┼───────────────────────────┤
            │                         │                           │
  PRUDENT   │  "Ship now, refactor    │  "Now we know how we     │
            │   in sprint 2"          │   should have done it"    │
            │                         │                           │
            │  [STRATEGIC]            │  [LEARNING]               │
            │  Action: Document debt, │  Action: Refactor when    │
            │  schedule payment       │  touching this code next  │
            │                         │                           │
            └─────────────────────────┴───────────────────────────┘
```

### Debt Lifecycle

```
IDENTIFY → DOCUMENT (TD-XXX) → CALCULATE INTEREST → PRIORITIZE → PLAN PAYMENT → VERIFY
```

1. **Identify**: Code smells, test failures, slow builds, repeated bugs
2. **Document**: Assign ID (TD-001), describe debt, note location, estimate age
3. **Calculate Interest**: `Annual Interest = Cost per Incident x Incidents per Year`
4. **Prioritize**: `Priority Score = Annual Interest / Payoff Effort`
5. **Plan Payment**: Schedule into sprint, allocate capacity
6. **Verify**: Confirm debt is paid, update register, measure improvement

### Interest Calculation

```
Example: Flaky test causes 2 hours of investigation per incident,
         happens 6 times per quarter (24/year).

Annual Interest = 2 hours x 24 incidents = 48 hours/year
Payoff Effort   = 8 hours to fix the root cause
Priority Score  = 48 / 8 = 6.0 → MEDIUM (3-10 range)
```

### Priority Thresholds

| Score | Priority | Timeline | Action |
|---|---|---|---|
| > 10 | HIGH | Pay within 30 days | Schedule immediately |
| 3-10 | MEDIUM | Pay within 1-4 months | Add to backlog with deadline |
| < 3 | LOW | Pay when convenient (>4 months) | Address when touching that code |

### Budget Rules

- Maximum 10 open debt items per module
- HIGH priority items must be paid within 30 days
- Allocate 20% of sprint capacity to debt payment
- Every PR that adds debt must document it (TD-XXX in PR description)
- Boy Scout Rule: leave code better than you found it

---

## 6. Design Pattern Selection Decision Trees

### State Management

```
Is state local to one component?
├── YES → useState / local variable
└── NO → Is it shared by 2-3 siblings?
    ├── YES → Lift state to parent / prop drilling
    └── NO → Is it app-wide?
        ├── YES (simple) → React Context / Zustand store
        ├── YES (complex) → Zustand with slices / Redux Toolkit
        └── YES (audit trail needed) → Event sourcing
```

### Authentication

```
Who are the users?
├── Internal tool → Session-based auth (simplest)
├── API consumers → JWT (stateless, scalable)
├── Third-party login needed → OAuth2 / OIDC
└── Enterprise SSO → SAML 2.0
```

### Caching Strategy

```
How many processes?
├── Single process → In-memory Map / LRU cache
├── Multiple processes, same machine → Shared memory / file cache
├── Distributed → Redis / Memcached
└── Static assets → CDN (CloudFront, Cloudflare)
```

### Async Work Patterns

```
How reliable must execution be?
├── Best effort → setTimeout / setImmediate
├── Must complete → Job queue (BullMQ, SQS)
├── Real-time stream → WebSocket / SSE / Stream
└── Event-driven → Serverless function (Lambda, Cloud Functions)
```

### Data Storage

```
What kind of data?
├── Structured, relational → SQLite (local) / PostgreSQL (server)
├── Documents, flexible schema → MongoDB / Firestore
├── Key-value, fast access → Redis / DynamoDB
├── Time series → InfluxDB / TimescaleDB
├── Full audit trail → Event store / append-only log
└── Search → Elasticsearch / Meilisearch
```

### Service Architecture

```
Team size and complexity?
├── 1-5 developers → Monolith (one deployable)
├── 5-20 developers → Modular monolith (clear boundaries, one deploy)
├── 20+ developers → Microservices (independent deploy)
└── Variable load → Serverless (pay per use)
```

---

## 7. Code Smell Quick Reference

| Smell | Detection | Fix | Urgency |
|---|---|---|---|
| Long method (>50 lines) | Line count | Extract method | MEDIUM |
| Feature envy | Method uses another class's data more than its own | Move method to where data lives | MEDIUM |
| Shotgun surgery | One change requires editing 5+ files | Consolidate related logic | HIGH |
| Primitive obsession | Using strings/numbers for domain concepts (e.g., `status: string` instead of `Status` enum) | Value objects / enums | MEDIUM |
| Data clump | Same 3+ parameters passed together repeatedly | Extract into a type/class | LOW |
| God class (>300 lines) | File length, too many responsibilities | Split by single responsibility | HIGH |
| Dead code | grep shows no callers, no test coverage | Delete (git has history) | LOW |
| Duplicated logic | Similar blocks in 3+ places | Extract shared utility | MEDIUM |
| Boolean blindness | `doThing(true, false, true)` | Use named options object | MEDIUM |
| Deep nesting (>3 levels) | Indentation depth | Early returns, extract helper | MEDIUM |
| Inappropriate intimacy | Class accesses private details of another | Define proper interface | HIGH |
| Speculative generality | Abstractions for future use cases that don't exist | YAGNI — delete unused abstractions | LOW |

---

## 8. Legacy Code Protocol

From the Unicorn Team — how to safely modify code you did not write and do
not fully understand.

### Step 1: Establish Baseline

Run all existing tests. Record pass/fail counts. If there are no tests,
note that — you are flying blind.

### Step 2: Add Characterization Tests

Test what the code **DOES**, not what it **SHOULD** do. These tests document
current behavior, including bugs. They are your safety net.

```typescript
// Characterization test example
test('formatPrice returns string with 2 decimals', () => {
  // Testing actual behavior, even if it seems wrong
  expect(formatPrice(10)).toBe('$10.00');
  expect(formatPrice(0)).toBe('$0.00');
  expect(formatPrice(-5)).toBe('$-5.00'); // Bug? Maybe. But it's current behavior.
});
```

### Step 3: Map Dependency Graph

Draw (or mentally model) what depends on what:

```
Module A ──depends on──> Module B ──depends on──> Module C
    │                        │
    └──depends on──> Module D (shared utility)
```

### Step 4: Identify Load-Bearing Walls

Find the code that everything else depends on. These are **high-risk change
points** — modifying them affects the entire system.

Signs of load-bearing code:
- Many imports/requires pointing to it
- Used in critical paths (auth, data persistence, routing)
- No tests covering it (highest risk)
- Complex with many side effects

### Step 5: Find Seams

Seams are safe points where you can make changes without affecting
everything else. Look for:
- Function boundaries (inputs and outputs are clear)
- Module boundaries (imports/exports define the contract)
- Interface boundaries (abstractions that can be swapped)
- Configuration points (behavior controlled by config, not code)

### Step 6: Refactor Incrementally

One seam at a time:
1. Write characterization tests for the seam
2. Make the change
3. Run ALL tests
4. Commit
5. Move to next seam

**Never refactor multiple seams in one commit.**

---

## 9. Common Patterns to Recognize

When reading a codebase, identify which patterns are in use:

| Pattern | Recognition Signal |
|---|---|
| MVC | Separate model, view, controller directories or layers |
| Repository | Classes that abstract database access behind methods |
| Strategy | Interface with multiple implementations, selected at runtime |
| Observer | Event listeners, subscriptions, callback registrations |
| Factory | Functions/classes that create other objects based on input |
| Decorator | Wrapping objects/functions to add behavior |
| Adapter | Classes that convert one interface to another |
| Singleton | `getInstance()`, module-level instance, `app.use()` |
| Command | Action objects with `execute()` / `undo()` methods |
| Chain of Responsibility | Handlers that pass to `next()` if they can't handle |
| State Machine | Explicit states with defined transitions |
| Pub/Sub | Event bus, message broker, `emit`/`on` patterns |
| Middleware | `(req, res, next)` pattern, pipeline processing |
| HOC | Functions that take a component and return an enhanced component |
| Render Props | Components that take a function as children/render prop |
| Compound Components | Components designed to work together (Tabs/Tab/TabPanel) |
| Provider | Context.Provider wrapping tree to share state |
| Container/Presenter | Smart components (data) + dumb components (display) |
| Module | Self-contained units with explicit exports |
| Facade | Simplified interface hiding complex subsystem |

---

## 10. Verification Gates

Before completing any engineering task, check these gates:

### Gate 1: Strategic Reading

- [ ] Code was read following execution flow, not alphabetically
- [ ] Entry point was identified first
- [ ] Data flow was traced end-to-end
- [ ] Error paths were mapped
- [ ] Integration points were cataloged

### Gate 2: Pattern Transfer

- [ ] Problem class was identified (not just surface symptom)
- [ ] Prior implementations were recalled (at least 2 domains)
- [ ] Essence was extracted (language-independent steps)
- [ ] Local idioms were identified (framework conventions)
- [ ] Implementation uses framework best practices

### Gate 3: Technical Debt

- [ ] Debt has an ID (TD-XXX)
- [ ] Interest is calculated (cost x frequency)
- [ ] Priority score is computed (interest / effort)
- [ ] Payment is scheduled based on threshold
- [ ] Debt register is updated

### Gate 4: Pattern Selection

- [ ] Decision tree was followed (not gut feeling)
- [ ] At least 2 alternatives were considered
- [ ] Selection rationale is documented
- [ ] Trade-offs are acknowledged
- [ ] Pattern matches the problem scale (not over-engineered)

---

## 11. INVARIANTS (Non-Negotiable Rules)

These rules are absolute. Never violate them regardless of time pressure,
scope, or context.

1. **Always read code strategically (execution flow), never alphabetically.**
   Reading in file system order wastes time and misses the architecture.

2. **Every pattern transfer must extract the ESSENCE before implementing.**
   Skipping the essence step leads to cargo-cult programming — copying syntax
   without understanding the underlying mechanism.

3. **Technical debt must be quantified (interest + payoff cost), not just
   described.** "We have tech debt" is useless. "TD-042 costs us 48 hours/year
   and takes 8 hours to fix" is actionable.

4. **Never choose a pattern without considering 2+ alternatives.** The first
   pattern that comes to mind is often not the best. Force yourself to consider
   at least one alternative and articulate why you chose one over the other.

5. **Legacy code changes must have characterization tests first.** Changing
   code you don't fully understand without tests is gambling. Add tests that
   document current behavior before making any modifications.

6. **Code reading depth matches the task.** Don't read at L4 when L1 suffices.
   Don't skim at L1 when the task requires L3 understanding. Match your depth
   to the work.

7. **Pattern recognition precedes implementation.** Before writing code, ask:
   "What problem class is this? Has this been solved before?" The answer is
   almost always yes.

---

## Usage

This skill activates when:
- Reading an unfamiliar codebase
- Transferring a pattern from one domain to another
- Assessing or managing technical debt
- Choosing between design patterns
- Working with legacy code
- Performing architecture reviews
- Onboarding to a new project

Combine with:
- `fireworks-review` for code review workflows
- `fireworks-refactor` for refactoring guidance
- `fireworks-debug` for debugging protocols
- `fireworks-architect` for architecture decisions
- `fireworks-security` for security pattern selection

---

## References

For deep dives into each meta-skill:

- [Code Reading](references/code-reading.md) -- advanced code archaeology, execution flow tracing, annotation techniques
- [Pattern Catalog](references/pattern-catalog.md) -- GoF patterns, domain patterns, architectural patterns with decision trees
- [Pattern Transfer](references/pattern-transfer.md) -- Essence/Idiom/Reify methodology for cross-domain pattern application
- [Technical Debt](references/technical-debt.md) -- debt quantification, prioritization matrices, remediation strategies

---

### First Principles Framework (ADI Cycle)
When analyzing unfamiliar code patterns, use the Abduction-Deduction-Induction cycle:
1. **Abduction**: Observe the code behavior. Generate hypotheses about WHY it's structured this way.
2. **Deduction**: From each hypothesis, deduce what OTHER patterns should exist if the hypothesis is true.
3. **Induction**: Search for those predicted patterns. If found, hypothesis strengthens. If not, reject.

### 7 Persuasion Principles for Skill Writing
When authoring skills or agent prompts, apply systematically:
| Principle | When to Use | Example |
|---|---|---|
| Authority | Discipline enforcement | "YOU MUST", "NEVER", "The Iron Law" |
| Commitment | Multi-step processes | "Announce at start", explicit tracking |
| Scarcity | Verification requirements | "IMMEDIATELY verify", urgency |
| Social Proof | Universal practices | "Every time", "All engineers" |
| Unity | Collaborative workflows | "We", shared identity |
| Reciprocity | Rarely needed | Obligation patterns |
| Liking | AVOID | Creates sycophancy — anti-pattern |

Combination: Discipline skills = Authority + Commitment + Social Proof. Guidance = Moderate Authority + Unity.

---

## Related Skills

- `fireworks-refactor` — apply patterns to fix smells
- `fireworks-review` — identify patterns in review
- `fireworks-research` — code reading for investigation

---

## Scope Boundaries

- **MINIMUM**: Always read entry points before deep-diving.
- **MAXIMUM**: L1 comprehension is sufficient for most tasks — only go deeper when needed.
