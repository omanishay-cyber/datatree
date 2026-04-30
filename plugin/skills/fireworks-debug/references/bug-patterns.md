# Bug Patterns — Electron + React + TypeScript + Windows

> Deep-dive reference for the most common bug patterns encountered in
> Electron desktop apps built with React, TypeScript, Zustand, and sql.js
> running on Windows.

---

## 1. Stale Closure

### Cause
When a `useEffect`, `setTimeout`, `setInterval`, or event handler captures a variable from its enclosing scope, it "closes over" the value at the time the closure was created. If the state changes later, the closure still sees the old value.

### Detection
```typescript
// In the callback, log the value you suspect is stale:
useEffect(() => {
  const handler = () => {
    console.log('Current count:', count); // Shows old value
  };
  window.addEventListener('click', handler);
  return () => window.removeEventListener('click', handler);
}, []); // Empty deps = closure captures initial count forever
```

### Fix
**Option A — Add to dependency array:**
```typescript
useEffect(() => {
  const handler = () => console.log('Count:', count);
  window.addEventListener('click', handler);
  return () => window.removeEventListener('click', handler);
}, [count]); // Re-creates handler when count changes
```

**Option B — Use a ref for latest value:**
```typescript
const countRef = useRef(count);
countRef.current = count; // Always up to date

useEffect(() => {
  const handler = () => console.log('Count:', countRef.current);
  window.addEventListener('click', handler);
  return () => window.removeEventListener('click', handler);
}, []); // Safe because ref.current is always fresh
```

### Prevention
- Enable `eslint-plugin-react-hooks` with `exhaustive-deps` rule.
- Use refs for values needed in long-lived callbacks.
- Prefer Zustand selectors over closed-over state for store values.

---

## 2. Silent IPC Failure

### Cause
The renderer calls `window.api.someMethod()` via the preload bridge, but nothing happens — no result, no error. This occurs when:
- The channel name in `ipcRenderer.invoke('channel')` does not match `ipcMain.handle('channel')`.
- The method is not exposed in `contextBridge.exposeInMainWorld`.
- The handler throws but the error is swallowed.

### Detection
Add console.log on BOTH sides of the IPC boundary:
```typescript
// In preload.ts:
someMethod: async (...args) => {
  console.log('[preload] someMethod called with:', args);
  const result = await ipcRenderer.invoke('some-channel', ...args);
  console.log('[preload] someMethod result:', result);
  return result;
};

// In main ipc-handlers.ts:
ipcMain.handle('some-channel', async (event, ...args) => {
  console.log('[main] some-channel handler called with:', args);
  // ... handler logic
});
```

### Fix
1. Verify channel name matches EXACTLY on both sides (case-sensitive).
2. Verify the method is exposed in `contextBridge.exposeInMainWorld`.
3. Verify the handler is registered (check that the file with `ipcMain.handle` is imported at startup).
4. Add try/catch in the handler and return structured errors:
```typescript
ipcMain.handle('some-channel', async (event, ...args) => {
  try {
    const result = await doWork(...args);
    return { success: true, data: result };
  } catch (error) {
    console.error('[main] some-channel error:', error);
    return { success: false, error: String(error) };
  }
});
```

### Prevention
- Use a typed IPC system: define channel names as a union type shared between main and renderer.
- Use a single source of truth for channel names (e.g., `ipc-channels.ts`).
- Always return structured responses: `{ success: boolean, data?: T, error?: string }`.

---

## 3. Async Ordering

### Cause
Multiple async operations are initiated, and they resolve in a different order than expected. For example:
- User types a search query. Each keystroke triggers a fetch.
- The fetch for "ab" resolves AFTER the fetch for "abc", overwriting the correct results.

### Detection
- Data appears correct briefly, then changes to wrong data.
- Console logs show responses arriving out of order.
- Adding a slight network delay makes the problem more obvious.

