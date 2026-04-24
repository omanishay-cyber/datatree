# Strategic Code Reading — Complete Reference

## Core Principle

Code is a living system with execution flow. Read it the way the runtime
executes it, not the way the file system alphabetizes it. Every minute spent
reading strategically saves ten minutes of confused debugging.

---

## The Strategic Reading Protocol

### Phase 1: Orientation (5-10 minutes)

Before reading a single line of code, answer these questions:

1. **What is this project?** — Read README.md, package.json, or equivalent
2. **What technology stack?** — Languages, frameworks, build tools
3. **What is the entry point?** — `main` field in package.json, `scripts.start`,
   `electron.main`, or equivalent
4. **What is the directory structure?** — `ls -la` or `tree` the top level
5. **What are the dependencies?** — Scan package.json dependencies and devDependencies

### Phase 2: Entry Point Deep Dive (10-20 minutes)

Read the entry point file completely. This tells you:

- How the application bootstraps
- What modules are loaded first
- What configuration is applied
- What error handling wraps the entire app

### Phase 3: Flow Tracing (20-60 minutes)

Pick the primary user flow and trace it end-to-end:

```
User action → UI event handler → state update → business logic →
data access → storage → response → UI update
```

### Phase 4: Branching Exploration (ongoing)

From the primary flow, branch out to:
- Error handling paths
- Secondary features
- Background processes
- Configuration and initialization
- Cleanup and shutdown

---

## 4 Comprehension Levels — Detailed Guide

### L1 — Behavior (What Does It DO?)

**Time budget:** 30 seconds per function/module

**What to read:**
- Function name and parameters
- Return type
- JSDoc / docstring
- First and last lines of the function body
- Associated unit tests (these ARE the specification)

**Outcome:** A one-sentence description of what this function does.

**Example:**
```typescript
// Reading this at L1:
export async function syncInventory(
  storeId: string,
  options?: SyncOptions
): Promise<SyncResult> { ... }

// L1 understanding: "Syncs inventory for a given store, returns a result."
```

**When L1 is sufficient:**
- Code review of areas outside your change
- Building a mental map of the codebase
- Deciding which module to read deeper
- Answering "does this codebase already have X?"

### L2 — Mechanics (HOW Does It Work?)

**Time budget:** 2-5 minutes per function

**What to read:**
- Complete function body, line by line
- All branching logic (if/else, switch, ternary)
- Loop behavior and termination conditions
- Error handling (try/catch, result types)
- Side effects (mutations, API calls, file writes)

**Outcome:** Step-by-step understanding of the algorithm.

**Example:**
```typescript
// L2 understanding of syncInventory:
// 1. Validates storeId exists in database
// 2. Fetches remote inventory from API
// 3. Compares remote vs local item-by-item
// 4. Generates diff (added, removed, modified)
// 5. Applies diff to local database in a transaction
// 6. Returns SyncResult with counts and any errors
```

**When L2 is needed:**
- Fixing a bug in this function
- Adding a new feature that extends this function
- Writing tests for this function
- Understanding why a specific behavior occurs

### L3 — Design (WHY This Way?)

**Time budget:** 10-30 minutes per function/module

**What to read:**
- Code comments (especially "NOTE:", "HACK:", "TODO:", "FIXME:")
- Git log for this file: `git log --oneline -20 -- path/to/file`
- Git blame for specific lines: `git blame -L 42,60 path/to/file`
- Linked issue/PR references in commit messages
- PR descriptions and review comments
- Architecture decision records (ADRs) if they exist

**Outcome:** Understanding of constraints, trade-offs, and rejected alternatives.

**Example:**
```
// L3 understanding of syncInventory:
// - Uses item-by-item comparison instead of hash because of PR #234
//   where hash collisions caused silent data loss
// - Transaction is per-batch (100 items) not per-sync because of
//   SQLite write lock duration issues (commit abc1234)
// - Retry logic was added in response to intermittent API timeouts
//   (issue #89)
```

**When L3 is needed:**
- Refactoring (must understand WHY to know what to preserve)
- Proposing an alternative approach
- Architecture review
- Writing documentation

### L4 — Impact (What ELSE Is Affected?)

**Time budget:** 30-60 minutes per function/module

**What to do:**
- Grep the function name across the entire codebase
- Identify all callers (direct and indirect)
- Check all tests that exercise this function
- Trace through state management (if this state changes, what re-renders?)
- Map the blast radius of a change

**Tools:**
```bash
# Find all usages
grep -rn "syncInventory" src/

# Find all imports
grep -rn "from.*sync" src/

# Git log for related changes
git log --all --oneline -- '**/sync*'

# Find test files
find . -name "*sync*test*" -o -name "*sync*spec*"
```

**Outcome:** Complete impact map of what a change would affect.

**When L4 is needed:**
- Changing a public API / function signature
- Removing or deprecating functionality
- Making breaking changes
- Architecture changes that affect multiple modules

---

## Codebase Mapping Technique

When onboarding to a new codebase, build a mental (or written) architecture
map by answering these questions in order:

### Layer 1: The Box

```
What goes IN?    → User input, API requests, file uploads, IPC messages
What comes OUT?  → Rendered UI, API responses, file writes, notifications
What is STORED?  → Database records, file system, state store, cache
```

### Layer 2: The Components

```
How is input ROUTED?     → Router, IPC channels, event handlers
How is data VALIDATED?   → Schemas, type guards, middleware
How is logic ORGANIZED?  → Services, managers, controllers
How is data ACCESSED?    → Repositories, queries, ORM
How are errors HANDLED?  → Error boundaries, try/catch, result types
```

### Layer 3: The Connections

