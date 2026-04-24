# fireworks-performance — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 4. Memory Leak Detection Checklist

### The Usual Suspects

Check EVERY item when investigating memory issues:

- [ ] **Event listeners without removeEventListener** — especially `window.addEventListener`
- [ ] **setInterval without clearInterval** — timers run forever if not cleared
- [ ] **setTimeout in unmounted components** — callback fires after unmount
- [ ] **Zustand subscriptions without unsubscribe** — `store.subscribe()` returns unsubscribe function
- [ ] **IPC listeners without removeListener** — `ipcRenderer.on()` accumulates listeners
- [ ] **DOM references in closures** — old DOM nodes kept alive by event handlers
- [ ] **Growing arrays/maps without limits** — history, logs, cache without eviction
- [ ] **Promises that never resolve** — keep the entire scope alive
- [ ] **WebSocket connections not closed** — hold references to callbacks
- [ ] **AbortController not used with fetch** — requests continue after component unmount

### The Fix Pattern

```tsx
useEffect(() => {
  // Setup
  const controller = new AbortController();
  const handler = () => { /* ... */ };
  window.addEventListener('resize', handler);

  const unsubscribe = store.subscribe(handler);

  const intervalId = setInterval(() => { /* ... */ }, 1000);

  const ipcHandler = (_event: any, data: any) => { /* ... */ };
  window.electron.ipcRenderer.on('channel', ipcHandler);

  // ALWAYS return cleanup
  return () => {
    controller.abort();
    window.removeEventListener('resize', handler);
    unsubscribe();
    clearInterval(intervalId);
    window.electron.ipcRenderer.removeListener('channel', ipcHandler);
  };
}, []);
```

> See `references/memory-leaks.md` for detection tools and advanced patterns.

---

## 5. Vite Optimization Quick-Reference

### Manual Chunks (Vendor Splitting)

```ts
// vite.config.ts
export default defineConfig({
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom', 'react-router-dom'],
          'vendor-ui': ['framer-motion', '@radix-ui/react-dialog'],
          'vendor-state': ['zustand', 'immer'],
          'vendor-utils': ['date-fns', 'clsx', 'tailwind-merge'],
        },
      },
    },
  },
});
```

### Critical Performance Options

```ts
export default defineConfig({
  build: {
    minify: 'esbuild',          // Faster than terser, good enough
    cssCodeSplit: true,          // Load CSS per route, not all at once
    reportCompressedSize: true,  // Show gzipped sizes in build output
    chunkSizeWarningLimit: 500,  // Warn on chunks >500KB
    target: 'chrome114',         // Electron's Chrome version — enables modern syntax
    sourcemap: false,            // Disable in production for size
  },
  optimizeDeps: {
    include: ['react', 'react-dom', 'zustand'], // Pre-bundle for faster dev startup
  },
});
```

### HMR Configuration

```ts
export default defineConfig({
  server: {
    hmr: {
      overlay: true,  // Show errors in browser overlay
    },
    warmup: {
      clientFiles: ['./src/App.tsx', './src/main.tsx'], // Pre-transform critical files
    },
  },
});
```

### Dead Code Elimination

```ts
export default defineConfig({
  define: {
    __DEV__: JSON.stringify(process.env.NODE_ENV !== 'production'),
  },
});
// Usage: if (__DEV__) { console.log('debug info'); }
// Stripped entirely from production builds
```

> See `references/vite-config.md` for complete configuration guide.

---

## 6. Electron Startup Optimization

### Critical Path

```
app.whenReady() -> createWindow() -> loadURL() -> DOMContentLoaded -> First Paint
```

Every millisecond on this path delays the user seeing the app.

### Optimization Strategies

1. **Defer non-critical work** — Move database init, sync checks, and plugin loading AFTER the window is visible.

```ts
// BAD: blocking startup
await initDatabase();
await checkForUpdates();
await loadPlugins();
createWindow();

// GOOD: show window fast, then do work
createWindow();
// After window is shown:
queueMicrotask(async () => {
  await initDatabase();
  await checkForUpdates();
  await loadPlugins();
});
```

2. **Lazy-load heavy modules** — Don't require everything at the top of main.ts.

```ts
// BAD: loads sql.js at startup even if not needed yet
import initSqlJs from 'sql.js';

// GOOD: load when first needed
let db: Database | null = null;
async function getDatabase() {
  if (!db) {
    const SQL = await import('sql.js');
    const initSqlJs = SQL.default;
    const sqlPromise = initSqlJs();
    db = (await sqlPromise).Database();
  }
  return db;
}
```

