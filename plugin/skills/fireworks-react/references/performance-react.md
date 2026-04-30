# React Performance -- Profiling, Optimization & Streaming

> React DevTools profiling workflow, wasted render identification, Suspense for data fetching, streaming SSR, bundle optimization, and Core Web Vitals for the maintainer's premium apps.

---

## 1. React DevTools Profiler -- Complete Workflow

### Setup

1. Install React DevTools browser extension (or use standalone for Electron)
2. Ensure app runs in development mode (`import.meta.env.DEV === true`)
3. Open DevTools -> Profiler tab

### Profiling Session

```
Step 1: Click "Record" (blue circle)
Step 2: Perform the exact user action that feels slow
Step 3: Click "Stop" (red circle)
Step 4: Analyze the flamegraph
```

### Reading the Flamegraph

```
Color coding:
  - Gray:     Did not render (good!)
  - Blue/Teal: Rendered, but fast (< 5ms)
  - Yellow:   Rendered, somewhat slow (5-16ms)
  - Red:      Rendered, very slow (> 16ms) -- investigate this!

Width = relative render time
Depth = component tree depth
```

### "Why Did This Render?" Checklist

Enable "Record why each component rendered" in Profiler settings. Common causes:

| Reason | What Happened | Fix |
|--------|--------------|-----|
| "Props changed" | Parent passed new object/array/function ref | useMemo/useCallback in parent |
| "State changed" | Component's own state updated | Check if state update was necessary |
| "Context changed" | A context provider re-rendered | Split context, memoize provider value |
| "Parent rendered" | Parent re-rendered, child didn't memo | React.memo if child is expensive |
| "Hooks changed" | A hook returned a new value | Check hook dependencies |

### Programmatic Profiler API

```tsx
import { Profiler, ProfilerOnRenderCallback } from 'react';

const onRender: ProfilerOnRenderCallback = (
  id,           // "InventoryTable"
  phase,        // "mount" | "update"
  actualDuration, // time spent rendering
  baseDuration,   // estimated time without memoization
  startTime,
  commitTime,
) => {
  if (actualDuration > 16) {
    console.warn(`[Perf] ${id} ${phase}: ${actualDuration.toFixed(1)}ms (base: ${baseDuration.toFixed(1)}ms)`);
  }
};

// Wrap any component tree
<Profiler id="InventoryTable" onRender={onRender}>
  <InventoryTable />
</Profiler>
```

### Production Profiling

```tsx
// Vite config for production profiling build
export default defineConfig({
  build: {
    rollupOptions: {
      // Enable React profiling in production
      alias: {
        'react-dom': 'react-dom/profiling',
        'scheduler/tracing': 'scheduler/tracing-profiling',
      },
    },
  },
});
```

---

## 2. Identifying Wasted Renders

### Render Audit Technique

```tsx
// Temporary: add to any component to log renders
function useRenderCount(name: string) {
  const count = useRef(0);
  count.current += 1;
  useEffect(() => {
    console.log(`[Render] ${name}: #${count.current}`);
  });
}

// Usage (remove after debugging)
function ProductRow({ product }: { product: Product }) {
  useRenderCount(`ProductRow-${product.id}`);
  // ...
}
```

### Common Wasted Render Sources

#### Source 1: Inline Object/Array Literals

```tsx
// PROBLEM: new object every render
<Chart options={{ responsive: true, theme: 'dark' }} />

// FIX: extract to constant or useMemo
const CHART_OPTIONS = { responsive: true, theme: 'dark' } as const;
<Chart options={CHART_OPTIONS} />

// Or if it depends on props:
const chartOptions = useMemo(() => ({ responsive: true, theme }), [theme]);
<Chart options={chartOptions} />
```

#### Source 2: Inline Callbacks on Memo'd Children

```tsx
// PROBLEM: new function every render breaks React.memo
<MemoizedList onItemClick={(id) => selectItem(id)} />

// FIX: useCallback
const handleItemClick = useCallback((id: string) => selectItem(id), [selectItem]);
<MemoizedList onItemClick={handleItemClick} />
```

#### Source 3: Context Provider Value

```tsx
// PROBLEM: provider value is new object every render
function AppProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  return (
    <AppContext.Provider value={{ user, setUser }}> {/* NEW object every render */}
      {children}
    </AppContext.Provider>
  );
}

