# Error Fingerprints — Classification Taxonomy

> Systematic approach to classifying errors by type, extracting fingerprints,
> and matching against known patterns for rapid diagnosis.

---

## 1. The 8 Error Categories

Every error falls into one of these categories. Classify FIRST, then debug.

### Category 1: Type Errors
**Signature**: `TypeError`, property access on wrong type, function call on non-function.
**Root Causes**: Missing null check, wrong import, stale closure, type assertion lie.
**First Action**: Check the variable — what is its actual value at runtime?

### Category 2: Reference Errors
**Signature**: `ReferenceError`, variable not defined, module not found.
**Root Causes**: Typo in variable name, missing import, wrong scope, build config issue.
**First Action**: Check spelling, check imports, check scope chain.

### Category 3: State Errors
**Signature**: Wrong data displayed, stale values, missing updates, infinite re-renders.
**Root Causes**: Stale closure, wrong deps array, missing store subscription, race condition.
**First Action**: Log the state at the point of use — is it what you expect?

### Category 4: Environment Errors
**Signature**: `ENOENT`, `EACCES`, `EPERM`, path issues, permission denied, module ABI mismatch.
**Root Causes**: Wrong file path, missing directory, wrong Node version, platform difference.
**First Action**: Log the path/environment — is it what you expect? Check `process.platform`, `process.arch`.

### Category 5: Constraint Errors
**Signature**: `SQLITE_CONSTRAINT`, validation failures, duplicate key, foreign key violation.
**Root Causes**: Duplicate data, missing parent record, invalid input not caught by validation.
**First Action**: Check the data being inserted — does it violate a constraint?

### Category 6: Network Errors
**Signature**: `ECONNREFUSED`, `ETIMEDOUT`, `ERR_NETWORK`, CORS errors, fetch failures.
**Root Causes**: Server down, wrong URL, CORS misconfiguration, network offline, SSL issue.
**First Action**: Check network tab — was the request sent? What was the response?

### Category 7: Memory Errors
**Signature**: `ENOMEM`, `heap out of memory`, app slowing over time, renderer crash (OOM).
**Root Causes**: Memory leak (event listeners, closures, detached DOM), processing too much data.
**First Action**: Open DevTools Memory tab. Take heap snapshots 1 minute apart. Compare.

### Category 8: Build Errors
**Signature**: `tsc` errors, Vite build failures, Electron builder failures, module resolution.
**Root Causes**: Type mismatch, missing dependency, wrong config, incompatible versions.
**First Action**: Read the exact error code and message. Check `references/build-errors.md`.

---

## 2. Fingerprint Extraction Method

An error fingerprint uniquely identifies a class of errors. Extract these 4 components:

### Component 1: Error Name
The constructor name of the error: `TypeError`, `ReferenceError`, `SyntaxError`, etc.

### Component 2: Message Pattern
The error message with variable parts replaced by wildcards:
- `Cannot read properties of undefined (reading 'name')` becomes `Cannot read properties of undefined (reading '*')`
- `TS2345: Argument of type 'string' is not assignable to parameter of type 'number'` becomes `TS2345: Argument of type '*' is not assignable to parameter of type '*'`

### Component 3: Top 3 Stack Frames (YOUR code only)
Skip node_modules, skip framework internals. Extract the top 3 frames from YOUR code:
```
1. src/renderer/pages/Products.tsx:45
2. src/renderer/stores/productStore.ts:23
3. src/renderer/hooks/useProducts.ts:12
```

### Component 4: Process Context
Where the error occurred:
- `main` — Electron main process
- `renderer` — Electron renderer process (browser window)
- `preload` — Preload script
- `build` — Build/compile time (tsc, Vite)
- `test` — Test runner

### Combined Fingerprint Format
```
[ErrorName] | [MessagePattern] | [TopFrame] | [Process]
TypeError | Cannot read properties of undefined (reading '*') | Products.tsx:45 | renderer
```