3. **Splash screen pattern** — Show a lightweight window immediately, load the full app behind it.

4. **Preload script optimization** — Keep preload scripts minimal. Only expose the IPC bridge.

> See `references/electron-perf.md` for detailed startup profiling.

---

## 7. IPC Performance

### Batch Multiple Calls

```ts
// BAD: 10 separate IPC round-trips
const name = await invoke('get-setting', 'name');
const theme = await invoke('get-setting', 'theme');
const lang = await invoke('get-setting', 'language');

// GOOD: 1 IPC call
const settings = await invoke('get-settings', ['name', 'theme', 'language']);
```

### Debounce Frequent Updates

```ts
// BAD: sends IPC on every keystroke
input.addEventListener('input', (e) => {
  invoke('search', e.target.value);
});

// GOOD: debounce to max 60fps
import { debounce } from 'lodash-es';
const debouncedSearch = debounce((value: string) => {
  invoke('search', value);
}, 16); // ~60fps
```

### Minimize Payload Size

```ts
// BAD: sending entire database row with all columns
invoke('save-product', fullProductObject);

// GOOD: send only changed fields
invoke('update-product', { id: product.id, price: newPrice });
```

### Use Structured Clone for Large Data

```ts
// For large arrays/objects, structured clone is faster than JSON serialization
// Electron 28+ uses structured clone by default for IPC
// Ensure you're not accidentally serializing with JSON.stringify
```

---

## 8. sql.js Query Optimization

### Index Strategy

```sql
-- Create indexes for columns used in WHERE, JOIN, ORDER BY
CREATE INDEX IF NOT EXISTS idx_products_name ON products(name);
CREATE INDEX IF NOT EXISTS idx_products_category ON products(category_id);
CREATE INDEX IF NOT EXISTS idx_sales_date ON sales(sale_date);

-- Composite index for common queries
CREATE INDEX IF NOT EXISTS idx_sales_date_product ON sales(sale_date, product_id);
```

### EXPLAIN QUERY PLAN

```sql
-- Always check query plan for slow queries
EXPLAIN QUERY PLAN SELECT * FROM products WHERE category_id = 5 ORDER BY name;
-- Look for: SCAN TABLE (bad) vs SEARCH TABLE USING INDEX (good)
```

### Pagination with LIMIT/OFFSET

```sql
-- BAD: loading all products at once
SELECT * FROM products;

-- GOOD: paginate
SELECT * FROM products ORDER BY id LIMIT 50 OFFSET 0;
-- Next page: OFFSET 50, then 100, etc.
```

### Prepared Statements

```ts
// BAD: re-parsing SQL every call
function getProduct(id: number) {
  return db.exec(`SELECT * FROM products WHERE id = ${id}`);
}

// GOOD: prepare once, bind parameters
const stmt = db.prepare('SELECT * FROM products WHERE id = ?');
function getProduct(id: number) {
  stmt.bind([id]);
  const result = stmt.getAsObject();
  stmt.reset();
  return result;
}
```

### Batch Inserts with Transactions

```ts
// BAD: auto-commit per row (100 rows = 100 disk writes)
for (const product of products) {
  db.run('INSERT INTO products VALUES (?, ?)', [product.id, product.name]);
}

// GOOD: single transaction (100 rows = 1 disk write)
db.run('BEGIN TRANSACTION');
const stmt = db.prepare('INSERT INTO products VALUES (?, ?)');
for (const product of products) {
  stmt.run([product.id, product.name]);
}
stmt.free();
db.run('COMMIT');
```

---

## 9. Verification Gates

Before marking ANY performance optimization as complete, verify ALL of these:

### Mandatory Checks

- [ ] **Bundle size delta**: Compare `npm run build` output before and after. Max 5% increase without written justification.
- [ ] **No new synchronous operations**: No `fs.readFileSync`, no synchronous IPC, no blocking main thread.
- [ ] **No memory leaks**: All useEffect hooks return cleanup. All listeners removed. All timers cleared.
- [ ] **Memoization verified**: If you added React.memo/useMemo/useCallback, verify with React DevTools Profiler that it actually reduces re-renders.
- [ ] **TypeScript passes**: `tsc --noEmit` with zero errors.
- [ ] **Both themes tested**: Check light AND dark mode — sometimes performance fixes break visual styling.
- [ ] **Improvement documented**: Before number, after number, percentage, what changed.

