# Codebase Mapping — Deep Reference

> Techniques for understanding, mapping, and documenting existing codebases. This is the foundation of all research — understand what exists before looking elsewhere.

---

## Architecture Mapping

### Step 1: Identify Entry Points

Every application has entry points where execution begins. Find them first.

```bash
# Electron apps — look for main process entry
Glob: "**/main.ts", "**/main.js", "**/electron-main.*"
Grep: "app.whenReady" or "app.on('ready'"

# Renderer entry — where the UI starts
Glob: "**/renderer/index.*", "**/src/main.tsx", "**/src/App.tsx"

# Preload scripts — the bridge between worlds
Glob: "**/preload.*"
Grep: "contextBridge.exposeInMainWorld"

# Config files — define how things are built
Read: package.json, tsconfig.json, vite.config.ts, electron-builder.yml
```

### Step 2: Trace Imports to Build Dependency Graph

Starting from each entry point, trace imports to understand the dependency tree.

```bash
# Generate visual dependency graph
npx madge --image graph.svg src/

# Find circular dependencies (these are bugs waiting to happen)
npx madge --circular src/

# List orphaned files (imported by nothing)
npx madge --orphans src/

# List files with most dependents (high-impact files)
npx madge --summary src/
```

### Step 3: Layer Identification

Most applications follow a layered architecture. Identify the boundaries.

```
LAYER 1: UI Components
  Location: src/components/, src/pages/, src/views/
  Responsibility: Rendering, user interaction, local UI state
  Depends on: State layer, utility layer

LAYER 2: State Management
  Location: src/stores/, src/state/, src/hooks/
  Responsibility: Application state, business logic
  Depends on: IPC/API layer, utility layer

LAYER 3: IPC Bridge (Electron-specific)
  Location: src/preload/, src/ipc/
  Responsibility: Communication between renderer and main process
  Depends on: Type definitions shared between layers

LAYER 4: Main Process Handlers
  Location: src/main/, src/handlers/, src/services/
  Responsibility: System access, file I/O, database, native APIs
  Depends on: Data layer, utility layer

LAYER 5: Data Layer
  Location: src/database/, src/models/, src/repositories/
  Responsibility: Data persistence, queries, migrations
  Depends on: Database driver (sql.js, better-sqlite3, etc.)

LAYER 6: Utility / Shared
  Location: src/utils/, src/lib/, src/shared/
  Responsibility: Pure functions, type definitions, constants
  Depends on: Nothing (or only external libraries)
```

---

## Pattern Recognition

### Naming Conventions Checklist

When reading a new codebase, document these patterns:

```
Files:
  Components: PascalCase.tsx? kebab-case.tsx? feature/Component.tsx?
  Stores: *.store.ts? *Store.ts? use*.ts?
  Utilities: *.utils.ts? *.helpers.ts? *.lib.ts?
  Types: *.types.ts? *.d.ts? inline?
  Tests: *.test.ts? *.spec.ts? __tests__/?

Variables:
  Components: PascalCase? (React standard)
  Hooks: usePrefix? (React convention)
  Constants: UPPER_SNAKE? camelCase?
  Types/Interfaces: IPrefix? TPrefix? No prefix?
  Enums: PascalCase? UPPER_SNAKE values?

Functions:
  Event handlers: handleX? onX?
  Async: async/await? .then()? Both?
  Error handling: try/catch? Result type? Error boundary?
```

### File Organization Patterns

```
Feature-based (preferred for large apps):
  src/features/inventory/
    components/
    stores/
    hooks/
    types.ts
    index.ts

Layer-based (simpler apps):
  src/components/
  src/stores/
  src/hooks/
  src/types/

Hybrid (most common):
  src/components/     (shared components)
  src/features/       (feature-specific code)
  src/stores/         (global stores)
  src/utils/          (shared utilities)
```

---

## Data Flow Tracing

### End-to-End Trace Template

Trace a single user action through the entire system:

```
USER ACTION: [e.g., "User clicks Save button on invoice form"]

1. COMPONENT: InvoiceForm.tsx
   - Button onClick handler fires
   - Collects form data from local state
   - Calls store action: useInvoiceStore.getState().saveInvoice(data)

2. STORE: invoiceStore.ts
   - saveInvoice action validates data
   - Transforms data to IPC format
   - Calls: window.api.invoice.save(transformedData)

3. PRELOAD: preload.ts
   - Exposes: contextBridge.exposeInMainWorld('api', { invoice: { save: ... } })
   - Forwards call: ipcRenderer.invoke('invoice:save', data)

4. MAIN HANDLER: invoiceHandler.ts
   - ipcMain.handle('invoice:save', async (event, data) => { ... })
   - Validates data server-side
   - Calls repository: invoiceRepo.save(data)

5. DATA LAYER: invoiceRepository.ts
   - Constructs SQL INSERT/UPDATE
   - Executes against sql.js database
   - Returns result (success/failure + ID)

6. RESPONSE PATH: (reverse)
   - Handler returns result to IPC
   - Preload forwards to renderer
   - Store receives result, updates state
   - Component re-renders with new data
   - User sees success notification
```

### Useful Grep Patterns for Tracing

```bash
# Find all IPC channels
Grep: "ipcMain.handle|ipcMain.on|ipcRenderer.invoke|ipcRenderer.send"

# Find all store actions
Grep: "set\(\(state\)|getState\(\)"

# Find all event handlers in components
Grep: "onClick|onChange|onSubmit|onKeyDown"

# Find all database queries
Grep: "db\.run|db\.exec|db\.prepare|\.query\("
```

---

## Dependency Analysis

### Health Indicators

```bash
# Check for outdated dependencies
npm outdated

# Check for security vulnerabilities
npm audit

# Check dependency tree depth
npm ls --depth=3

# Find duplicate packages
npm ls --all | sort | uniq -d
```

### Coupling Hotspots

Files imported by many others are coupling hotspots. Changes to these files have wide blast radius.

```bash
# Find most-imported files (high coupling)
npx madge --summary src/ | sort -t: -k2 -rn | head -20

# Find files that import the most (high dependency)
npx madge src/ --json | node -e "
  const data = JSON.parse(require('fs').readFileSync('/dev/stdin','utf8'));
  Object.entries(data).sort((a,b) => b[1].length - a[1].length).slice(0,20)
    .forEach(([f,deps]) => console.log(deps.length, f));
"
```