---

## 3. Auto-Classification Patterns

Use these regex patterns to auto-classify errors:

```typescript
const ERROR_PATTERNS: Array<{ pattern: RegExp; category: string; action: string }> = [
  // Type Errors
  { pattern: /Cannot read propert/i, category: 'Type', action: 'Check null/undefined access' },
  { pattern: /is not a function/i, category: 'Type', action: 'Check import, check method exists' },
  { pattern: /is not iterable/i, category: 'Type', action: 'Check value is array/iterable' },
  { pattern: /Cannot convert undefined/i, category: 'Type', action: 'Add null check before operation' },

  // Reference Errors
  { pattern: /is not defined/i, category: 'Reference', action: 'Check import, check spelling' },
  { pattern: /Cannot find module/i, category: 'Reference', action: 'Check path, run npm install' },
  { pattern: /Cannot find name/i, category: 'Reference', action: 'Add import or declaration' },

  // State Errors
  { pattern: /Too many re-renders/i, category: 'State', action: 'Check for setState in render' },
  { pattern: /Cannot update.*while rendering/i, category: 'State', action: 'Move setState to useEffect' },
  { pattern: /Maximum call stack/i, category: 'State', action: 'Check for infinite recursion/loop' },

  // Environment Errors
  { pattern: /ENOENT/i, category: 'Environment', action: 'Check file path exists' },
  { pattern: /EACCES|EPERM/i, category: 'Environment', action: 'Check file permissions' },
  { pattern: /MODULE_NOT_FOUND/i, category: 'Environment', action: 'Run npm install, check path' },

  // Constraint Errors
  { pattern: /SQLITE_CONSTRAINT/i, category: 'Constraint', action: 'Check data for duplicates/missing refs' },
  { pattern: /UNIQUE constraint/i, category: 'Constraint', action: 'Use INSERT OR REPLACE or check exists' },
  { pattern: /FOREIGN KEY/i, category: 'Constraint', action: 'Insert parent record first' },

  // Network Errors
  { pattern: /ECONNREFUSED/i, category: 'Network', action: 'Check server is running' },
  { pattern: /CORS/i, category: 'Network', action: 'Use IPC for requests from main process' },
  { pattern: /ETIMEDOUT/i, category: 'Network', action: 'Check network, increase timeout' },
  { pattern: /ERR_NETWORK/i, category: 'Network', action: 'Check connectivity, check URL' },

  // Memory Errors
  { pattern: /heap out of memory/i, category: 'Memory', action: 'Check for leaks, increase limit' },
  { pattern: /ENOMEM/i, category: 'Memory', action: 'Reduce data size, process in chunks' },

  // Build Errors
  { pattern: /TS\d{4}:/i, category: 'Build', action: 'Check references/build-errors.md' },
  { pattern: /Failed to resolve/i, category: 'Build', action: 'Check import path, run npm install' },
  { pattern: /Failed to parse source/i, category: 'Build', action: 'Check for syntax errors' },
];
```

---

## 4. Error Deduplication

When the same error occurs multiple times, deduplicate using the fingerprint:

1. Extract fingerprint for each error occurrence
2. Group by fingerprint — same fingerprint = same root cause
3. Fix the root cause once — all occurrences resolve
4. If the same fingerprint recurs after a fix, the fix was incomplete

---

## 5. Error Registry Integration

After fixing an error, add it to `~/.claude/evolution/error-registry.json`:

```json
{
  "fingerprint": "TypeError | Cannot read properties of undefined (reading '*') | ProductList.tsx:* | renderer",
  "category": "Type",
  "rootCause": "Product data loaded async, component renders before data arrives",
  "fix": "Added optional chaining and loading state check",
  "preventionRule": "Always add loading/error states for async data",
  "dateFixed": "2026-03-26",
  "recurrenceCount": 0
}
```

This builds institutional knowledge across sessions.