```
What calls what?           → Dependency graph (imports/requires)
What triggers what?        → Event flow (listeners, observers)
What shares state?         → State management (stores, context)
What runs in parallel?     → Workers, async operations, background tasks
What is the deploy unit?   → Monolith, services, packages
```

---

## Integration Point Checklist

When mapping a codebase, check for each of these 15 integration types:

1. **HTTP Client** — `fetch`, `axios`, `got` — outgoing API calls
2. **HTTP Server** — `express`, `fastify`, `koa` — incoming API routes
3. **WebSocket** — Real-time bidirectional communication
4. **Database** — SQL queries, ORM methods, connection pools
5. **File System** — `fs.readFile`, `fs.writeFile`, path operations
6. **IPC** — `ipcMain`, `ipcRenderer`, `contextBridge` (Electron)
7. **Child Process** — `spawn`, `exec`, `fork` — subprocess management
8. **Environment Variables** — `process.env`, `.env` files, config
9. **CLI Arguments** — `process.argv`, `commander`, `yargs`
10. **Timers** — `setTimeout`, `setInterval`, `cron` — scheduled work
11. **Event Emitters** — Custom event bus, Node EventEmitter
12. **Browser APIs** — `localStorage`, `sessionStorage`, `IndexedDB`
13. **OS APIs** — Clipboard, notifications, system tray, shortcuts
14. **Authentication** — Login flows, token management, session handling
15. **Logging** — Console, file logging, external log services

---

## Legacy Code Reading Checklist

When inheriting code you did not write:

- [ ] Read the README (if it exists) — but don't trust it blindly
- [ ] Check the last 20 commits — understand recent activity
- [ ] Run the test suite — how much passes? How much exists?
- [ ] Find the entry point — where does execution begin?
- [ ] Trace one happy path end-to-end
- [ ] Trace one error path end-to-end
- [ ] Identify the data model — what is stored and where?
- [ ] Map external dependencies — what external services are called?
- [ ] Check for configuration — environment variables, config files
- [ ] Look for documentation in unexpected places — wiki, Confluence, Notion, comments
- [ ] Identify patterns in use — MVC? Repository? Event-driven?
- [ ] Check for dead code — unused exports, unreachable branches
- [ ] Note any code smells — but don't fix them yet
- [ ] Build and run locally — can you actually start the application?

---

## Example: Reading an Electron + React App

Here is the strategic reading order for a typical Electron + React + TypeScript
application (like your Electron project):

### Pass 1: Project Shape (5 min)

```
1. package.json         → name, scripts, dependencies, electron version
2. tsconfig.json        → compiler options, paths, strict mode
3. vite.config.ts       → build config, aliases, plugins
4. Directory listing    → src/, electron/, public/, etc.
```

### Pass 2: Electron Main Process (10 min)

```
5. electron/main.ts     → app.whenReady, BrowserWindow creation
6. electron/preload.ts  → contextBridge.exposeInMainWorld
7. electron/ipc/        → IPC handler registrations (invoke/handle)
```

### Pass 3: React Application (15 min)

```
8. src/main.tsx          → ReactDOM.createRoot, providers, router
9. src/App.tsx           → Layout, route definitions, error boundaries
10. src/router.tsx       → Route tree (if separate file)
11. src/pages/           → Primary pages (follow main user flow)
```

### Pass 4: State Management (10 min)

```
12. src/stores/          → Zustand stores, state shape, actions
13. src/hooks/           → Custom hooks, especially data-fetching ones
14. src/context/         → React context providers (if any)
```

### Pass 5: Data Layer (10 min)

```
15. src/database/        → Schema, migrations, query builders
16. src/services/        → Business logic, data transformation
17. src/api/             → External API clients
```

### Pass 6: Supporting Code (as needed)

```
18. src/components/      → Shared UI components (read as encountered)
19. src/utils/           → Utilities (read as encountered)
20. src/types/           → Type definitions (reference as needed)
```

### Total Time: ~50 minutes for solid L1-L2 understanding

---

## Tools for Code Reading

### grep / ripgrep — Find Usages

```bash
# Find all usages of a function
rg "functionName" --type ts

# Find all imports of a module
rg "from.*moduleName" --type ts

# Find all TODO/FIXME/HACK comments
rg "(TODO|FIXME|HACK)" --type ts
```

### git log — Understand History

```bash
# Recent changes to a file
git log --oneline -20 -- path/to/file.ts

# Who changed this file and when
git log --format="%h %an %s" -- path/to/file.ts

# Changes between two dates
git log --since="2024-01-01" --until="2024-02-01" --oneline
```

### git blame — Understand Line-Level History

```bash
# Who wrote each line
git blame path/to/file.ts

# Blame specific lines
git blame -L 42,60 path/to/file.ts

# Ignore whitespace changes
git blame -w path/to/file.ts
```

### Dependency Graphs

```bash
# npm dependency tree
npm ls --depth=2

# Find circular dependencies
npx madge --circular src/

# Generate visual dependency graph
npx madge --image graph.svg src/
```

---

## Common Reading Mistakes to Avoid

1. **Reading alphabetically** — The file system order has nothing to do with
   execution order.

2. **Starting with utilities** — Utils are leaves. Start at the root (entry point).

3. **Reading every file** — You don't need L2 on everything. Most code needs
   only L1 understanding.

4. **Ignoring tests** — Tests are the best documentation of intended behavior.
   Read them early.

5. **Trusting comments blindly** — Comments can be outdated. Code is truth.
   Comments are suggestions.

6. **Not running the app** — Reading code without running it is like reading
   sheet music without hearing it. Run it first.

7. **Getting lost in abstractions** — When you hit an abstraction boundary,
   note it and come back. Don't follow every rabbit hole.

8. **Skipping error handling** — Error paths reveal assumptions, edge cases,
   and system boundaries. They are often more informative than happy paths.