// FIX: memoize the value
function AppProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const value = useMemo(() => ({ user, setUser }), [user]);
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

// BETTER FIX: split into value + dispatch contexts
const UserContext = createContext<User | null>(null);
const UserDispatchContext = createContext<React.Dispatch<React.SetStateAction<User | null>>>(() => {});
```

#### Source 4: Zustand Whole-Store Subscription

```tsx
// PROBLEM: re-renders on ANY store change
const store = useInventoryStore();

// FIX: granular selectors
const items = useInventoryStore((s) => s.items);
const addItem = useInventoryStore((s) => s.addItem);
```

---

## 3. Memoization Strategy

### The Decision Flow

```
1. Identify the slow component (Profiler)
2. Check WHY it re-renders (DevTools "why" reason)
3. Is it rendering too often? --> React.memo the component
4. Is a prop unstable? --> useMemo (value) or useCallback (function) in the PARENT
5. Is it internally slow? --> useMemo on the expensive computation
6. None of the above? --> Consider virtualization or code splitting
```

### React.memo -- Correct Usage

```tsx
// ONLY memo when the component is expensive AND parent re-renders often
export const ProductCard = React.memo(function ProductCard({ product, onSelect }: ProductCardProps) {
  return (
    <motion.div
      whileHover={{ scale: 1.02 }}
      className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-xl p-4"
    >
      <h3>{product.name}</h3>
      <p>${product.price.toFixed(2)}</p>
      <button onClick={() => onSelect(product.id)}>Select</button>
    </motion.div>
  );
});

// Custom comparator for complex props
export const DataGrid = React.memo(
  function DataGrid({ data, columns }: DataGridProps) { /* ... */ },
  (prev, next) => {
    return prev.data.length === next.data.length
      && prev.data === next.data
      && prev.columns === next.columns;
  }
);
```

---

## 4. Suspense for Data Fetching

### React 19 Pattern with use()

```tsx
// Parent creates the promise
function ProductPage({ id }: { id: string }) {
  const productPromise = useMemo(() => fetchProduct(id), [id]);
  return (
    <Suspense fallback={<ProductSkeleton />}>
      <ProductDetails productPromise={productPromise} />
    </Suspense>
  );
}

// Child reads the promise with use()
function ProductDetails({ productPromise }: { productPromise: Promise<Product> }) {
  const product = use(productPromise);
  return (
    <div>
      <h1>{product.name}</h1>
      <p>${product.price}</p>
    </div>
  );
}
```

### Suspense Boundaries Strategy

```tsx
// Granular Suspense boundaries for independent sections
function Dashboard() {
  return (
    <div className="grid grid-cols-2 gap-6">
      <Suspense fallback={<ChartSkeleton />}>
        <SalesChart />          {/* Can load independently */}
      </Suspense>

      <Suspense fallback={<StatsSkeleton />}>
        <QuickStats />          {/* Can load independently */}
      </Suspense>

      <Suspense fallback={<TableSkeleton />}>
        <RecentOrders />        {/* Can load independently */}
      </Suspense>

      <Suspense fallback={<AlertsSkeleton />}>
        <LowStockAlerts />      {/* Can load independently */}
      </Suspense>
    </div>
  );
}
```

### Skeleton Components (Premium Loading States)

```tsx
function ProductSkeleton() {
  return (
    <div className="animate-pulse backdrop-blur-xl bg-white/5 dark:bg-black/10 border border-white/10 rounded-xl p-6">
      <div className="h-6 w-2/3 bg-white/10 rounded mb-4" />
      <div className="h-4 w-1/3 bg-white/10 rounded mb-2" />
      <div className="h-4 w-full bg-white/10 rounded mb-2" />
      <div className="h-4 w-4/5 bg-white/10 rounded" />
    </div>
  );
}
```

---

## 5. Code Splitting & Lazy Loading

### Route-Level Splitting (Always Do This)

```tsx
const Inventory = lazy(() => import('./pages/Inventory'));
const Reports = lazy(() => import('./pages/Reports'));
const Settings = lazy(() => import('./pages/Settings'));
const Analytics = lazy(() => import('./pages/Analytics'));