### Optional Checks (for major optimizations)

- [ ] Lighthouse Performance score maintained or improved
- [ ] First Contentful Paint not regressed
- [ ] No layout shifts introduced (CLS)
- [ ] Memory usage stable over 5 minutes of use

---

## 10. Anti-Premature-Completion

### Phrases That Are NOT Done

- "I optimized the component" -- WHERE IS THE MEASUREMENT?
- "I added React.memo" -- DID YOU VERIFY IT REDUCES RENDERS?
- "I split the bundle" -- WHAT IS THE NEW SIZE?
- "I fixed the memory leak" -- DID YOU TAKE BEFORE/AFTER HEAP SNAPSHOTS?
- "Performance should be better now" -- SHOULD? MEASURE IT.

### What "Done" Actually Looks Like

```
## Optimization Complete: Product List Rendering

### Before
- Render time: 47ms (React DevTools Profiler)
- Re-render count: 12/sec during search
- Bundle chunk: 180KB (vite build output)

### Changes
1. Wrapped ProductTable in React.memo with custom comparator
2. Extracted SearchInput into separate component to prevent cascade
3. Used useMemo for sorted product list (1200 items)

### After
- Render time: 8ms (83% improvement)
- Re-render count: 0 unnecessary re-renders
- Bundle chunk: 181KB (negligible increase from memo)

### Verified
- tsc --noEmit: PASS
- Light theme: PASS
- Dark theme: PASS
- React DevTools confirms 0 unnecessary re-renders
```

---

## 11. 3-Strike Rule

If 3 optimization attempts on the same bottleneck show no measurable improvement:

1. **STOP optimizing that area.**
2. The bottleneck is somewhere else. Profile AGAIN from scratch.
3. Common misdirections:
   - Optimizing React rendering when the bottleneck is in IPC
   - Optimizing bundle size when the bottleneck is in runtime performance
   - Optimizing query speed when the bottleneck is in rendering the results
   - Memoizing everything when the real issue is an O(n^2) algorithm

### Red Flags That You're Optimizing the Wrong Thing

- Adding `React.memo` to 5+ components in one session
- The profiler still shows the same render time after optimization
- Bundle size decreased but the app doesn't feel faster
- Memory usage didn't change after "fixing" a leak

---

## 12. Reference Links

### Internal References

- `references/react-rendering.md` — React DevTools Profiler workflow, re-render causes and fixes
- `references/bundle-optimization.md` — Tree-shaking, code splitting, lazy loading, chunk analysis
- `references/vite-config.md` — Build config, dev server, HMR, multi-process Electron builds
- `references/electron-perf.md` — Startup time, main process CPU, IPC optimization, ASAR
- `references/memory-leaks.md` — Detection tools, common causes, fix patterns, prevention

### External Resources

- React DevTools Profiler: https://react.dev/reference/react/Profiler
- Vite Build Optimization: https://vitejs.dev/guide/build
- Chrome DevTools Memory: https://developer.chrome.com/docs/devtools/memory-problems
- Electron Performance: https://www.electronjs.org/docs/latest/tutorial/performance
- web.dev Performance: https://web.dev/performance

### Related Skills

- `fireworks-debug` — For debugging correctness issues (not performance)
- `fireworks-review` — For code review with performance as one dimension
- `fireworks-architect` — For architectural decisions that affect performance
- `fireworks-test` — For writing performance regression tests
- `premium-design` — For UI quality alongside performance
- `electron-patterns` — For Electron-specific patterns and IPC

---

### DCI Pre-Loading for Performance Context
At skill activation, pre-load performance baselines:
- Current bundle size (`du -sh dist/`)
- Node.js version and memory limits
- Electron version and renderer process count
- Last known frame budget (8ms for 120Hz)
This eliminates discovery tool calls and jumps straight to profiling.

### Token-Efficient Performance Analysis
For large codebases, use RTK (Real-Time Knowledge) compression:
- Summarize performance findings as structured JSON, not prose
- Pass only metrics between agents, not full profiling output
- 60-90% token savings on multi-agent performance investigations

---

## Related Skills

- `fireworks-debug` — performance debugging decision tree
- `fireworks-design` — render optimization
- `fireworks-refactor` — performance refactoring

---

## Scope Boundaries

- **MINIMUM**: Always measure before optimizing (Golden Loop).
- **MAXIMUM**: Do not optimize code that runs < 10ms unless it's in a hot loop.
