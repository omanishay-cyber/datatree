# Performance Review Reference — Fireworks Review

Detailed checklist for the **Performance** lens. Use this reference when reviewing code for efficiency, resource usage, and responsiveness.

---

## React Re-renders

### Unnecessary Renders from Parent
- When a parent component re-renders, ALL children re-render by default
- Children that receive no changed props still re-render without `React.memo`
- Wrap expensive/heavy child components in `React.memo()` to skip unnecessary renders
- `React.memo` uses shallow comparison by default — deep objects always look "new"

### Missing React.memo on Heavy Components
- Components that render large lists, charts, or complex DOM trees
- Components that perform expensive calculations during render
- Components that are children of frequently-updating parents (e.g., timer, real-time data)
- Do NOT memo everything — only memo components where re-render cost is measurable
- Premature memo-ization adds complexity without benefit for lightweight components

### Unstable References in Props
```typescript
// BAD: New object/function created every render — defeats React.memo
<DataTable
  filters={{ status: 'active', sort: 'name' }}  // New object every render
  onRowClick={(row) => handleClick(row)}          // New function every render
  style={{ padding: 16 }}                         // New object every render
/>

// GOOD: Stable references
const filters = useMemo(() => ({ status: 'active', sort: 'name' }), []);
const handleRowClick = useCallback((row) => handleClick(row), [handleClick]);
const tableStyle = useMemo(() => ({ padding: 16 }), []);
<DataTable filters={filters} onRowClick={handleRowClick} style={tableStyle} />
```

### Missing useMemo / useCallback
- **useMemo**: Memoize expensive computed values (sorting, filtering, transforming large arrays)
- **useCallback**: Memoize functions passed as props to memoized children
- **When NOT needed**: Simple computations, values not passed as props, non-memoized children
- **Dependency arrays**: Must include ALL values used inside. Missing deps = stale data bugs.

### Zustand-Specific Performance
- Use selectors to subscribe to specific state slices: `useStore(state => state.products)`
- Avoid subscribing to entire store: `useStore()` — re-renders on ANY state change
- Use `shallow` equality for object selectors: `useStore(state => ({ a: state.a, b: state.b }), shallow)`
- Split large stores into domain-specific stores to reduce subscription scope

### Key Prop Anti-Patterns
- Using array index as key when list items can be reordered, added, or removed
- Using `Math.random()` as key — forces full unmount/remount every render
- Missing key prop on list items — React falls back to index (with warnings)
- Correct key: a stable, unique identifier from the data (e.g., `product.id`)

---

## Bundle Size

### Unused Imports
- Importing an entire module when only one function is needed
  ```typescript
  // BAD: Imports entire lodash (70KB+)
  import _ from 'lodash';
  _.debounce(fn, 300);

  // GOOD: Import specific function (3KB)
  import debounce from 'lodash/debounce';
  // OR use lodash-es for tree-shaking
  import { debounce } from 'lodash-es';
  ```
- Importing components that are only used in rare code paths — use dynamic imports

### Large Libraries Imported Entirely
- **moment.js**: 300KB+ — replace with `date-fns` (tree-shakeable) or `dayjs` (2KB)
- **lodash**: 70KB+ — use `lodash-es` or individual function imports
- **chart libraries**: Import only the chart types you use, not the entire library
- **icon libraries**: Import individual icons, not the full set
  ```typescript
  // BAD: Imports all icons
  import * as Icons from 'lucide-react';

  // GOOD: Import specific icons
  import { Search, Filter, Download } from 'lucide-react';
  ```

### Dynamic Imports for Heavy Components
```typescript
// Lazy-load heavy components
const Chart = lazy(() => import('./components/Chart'));
const PDFExport = lazy(() => import('./components/PDFExport'));
const ReportBuilder = lazy(() => import('./features/ReportBuilder'));

// Use with Suspense
<Suspense fallback={<LoadingSkeleton />}>
  <Chart data={data} />
</Suspense>
```
- Use dynamic imports for: modals, settings pages, export features, charts, rich editors
- Do NOT lazy-load: navigation components, layouts, frequently-accessed pages

### Tree-Shaking Verification
- Ensure `"sideEffects": false` in `package.json` for tree-shaking to work
- Use named exports (not default exports) for better tree-shaking
- Avoid re-exporting everything from barrel files (`index.ts` that exports `*`)
- Check bundle analysis: `npx vite-bundle-visualizer` or `webpack-bundle-analyzer`

---

## Memory Leaks

### Event Listeners Without Cleanup
```typescript
// BAD: Listener leaks on unmount
useEffect(() => {
  window.addEventListener('resize', handleResize);
}, []); // No cleanup!

// GOOD: Remove listener on unmount
useEffect(() => {
  window.addEventListener('resize', handleResize);
  return () => window.removeEventListener('resize', handleResize);
}, [handleResize]);
```
- Every `addEventListener` must have a corresponding `removeEventListener` in cleanup
- Every `ipcRenderer.on` must have a corresponding `ipcRenderer.removeListener`
- Electron: `BrowserWindow` event listeners leak if window is destroyed without removing them