function AppRoutes() {
  return (
    <Suspense fallback={<PageSkeleton />}>
      <Routes>
        <Route path="/inventory" element={<Inventory />} />
        <Route path="/reports" element={<Reports />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/analytics" element={<Analytics />} />
      </Routes>
    </Suspense>
  );
}
```

### Component-Level Splitting (For Heavy Components)

```tsx
// Only load the rich text editor when needed
const RichTextEditor = lazy(() => import('./components/RichTextEditor'));
const PdfViewer = lazy(() => import('./components/PdfViewer'));
const ChartBuilder = lazy(() => import('./components/ChartBuilder'));

function NoteEditor({ mode }: { mode: 'text' | 'pdf' | 'chart' }) {
  return (
    <Suspense fallback={<EditorSkeleton />}>
      {mode === 'text' && <RichTextEditor />}
      {mode === 'pdf' && <PdfViewer />}
      {mode === 'chart' && <ChartBuilder />}
    </Suspense>
  );
}
```

### Preloading on Hover (Premium UX)

```tsx
// Preload the chunk when user hovers the nav link
const loaders = {
  inventory: () => import('./pages/Inventory'),
  reports: () => import('./pages/Reports'),
  settings: () => import('./pages/Settings'),
};

function NavItem({ to, label, loader }: { to: string; label: string; loader: () => Promise<unknown> }) {
  return (
    <Link
      to={to}
      onMouseEnter={loader}
      onFocus={loader}
      className="transition-all duration-200 hover:bg-white/10 px-4 py-2 rounded-lg"
    >
      {label}
    </Link>
  );
}
```

---

## 6. Virtualization (Large Lists)

### TanStack Virtual

```tsx
import { useVirtualizer } from '@tanstack/react-virtual';

function VirtualProductList({ products }: { products: Product[] }) {
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: products.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 72,
    overscan: 10,
  });

  return (
    <div
      ref={parentRef}
      className="h-[calc(100vh-200px)] overflow-auto rounded-xl backdrop-blur-xl bg-white/5 dark:bg-black/10 border border-white/10"
    >
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const product = products[virtualRow.index];
          return (
            <div
              key={product.id}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: virtualRow.size,
                transform: `translateY(${virtualRow.start}px)`,
              }}
              className="border-b border-white/5 px-4 flex items-center"
            >
              <ProductRow product={product} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

### Virtual Grid

```tsx
import { useVirtualizer } from '@tanstack/react-virtual';

function VirtualGrid({ items, columnCount = 3 }: { items: Product[]; columnCount?: number }) {
  const parentRef = useRef<HTMLDivElement>(null);
  const rowCount = Math.ceil(items.length / columnCount);

  const rowVirtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 200,
    overscan: 3,
  });

  return (
    <div ref={parentRef} className="h-[calc(100vh-200px)] overflow-auto">
      <div style={{ height: rowVirtualizer.getTotalSize(), position: 'relative' }}>
        {rowVirtualizer.getVirtualItems().map((virtualRow) => (
          <div
            key={virtualRow.key}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              height: virtualRow.size,
              transform: `translateY(${virtualRow.start}px)`,
            }}
            className="grid gap-4"
            style-gridTemplateColumns={`repeat(${columnCount}, 1fr)`}
          >
            {Array.from({ length: columnCount }).map((_, colIndex) => {
              const itemIndex = virtualRow.index * columnCount + colIndex;
              const item = items[itemIndex];
              if (!item) return <div key={colIndex} />;
              return <ProductCard key={item.id} product={item} />;
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
```

---

## 7. Streaming SSR (React 19)

### Server-Side Rendering with Suspense Streaming

```tsx
// server.ts
import { renderToPipeableStream } from 'react-dom/server';

app.get('*', (req, res) => {
  const { pipe, abort } = renderToPipeableStream(<App url={req.url} />, {
    bootstrapScripts: ['/client.js'],
    onShellReady() {
      // Shell (everything outside Suspense) is ready
      res.statusCode = 200;
      res.setHeader('Content-Type', 'text/html');
      pipe(res);
    },
    onShellError(error) {
      res.statusCode = 500;
      res.send('<h1>Server Error</h1>');
    },
    onError(error) {
      console.error('Streaming error:', error);
    },
  });

  // Abort after timeout
  setTimeout(() => abort(), 10000);
});
```

### Progressive Hydration

