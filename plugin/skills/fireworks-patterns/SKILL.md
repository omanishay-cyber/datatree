---
name: fireworks-patterns
description: Engineering meta-skills — strategic code reading, pattern transfer (Essence/Idiom/Reify), technical debt management, design pattern selection, and codebase comprehension protocols
version: 2.0.0
author: mneme
tags: [patterns, design-pattern, technical-debt, code-reading, legacy, code-smell, architecture]
triggers: [pattern, design pattern, technical debt, code reading, legacy, code smell, architecture review, refactor pattern]
---

# Fireworks Patterns — Engineering Meta-Skills

Consolidated engineering meta-skills from Unicorn Team. This skill covers the
foundational disciplines that make every other engineering skill more effective:
how to READ code strategically, how to TRANSFER patterns across domains, how to
MANAGE technical debt quantitatively, and how to SELECT design patterns using
decision trees rather than gut instinct.

---

## 1. Strategic Code Reading Protocol

### The Cardinal Rule

**NEVER read code alphabetically.** Reading files in directory order is the
single most common mistake. Code is a living system with execution flow — read
it the way the runtime reads it.

### Reading Order (Follow Execution Flow)

```
1. ENTRY POINT        → main.ts, index.ts, App.tsx, electron main process
2. ROUTE DEFINITIONS  → router config, IPC channel registration, API routes
3. HANDLERS           → route handlers, IPC handlers, event listeners
4. BUSINESS LOGIC     → services, domain logic, state management
5. DATA LAYER         → repositories, database queries, ORM models
6. UTILITIES          → helpers, formatters, validators, shared functions
```

### For Electron + React Applications

```
1. package.json        → scripts, entry points, dependencies
2. electron/main.ts    → app lifecycle, window creation, IPC registration
3. electron/preload.ts → contextBridge, exposed APIs
4. src/main.tsx        → React root, providers, router setup
5. src/App.tsx         → top-level layout, route definitions
6. src/pages/          → page components (follow primary user flow)
7. src/stores/         → Zustand stores, state shape, actions
8. src/components/     → shared components (read as needed)
9. src/utils/          → utilities (read as needed)
10. src/types/         → type definitions (reference as needed)
```

### Data Flow Tracing

For any feature, trace the complete data path:

```
INPUT         → Where does the data enter? (UI event, API call, IPC message)
VALIDATION    → How is it validated? (Zod, manual checks, type guards)
PROCESSING    → What transformations occur? (mapping, filtering, calculating)
STORAGE       → Where is it persisted? (database, file, memory, state store)
OUTPUT        → How does it reach the user? (render, response, notification)
```

### Error Path Mapping

For every code path, answer three questions:

1. **What can fail?** — Network errors, invalid input, missing data, permissions,
   timeout, out-of-memory, race conditions
2. **How is failure detected?** — try/catch, result types, validation, assertions,
   type guards, HTTP status codes
3. **How is failure handled?** — Retry, fallback, user notification, logging,
   circuit breaker, graceful degradation

### Integration Point Mapping

Identify every boundary where code touches an external system:

| Category | Examples |
|---|---|
| APIs | REST endpoints, GraphQL, gRPC, WebSocket |
| Databases | SQL queries, ORM calls, migrations |
| File System | Read/write, watch, temp files |
| IPC | Electron invoke/handle, postMessage |
| External Services | Auth providers, payment, email, cloud storage |
| OS | Clipboard, notifications, system tray, shortcuts |
| Browser APIs | localStorage, IndexedDB, Service Worker |
| Queues | Message brokers, job queues, event buses |

---

## 2. Four Comprehension Levels

### L1 — Behavior: What Does It DO?

**Method:** Read function signature + docstring/JSDoc + unit tests.

```
Time: 30 seconds per function
Outcome: "This function takes X and returns Y"
When: Quick orientation, code review, dependency analysis
```

### L2 — Mechanics: HOW Does It Work?

**Method:** Read the implementation line by line.

```
Time: 2-5 minutes per function
Outcome: "It does X by first doing A, then B, handling edge case C"
When: Bug fixing, extending functionality, writing tests
```

### L3 — Design: WHY This Way?

**Method:** Read comments, git log/blame, linked issues, PR descriptions.

```
Time: 10-30 minutes per function/module
Outcome: "This approach was chosen because of constraint X, alternative Y
          was rejected because of Z"
When: Refactoring, proposing alternatives, architecture review
```

### L4 — Impact: What ELSE Is Affected?

**Method:** grep the function name across the codebase, check all callers,
review dependent tests, trace through state management.

```
Time: 30-60 minutes per function/module
Outcome: "Changing this will affect modules A, B, C and requires updating
          tests D, E, F"
When: Architecture changes, API modifications, breaking changes
```

### Decision Guide

| Task | Levels Needed |
|---|---|
| Code review (surface) | L1 |
| Bug fix | L1 + L2 |
| Feature extension | L1 + L2 + L4 |
| Refactoring | L1 + L2 + L3 |
| Architecture change | All four levels |
| Onboarding to codebase | L1 for everything, L2 for core paths |

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