### Timers Without Cleanup
```typescript
// BAD: Timer continues after unmount
useEffect(() => {
  setInterval(() => pollData(), 5000);
}, []); // Interval never cleared!

// GOOD: Clear on unmount
useEffect(() => {
  const id = setInterval(() => pollData(), 5000);
  return () => clearInterval(id);
}, []);
```
- `setInterval` without `clearInterval` — timer fires forever
- `setTimeout` — usually fine, but cancel if component unmounts before it fires
- `requestAnimationFrame` without `cancelAnimationFrame`
- Debounced/throttled functions — cancel pending invocations on unmount

### Subscriptions Without Unsubscribe
- Zustand `subscribe()` returns an unsubscribe function — must call it on cleanup
- RxJS observables — must unsubscribe
- EventEmitter listeners — must remove on cleanup
- WebSocket connections — must close on component unmount
- Database watchers / file watchers — must close handles

### DOM References in Closures
- Closures capturing DOM elements prevent garbage collection
- Event handlers holding references to removed DOM nodes
- Refs (`useRef`) pointing to components that no longer exist
- WeakRef/WeakMap for caches that should not prevent GC

### Growing State Without Limits
```typescript
// BAD: Array grows forever
const [logs, setLogs] = useState<LogEntry[]>([]);
useEffect(() => {
  onLog((entry) => setLogs(prev => [...prev, entry]));
}, []);

// GOOD: Cap the array size
const MAX_LOGS = 1000;
useEffect(() => {
  onLog((entry) => setLogs(prev => [...prev.slice(-MAX_LOGS + 1), entry]));
}, []);
```
- In-memory caches without eviction policies (LRU, TTL, max size)
- Undo/redo history that grows unbounded
- Log buffers that accumulate indefinitely
- Accumulated event data from long-running sessions

---

## Synchronous Operations

### readFileSync / writeFileSync in Renderer
- File I/O in the renderer process blocks the UI thread
- Even in the main process, sync file ops block the event loop (freezes IPC)
- Use async versions: `fs.promises.readFile()`, `fs.promises.writeFile()`
- For Electron: do all file I/O in the main process via IPC, using async handlers

### execSync in Renderer
- Shell command execution blocks everything until complete
- Use `execFile` with callbacks or `child_process.spawn` for streaming output
- Long-running processes should use `spawn` with event listeners, not `exec`

### Synchronous IPC (sendSync)
- `ipcRenderer.sendSync()` blocks the renderer until main process responds
- If main process is busy, renderer freezes completely
- **Always use** `ipcRenderer.invoke()` which returns a Promise
- `sendSync` should NEVER appear in modern Electron apps

### Blocking the Main Process Event Loop
- Heavy computation in IPC handlers blocks ALL windows
- Move CPU-intensive work to a worker thread or child process
- Database operations: use async queries, not synchronous versions
- JSON parsing of large files: use streaming parsers for files > 10MB
- Crypto operations: use async versions (`crypto.subtle`, worker threads)

---

## Database Performance

### Missing Indexes
- Every column used in `WHERE`, `ORDER BY`, or `JOIN` should have an index
- Composite indexes for queries that filter on multiple columns
- Check query plan with `EXPLAIN QUERY PLAN` in SQLite
- Too many indexes slow down writes — index only what queries need

### N+1 Query Patterns
```typescript
// BAD: N+1 queries — 1 query for products, N queries for categories
const products = db.prepare('SELECT * FROM products').all();
for (const product of products) {
  product.category = db.prepare('SELECT * FROM categories WHERE id = ?').get(product.category_id);
}

// GOOD: Single query with JOIN
const products = db.prepare(`
  SELECT p.*, c.name as category_name
  FROM products p
  LEFT JOIN categories c ON p.category_id = c.id
`).all();
```
- Look for database queries inside loops
- Use JOINs to fetch related data in one query
- For complex relationships, use subqueries or CTEs

### Unbounded SELECT *
- `SELECT * FROM table` with no `LIMIT` on a table with 100k+ rows = OOM risk
- Always paginate: `LIMIT ? OFFSET ?` or cursor-based pagination
- Select only needed columns: `SELECT id, name, price FROM products`
- Use `COUNT(*)` for totals, not fetching all rows and checking `.length`

### Missing Pagination
- Any list UI that fetches all records at once
- Virtualized lists still need data pagination (don't load 50k rows into memory)
- Implement cursor-based pagination for real-time data (offset pagination breaks with inserts)
- Default page sizes: 25-100 for UI lists, 1000 for batch operations

### Transaction Batching
```typescript
// BAD: 1000 individual inserts = 1000 transactions = slow
for (const item of items) {
  db.prepare('INSERT INTO items VALUES (?, ?)').run(item.id, item.name);
}

// GOOD: Single transaction = fast
const insert = db.prepare('INSERT INTO items VALUES (?, ?)');
const insertMany = db.transaction((items) => {
  for (const item of items) insert.run(item.id, item.name);
});
insertMany(items);
```
- Batch writes into transactions — 100x faster in SQLite
- Read-only operations do not need explicit transactions
- Keep transaction scope small — don't hold transactions open during user interaction