```tsx
// Hydrate the shell immediately, Suspense boundaries hydrate as data arrives
import { hydrateRoot } from 'react-dom/client';

hydrateRoot(document.getElementById('root')!, <App />);

// Components inside Suspense boundaries will:
// 1. Show fallback immediately (from server HTML)
// 2. Stream actual content as server resolves each Suspense boundary
// 3. Hydrate interactivity once JavaScript loads
```

---

## 8. Bundle Optimization

### Vite-Specific Optimizations

```typescript
// vite.config.ts
export default defineConfig({
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom'],
          'vendor-motion': ['framer-motion'],
          'vendor-zustand': ['zustand'],
          'vendor-charts': ['recharts'],
        },
      },
    },
    chunkSizeWarningLimit: 500,
    sourcemap: true,
  },
  optimizeDeps: {
    include: ['react', 'react-dom', 'zustand', 'framer-motion'],
  },
});
```

### Tree-Shaking Tips

```tsx
// WRONG: imports entire library
import _ from 'lodash';
const sorted = _.sortBy(items, 'name');

// RIGHT: import only what you need
import sortBy from 'lodash/sortBy';
const sorted = sortBy(items, 'name');

// BEST: use native methods when possible
const sorted = [...items].sort((a, b) => a.name.localeCompare(b.name));
```

### Dynamic Import for Conditional Features

```tsx
// Only load PDF export when user clicks "Export"
async function handleExportPdf() {
  const { generatePdf } = await import('./utils/pdf-export');
  await generatePdf(data);
}

// Only load chart library when chart tab is visible
function AnalyticsTabs() {
  const [activeTab, setActiveTab] = useState('table');
  return (
    <>
      <TabBar active={activeTab} onChange={setActiveTab} />
      {activeTab === 'table' && <DataTable data={data} />}
      {activeTab === 'chart' && (
        <Suspense fallback={<ChartSkeleton />}>
          <LazyChartView data={data} />
        </Suspense>
      )}
    </>
  );
}
```

---

## 9. Core Web Vitals for React Apps

### LCP (Largest Contentful Paint)

```
Target: < 2.5 seconds

React-specific fixes:
- Code-split routes -- don't load all pages upfront
- Preload critical data (fetch in parallel with JS)
- Use Suspense with streaming for above-fold content
- Avoid render-blocking useEffect chains
```

### FID / INP (Interaction to Next Paint)

```
Target: < 200ms (INP)

React-specific fixes:
- Use useTransition for expensive state updates
- Use useDeferredValue for non-urgent renders
- Virtualize long lists (don't render 1000 rows)
- Break up large component trees with Suspense boundaries
- Avoid synchronous heavy computation in event handlers
```

### CLS (Cumulative Layout Shift)

```
Target: < 0.1

React-specific fixes:
- Set explicit dimensions on images/containers
- Use skeleton loaders that match final layout
- Avoid inserting content above existing content
- Use CSS containment on dynamic sections
- Reserve space for lazy-loaded components
```

---

## 10. Performance Debugging Checklist

When an interaction feels slow:

1. **Profile with React DevTools Profiler** -- identify the slow component
2. **Check render count** -- is it rendering more than expected?
3. **Check render duration** -- is each render slow, or just too many renders?
4. **If too many renders:**
   - [ ] Props creating new references? (useMemo/useCallback in parent)
   - [ ] Zustand subscribing to whole store? (use selectors)
   - [ ] Context value recreated? (memoize or split context)
   - [ ] Missing React.memo on expensive child?
5. **If each render is slow:**
   - [ ] Expensive computation? (useMemo)
   - [ ] Rendering too many DOM nodes? (virtualize)
   - [ ] Large component? (code-split with lazy)
   - [ ] Synchronous heavy work? (useTransition/Web Worker)
6. **If bundle is too large:**
   - [ ] Route-level code splitting?
   - [ ] Tree-shaking working? (check import style)
   - [ ] Heavy libraries loaded conditionally?
   - [ ] Analyze with `npx vite-bundle-visualizer`

### Performance Budget (the user Standard)

| Metric | Budget | Measurement |
|--------|--------|-------------|
| Initial bundle | < 200KB gzipped | `vite build --report` |
| Route chunk | < 50KB gzipped | Bundle analyzer |
| Component render | < 16ms | React Profiler |
| State update -> paint | < 100ms | Chrome DevTools Performance |
| List with 1000 items | < 50ms render | Virtualization required |
| Page transition | < 300ms perceived | Framer Motion + Suspense |