### Fix
**Option A — AbortController:**
```typescript
useEffect(() => {
  const controller = new AbortController();
  fetchData(query, { signal: controller.signal })
    .then(setData)
    .catch(err => {
      if (err.name !== 'AbortError') throw err;
    });
  return () => controller.abort();
}, [query]);
```

**Option B — Sequence Counter:**
```typescript
const requestIdRef = useRef(0);

useEffect(() => {
  const thisRequestId = ++requestIdRef.current;
  fetchData(query).then(data => {
    if (thisRequestId === requestIdRef.current) {
      setData(data); // Only apply if this is still the latest request
    }
  });
}, [query]);
```

### Prevention
- Use React Query, SWR, or TanStack Query which handle this automatically.
- For IPC: use the sequence counter pattern since AbortController does not work with IPC.

---

## 4. Windows Path Issues

### Cause
Windows uses backslashes (`\`) in file paths, but JavaScript string literals use backslash as an escape character. Additionally:
- Drive letters (`C:`) can cause issues with URL parsing.
- UNC paths (`\\server\share`) have different handling.
- Path length limits (260 chars by default on Windows) can truncate paths silently.

### Detection
- "File not found" or "ENOENT" errors that work fine on macOS/Linux.
- Paths in error messages show doubled backslashes or missing segments.
- `file://` URLs fail to load resources.

### Fix
```typescript
import path from 'path';

// WRONG:
const filePath = baseDir + '\\' + filename;
const filePath = `${baseDir}/${filename}`;

// RIGHT:
const filePath = path.join(baseDir, filename);

// For file:// URLs:
const fileUrl = new URL(`file:///${filePath.replace(/\\/g, '/')}`);

// For path normalization:
const normalized = path.normalize(filePath);
```

### Prevention
- NEVER concatenate paths as strings. Always use `path.join()` or `path.resolve()`.
- When converting to URLs, normalize backslashes to forward slashes.
- Use `app.getPath()` for standard directories (userData, documents, temp).
- Test on Windows — path bugs are invisible on macOS/Linux.

---

## 5. Database Race Conditions

### Cause
sql.js runs in-memory and is synchronous by nature, but when multiple async operations read/write to the database concurrently, the data can become inconsistent:
- Read-then-write is not atomic: another write can happen between the read and write.
- Concurrent writes can overwrite each other.
- Saving the database file while a write is in progress can corrupt it.

### Detection
- Data occasionally appears wrong or partially updated.
- Duplicate entries appear.
- Database file becomes corrupt after a crash.
- Bug is intermittent and hard to reproduce.

### Fix
```typescript
// Use an async mutex to serialize database access:
import { Mutex } from 'async-mutex';

const dbMutex = new Mutex();

async function safeDbOperation<T>(operation: () => T): Promise<T> {
  const release = await dbMutex.acquire();
  try {
    return operation();
  } finally {
    release();
  }
}

// Use transactions for multi-statement operations:
async function transferStock(fromId: number, toId: number, qty: number) {
  return safeDbOperation(() => {
    db.run('BEGIN TRANSACTION');
    try {
      db.run('UPDATE products SET stock = stock - ? WHERE id = ?', [qty, fromId]);
      db.run('UPDATE products SET stock = stock + ? WHERE id = ?', [qty, toId]);
      db.run('COMMIT');
    } catch (error) {
      db.run('ROLLBACK');
      throw error;
    }
  });
}
```

### Prevention
- Wrap all database operations behind a mutex or queue.
- Use transactions for any operation involving multiple statements.
- Save the database file on a debounced timer, not on every write.
- Keep database backups — save to a temp file first, then rename.

---

## 6. Type Assertion Crashes

### Cause
Using `as MyType` tells TypeScript to trust you, but if the runtime data does not match the asserted type, property access will fail:
```typescript
const data = JSON.parse(response) as Product; // What if response is not a Product?
console.log(data.name.toUpperCase()); // Crash if data.name is undefined
```

### Detection
- Runtime error: "Cannot read property 'X' of undefined" (or null).
- The TypeScript compiler shows no errors — because `as` silences them.
- The crash happens on data from external sources: IPC, database, file, network.

### Fix
```typescript
// Add runtime validation with a type guard:
function isProduct(data: unknown): data is Product {
  return (
    typeof data === 'object' &&
    data !== null &&
    'name' in data &&
    typeof (data as any).name === 'string' &&
    'price' in data &&
    typeof (data as any).price === 'number'
  );
}

const data = JSON.parse(response);
if (!isProduct(data)) {
  throw new Error(`Invalid product data: ${JSON.stringify(data)}`);
}
// Now TypeScript knows data is Product, safely.
```

### Prevention
- Never use `as` on data from external sources without runtime validation.
- Use Zod, Valibot, or custom type guards for parsing external data.
- Treat every IPC boundary and JSON.parse as an untrusted data source.

---

## 7. React Hydration Mismatch

### Cause
When server-rendered HTML differs from what the client renders on first pass. In Electron, this manifests when the initial render depends on data only available after mount (window size, platform info, user preferences loaded from DB).

### Detection
- Console warning: "Text content did not match" or "Hydration failed."
- Visual flicker on initial load — content jumps.
- Content appears differently on first render vs subsequent renders.

### Fix
```typescript
// Use useEffect for client-only content:
const [isClient, setIsClient] = useState(false);
useEffect(() => setIsClient(true), []);

return isClient ? <DynamicContent /> : <Placeholder />;
```

### Prevention
- Do not access `window`, `navigator`, or Electron APIs during initial render.
- Use lazy initialization for state that depends on runtime values.
- Keep the initial render deterministic and environment-independent.

---

## 8. Memory Leaks

### Cause
Resources allocated during component lifecycle are not released when the component unmounts:
- Event listeners added but not removed.
- Timers (setInterval, setTimeout) started but not cleared.
- Subscriptions (store, WebSocket) opened but not closed.
- IPC listeners registered but not unregistered.

### Detection
- App becomes slower over time.
- Memory usage in Task Manager / DevTools grows continuously.
- Events fire multiple times for a single action.
- Console logs appear duplicated.

### Fix
```typescript
useEffect(() => {
  const handler = (event: Event) => { /* ... */ };
  window.addEventListener('resize', handler);

  const timer = setInterval(() => { /* ... */ }, 1000);

  const unsubscribe = store.subscribe((state) => { /* ... */ });

  // CLEANUP — return a function that undoes everything:
  return () => {
    window.removeEventListener('resize', handler);
    clearInterval(timer);
    unsubscribe();
  };
}, []);
```

### Prevention
- Every `addEventListener` must have a matching `removeEventListener` in cleanup.
- Every `setInterval`/`setTimeout` must have a matching `clearInterval`/`clearTimeout`.
- Every subscription must have a matching unsubscribe.
- Use the React DevTools Profiler to detect components that mount but never unmount.

---

## 9. Z-Index Wars

### Cause
Arbitrary z-index values collide, creating unpredictable stacking. A modal appears behind an overlay. A dropdown is hidden by a sibling.

### Detection
- Element visually appears behind another element it should be above.
- Clicking an area interacts with the wrong element.
- The issue appears only with certain component combinations.

### Fix
Define a z-index scale and stick to it:
```typescript
// z-index-scale.ts
export const Z_INDEX = {
  base: 0,
  dropdown: 10,
  sticky: 20,
  overlay: 30,
  modal: 40,
  popover: 50,
  toast: 60,
  tooltip: 70,
  devtools: 100,
} as const;
```

Also check for stacking context creation — `transform`, `opacity < 1`, `filter`, `position: fixed/sticky` all create new stacking contexts, resetting z-index within them.

### Prevention
- Never use arbitrary z-index values (z-index: 9999).
- Use a defined scale imported from a single source.
- Understand stacking contexts — a high z-index inside a low stacking context still loses.

---

## 10. IPC Timeout / Freeze

### Cause
The main process handler for an IPC call performs a synchronous, long-running operation (heavy computation, synchronous file I/O, blocking database query). The renderer waits for the response, freezing the UI.

### Detection
- UI freezes when a specific action is performed.
- The freeze resolves after several seconds (when the operation completes).
- No error is thrown — the operation succeeds, just slowly.
- DevTools shows the renderer is "waiting" (gray in Performance tab).

### Fix
```typescript
// WRONG — synchronous heavy operation:
ipcMain.handle('process-data', (event, data) => {
  const result = heavyComputation(data); // Blocks the main process
  return result;
});

// RIGHT — async with worker or chunked:
ipcMain.handle('process-data', async (event, data) => {
  // Option A: Use worker_threads for CPU-intensive work
  const result = await runInWorker(heavyComputation, data);
  return result;

  // Option B: Use async file I/O
  const fileData = await fs.promises.readFile(filePath);
  return processData(fileData);
});
```

### Prevention
- Never use `fs.readFileSync`, `fs.writeFileSync`, or other sync APIs in IPC handlers.
- Use `worker_threads` for CPU-intensive operations.
- Add timeouts on the renderer side so the UI can show a loading state.
- Consider streaming large results via IPC events instead of a single invoke/handle.

---

## Latest Debugging Best Practices (2025-2026)

### Electron Debugging

**Main Process Debugging with Inspector**
```bash
# Launch Electron with debugger attached (pauses on first line)
electron --inspect-brk=9229 .

# Launch without pausing (attach later)
electron --inspect=9229 .

# Then open chrome://inspect in any Chromium browser and connect
```

**VS Code launch.json for Main Process**
```json
{
  "type": "node",
  "request": "launch",
  "name": "Debug Main Process",
  "runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron",
  "runtimeArgs": ["--remote-debugging-port=9222", "."],
  "windows": { "runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron.cmd" }
}
```

**Memory Leak Detection**
- JavaScript heap snapshots do NOT show native memory. Always track `process.memoryUsage().rss` alongside V8 heap for complete investigation.
- Use DevTools Memory tab: Take Heap Snapshot, look for large retained object groups and detached DOM trees.
- Monitor growing event listener counts:
```typescript
process.on('warning', (warning) => {
  console.warn('[Memory Warning]', warning.name, warning.message);
});
```

**IPC Debugging & Tracing**
- Add bidirectional logging at every IPC boundary (preload + main) during debug.
- Track unregistered listeners: `ipcMain.on` / `ipcRenderer.on` listeners that survive context reloads are the #1 IPC leak source.
- Use `removeListener()` or `removeAllListeners()` on component unmount.
- Batch IPC messages and throttle chatty channels; large payloads stall renderers and starve the main loop.
- Always use `electron-log` for production logging (works in both main and renderer, writes to platform-appropriate directories).

**Production Safety Checks**
```typescript
// Never ship with DevTools accessible
if (!app.isPackaged) {
  mainWindow.webContents.openDevTools();
}
```

**Renderer Process Profiling**
- DevTools Performance tab: record, then look for gray "waiting" blocks (IPC stalls).
- DevTools Application tab: check for storage leaks (IndexedDB, localStorage growing unbounded).
- Use `webContents.on('render-process-gone', ...)` to catch renderer crashes in production.

### React 18 Concurrent Debugging

**Understanding Concurrent Rendering Quirks**
- Components using concurrent rendering may render TWICE: once for a high-priority update and once for a low-priority one. This is intentional, not a bug. Values can differ between renders, which makes console.log debugging confusing.
- Strict Mode intentionally double-invokes render functions to surface impure renders. Disable temporarily if it masks real issues, but fix the impurity.

**Debugging useTransition and useDeferredValue**
```typescript
// useTransition: mark state updates as non-urgent
const [isPending, startTransition] = useTransition();

// Debug: log when transitions are pending
useEffect(() => {
  if (isPending) console.log('[Transition] Update deferred, showing stale content');
}, [isPending]);

// useDeferredValue: defer a value from props/libraries
const deferredQuery = useDeferredValue(query);

// Debug: detect when deferred value lags behind
useEffect(() => {
  if (query !== deferredQuery) {
    console.log('[Deferred] Stale value shown:', deferredQuery, 'Current:', query);
  }
}, [query, deferredQuery]);
```

**Tearing Detection**
- Tearing happens when paused concurrent renders read different versions of external state.
- Solution: all external stores MUST use `useSyncExternalStore` (Redux and Zustand do this internally since 2023+).
- Test tool: `dai-shi/will-this-react-global-state-work-in-concurrent-rendering` on GitHub to verify your store library.

**Suspense Boundary Debugging**
- Excessive nesting causes waterfall loading. Structure Suspense at logical data boundaries, not random component levels.
- Errors inside Suspense bubble to the nearest ErrorBoundary, NOT the Suspense component. Always pair them:
```tsx
<ErrorBoundary fallback={<ErrorUI />}>
  <Suspense fallback={<Skeleton />}>
    <DataComponent />
  </Suspense>
</ErrorBoundary>
```
- Add logging to promise-throwing components to track the data loading lifecycle:
```typescript
// In your data-fetching wrapper
function wrapPromise<T>(promise: Promise<T>) {
  let status = 'pending';
  let result: T;
  const suspender = promise.then(
    (r) => { status = 'success'; result = r; console.log('[Suspense] Data resolved'); },
    (e) => { status = 'error'; result = e; console.error('[Suspense] Data failed', e); }
  );
  return {
    read() {
      if (status === 'pending') { console.log('[Suspense] Throwing promise (suspending)'); throw suspender; }
      if (status === 'error') throw result;
      return result;
    }
  };
}
```

**React DevTools Profiler Tips**
- Use the Profiler tab to identify which component triggered a re-render and why.
- Enable "Highlight updates when components render" to visually see unnecessary re-renders.
- Filter commits by duration to find the slowest renders first.
- React 19+ adds Performance Tracks in browser DevTools for visualizing concurrent scheduling.

### TypeScript Advanced Debugging

**Source Map Configuration (Critical for Electron)**
```json
// tsconfig.json — always enable for debugging
{
  "compilerOptions": {
    "sourceMap": true,
    "declaration": true,
    "declarationMap": true,
    "inlineSources": true
  }
}
```

**Conditional Breakpoints in VS Code**
- Right-click the gutter > "Add Conditional Breakpoint" to avoid breaking on every loop iteration.
- Use "Expression" mode: `item.id === 42` only breaks when condition is true.
- Use "Hit Count" mode: `5` breaks only on the 5th hit.
- Known issue: source-mapped TypeScript variable names may differ from generated JS names in conditional expressions. Use the JS variable name if conditions don't trigger.

**Type Narrowing as a Debugging Tool**
```typescript
// Use discriminated unions to make impossible states unrepresentable
type Result<T> =
  | { status: 'loading' }
  | { status: 'error'; error: Error }
  | { status: 'success'; data: T };

// Exhaustive checking catches missing cases at compile time
function handleResult<T>(result: Result<T>) {
  switch (result.status) {
    case 'loading': return /* ... */;
    case 'error': return /* ... */;
    case 'success': return /* ... */;
    default: {
      const _exhaustive: never = result; // Compile error if a case is missing
      throw new Error(`Unhandled status: ${JSON.stringify(_exhaustive)}`);
    }
  }
}
```

**Runtime Validation at Boundaries**
```typescript
// Use Zod for IPC/API boundary validation (catches bugs before they propagate)
import { z } from 'zod';

const ProductSchema = z.object({
  id: z.number(),
  name: z.string(),
  price: z.number().positive(),
});

// In IPC handler:
ipcMain.handle('get-product', async (_, id: number) => {
  const raw = db.get('SELECT * FROM products WHERE id = ?', [id]);
  const parsed = ProductSchema.safeParse(raw);
  if (!parsed.success) {
    console.error('[DB] Invalid product data:', parsed.error.flatten());
    throw new Error('Data validation failed');
  }
  return parsed.data;
});
```

**Strict Mode as Prevention**
- `"strict": true` in tsconfig.json enables a suite of checks (noImplicitAny, strictNullChecks, strictFunctionTypes, etc.) that can reduce debugging time by up to 60% compared to plain JavaScript.
- Add `"noUncheckedIndexedAccess": true` to catch undefined array/object access at compile time.
- Use `satisfies` operator to validate types without widening:
```typescript
const config = {
  theme: 'dark',
  fontSize: 14,
} satisfies Record<string, string | number>;
// TypeScript knows config.theme is string, config.fontSize is number
```

**Turbo Console Log (VS Code Extension)**
- Automatically inserts meaningful console.log statements with variable name, file, and line number.
- Keyboard shortcut: select variable, press `Ctrl+Alt+L` to insert a log line below.
- Delete all inserted logs at once with a command, keeping code clean.

### AI-Assisted Debugging Patterns

**Context Packing (Most Important Technique)**
Before asking an LLM to debug, do a "brain dump" of everything it needs:
1. The exact error message and stack trace
2. The code that triggers the error (full function, not snippets)
3. What you expected vs. what happened
4. What you already tried
5. Project constraints (e.g., "We use Zustand, not Redux" or "This runs in Electron main process")
6. Relevant config files (tsconfig.json, package.json versions)

**Structured Debugging Prompts**
```
Role: You are debugging an Electron + React 18 + TypeScript application.
Context: [paste relevant code]
Error: [exact error message and stack trace]
Environment: Electron 40, Node 20, Windows 11
Constraint: Must not use `any` types. Must work in both main and renderer processes.
Task: Find the root cause and provide a fix with explanation.
```

**Self-Repair Loop Pattern**
Modern AI debugging follows a generate-test-fix cycle:
1. AI generates a fix
2. Run the code / tests automatically
3. Feed the error output back to the AI
4. AI refines the fix based on actual results
5. Repeat until tests pass (max 3 iterations, then escalate to human)

**Supervised Debugging Workflow (Recommended)**
- Treat the AI like a junior developer giving a first draft.
- Let it generate and run code, but review each step.
- Give specific feedback: "The fix works but use the built-in array filter instead of a for loop."
- Add constraints iteratively: "Also ensure O(n) time complexity."

**Chain-of-Thought Debugging**
- Ask the AI to reason step-by-step rather than jump to a fix.
- Prompt pattern: "Before suggesting a fix, trace the data flow from [entry point] to [error location] and identify where the value becomes incorrect."
- This surfaces the AI's reasoning, making outputs more accurate and auditable.

**CLAUDE.md / INVARIANTS.md as AI Guardrails**
- Define process rules in configuration files that the AI reads at session start.
- Include: coding style, forbidden patterns, required verification steps, known pitfalls.
- Example rules: "Never use `as` on IPC data", "Always check both dark and light themes", "Run tsc --noEmit before claiming done."

**CLI-Based AI Debugging Tools (2025-2026)**
- **Claude Code**: Chat with Claude directly in your project directory. It reads files, runs tests, and multi-step fixes issues.
- **GitHub Copilot Agent**: Clones your repo into a cloud VM, works on tasks in background, opens a PR when done.
- **Google Jules / Gemini CLI**: Asynchronous coding agents for background bug-fixing.
- **Debug-gym (Microsoft Research)**: Expands AI agent capabilities with breakpoints, code navigation, variable inspection, and test function creation.

**Effectiveness Metrics**
- AI-assisted debugging tools speed up bug resolution by ~40% on average.
- Studies show 87% success rate diagnosing and fixing defects with just 1-2 queries when proper context is provided.
- The biggest ROI comes from context quality, not model selection: a well-prompted smaller model outperforms a poorly-prompted larger one.
