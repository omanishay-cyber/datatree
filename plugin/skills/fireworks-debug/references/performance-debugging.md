# Performance Debugging — Decision Tree

> Systematic approach to diagnosing and fixing performance issues.
> Start with Q1 to identify the type of slowness, then follow the branch.

---

## Q1: What Is Slow?

### Branch A: Startup Is Slow
Time from launch to usable UI is too long.

**Diagnostic Steps**:
1. Add timestamps to key startup events:
```typescript
console.time('app-ready');
app.on('ready', () => {
  console.timeEnd('app-ready');
  console.time('window-load');
});
// In renderer:
console.timeEnd('window-load');
console.time('first-render');
// After first meaningful render:
console.timeEnd('first-render');
```

2. Check what blocks the critical path:
   - Is Electron taking long to start? (app-ready > 2s)
   - Is the window slow to load? (window-load > 1s)
   - Is React slow to render? (first-render > 500ms)
   - Is data loading blocking the UI? (database init, file reads)

3. Common fixes:
   - Lazy-load non-critical modules: `const Module = lazy(() => import('./Module'))`
   - Defer database initialization to after first render
   - Use a splash screen while heavy operations complete
   - Pre-bundle dependencies with Vite optimizeDeps
   - Reduce preload script size

### Branch B: A Specific Action Is Slow
Clicking a button, opening a page, or saving data takes too long.

**Diagnostic Steps**:
1. Profile the action:
```typescript
console.time('action-name');
await performAction();
console.timeEnd('action-name');
```

2. Break down the action into phases:
   - Event handler: how long does the handler take?
   - IPC round-trip: how long does invoke/handle take?
   - Database query: how long does the query take?
   - Re-render: how long does React take to update the UI?

3. Common fixes:
   - Move heavy computation to a Web Worker or worker_threads
   - Add database indexes for slow queries
   - Batch multiple IPC calls into one
   - Use `useMemo` for expensive derived computations
   - Debounce rapid-fire actions (search-as-you-type)

### Branch C: App Degrades Over Time
App starts fast but gets slower the longer it runs.

**Diagnostic Steps**:
1. This is usually a memory leak. See `references/memory-leak-detection.md`.
2. Check for growing data structures (unbounded arrays, maps, caches)
3. Check for accumulating event listeners
4. Monitor memory over time with process.memoryUsage()

### Branch D: Rendering Is Slow
UI feels janky, animations stutter, scrolling is not smooth.

**Diagnostic Steps**:
1. Open DevTools > Performance tab
2. Click Record, perform the slow action, stop recording
3. Look for:
   - Long yellow bars (JavaScript execution > 16ms per frame)
   - Red triangles at the top (dropped frames)
   - Large purple blocks (layout thrashing)
   - Gray blocks (idle / waiting for IPC)

4. Common fixes:
   - Virtualize long lists: use `react-window` or `@tanstack/virtual`
   - Reduce re-renders (see React Profiler section below)
   - Avoid layout thrashing: batch DOM reads and writes
   - Use CSS `will-change` for animated elements
   - Use `requestAnimationFrame` for visual updates

---

## React Profiler Workflow

### Step 1: Enable Profiler
Open React DevTools > Profiler tab > Click Record.

### Step 2: Perform the Action
Do the action that causes unnecessary re-renders.

### Step 3: Analyze Results
- Each colored bar is a commit (React update)
- Click a commit to see which components rendered
- Gray components did NOT render (good)
- Colored components DID render (check if necessary)

### Step 4: Find Unnecessary Re-Renders
For each component that rendered, check WHY:
- **Props changed**: Which prop changed? Was the change necessary?
- **State changed**: Was the state update necessary?
- **Parent re-rendered**: Did the parent force a re-render via new object/array props?

### Step 5: Fix Unnecessary Re-Renders
```typescript
// Problem: Parent creates new object every render
function Parent() {
  const style = { color: 'red' }; // New object every render!
  return <Child style={style} />;
}

// Fix A: useMemo
function Parent() {
  const style = useMemo(() => ({ color: 'red' }), []);
  return <Child style={style} />;
}

// Fix B: Move constant outside component
const style = { color: 'red' };
function Parent() {
  return <Child style={style} />;
}

// Fix C: React.memo on child
const Child = React.memo(function Child({ style }) {
  return <div style={style}>Content</div>;
});
```

---

## Zustand-Specific Performance

### Problem: Component Re-renders on Every Store Change
```typescript
// BAD: Subscribes to entire store — re-renders on ANY change
const store = useProductStore();

// GOOD: Subscribe to specific slice with selector
const products = useProductStore((state) => state.products);
const addProduct = useProductStore((state) => state.addProduct);
```

### Problem: Selector Returns New Object Every Time
```typescript
// BAD: Creates new object on every call
const { products, total } = useProductStore((state) => ({
  products: state.products,
  total: state.products.length,
}));

// GOOD: Use shallow comparison
import { useShallow } from 'zustand/react/shallow';
const { products, total } = useProductStore(
  useShallow((state) => ({
    products: state.products,
    total: state.products.length,
  }))
);
```

### Problem: Derived Data Recalculated Every Render
```typescript
// BAD: Filters on every render
function ProductList() {
  const products = useProductStore((s) => s.products);
  const filtered = products.filter(p => p.category === 'wine'); // Runs every render!
}

// GOOD: Memoize derived data
function ProductList() {
  const products = useProductStore((s) => s.products);
  const filtered = useMemo(
    () => products.filter(p => p.category === 'wine'),
    [products]
  );
}
```

### Store Splitting for Performance
If a store has unrelated sections, split it:
```typescript
// Instead of one mega-store:
const useAppStore = create(() => ({
  products: [], users: [], settings: {}, cart: [],
}));

// Split into focused stores:
const useProductStore = create(() => ({ products: [] }));
const useUserStore = create(() => ({ users: [] }));
const useSettingsStore = create(() => ({ settings: {} }));
const useCartStore = create(() => ({ cart: [] }));
```

---

## Performance Metrics to Capture

| Metric | Target | How to Measure |
|--------|--------|---------------|
| App startup | < 2s to usable UI | console.time from launch to first render |
| Page navigation | < 200ms | console.time from click to render complete |
| IPC round-trip | < 50ms for simple queries | console.time around invoke/handle |
| Database query | < 100ms for most queries | console.time around SQL execution |
| Re-render count | Minimal per action | React Profiler commit count |
| Frame rate | 60fps during animations | DevTools Performance tab |
| Memory | Stable over time | process.memoryUsage() at intervals |
| Bundle size | < 5MB for renderer | `npx vite-bundle-visualizer` |
