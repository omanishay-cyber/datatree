# Bundle Optimization — Deep Reference

> Part of the `fireworks-performance` skill. See `../SKILL.md` for the master guide.

---

## Tree-Shaking

Tree-shaking removes unused exports from the final bundle. It only works with ES modules (`import`/`export`), NOT CommonJS (`require`/`module.exports`).

### How to Enable

1. **Use ES module versions of libraries**:
   ```ts
   // BAD: CommonJS — entire library included
   import _ from 'lodash';
   _.map(items, fn);

   // GOOD: ES module — only map is included
   import { map } from 'lodash-es';
   map(items, fn);
   ```

2. **Mark packages as side-effect-free** in `package.json`:
   ```json
   {
     "sideEffects": false
   }
   ```
   Or specify files with side effects:
   ```json
   {
     "sideEffects": ["./src/polyfills.ts", "*.css"]
   }
   ```

3. **Use named exports**, not default exports with destructuring:
   ```ts
   // GOOD: tree-shakable
   export function formatDate() { /* ... */ }
   export function formatCurrency() { /* ... */ }

   // LESS GOOD: harder to tree-shake
   export default { formatDate, formatCurrency };
   ```

### Verify Tree-Shaking is Working

```bash
# Build with rollup visualization
npx vite-bundle-visualizer

# Check if unused exports appear in the bundle
# If they do, the library may not support tree-shaking
```

---

## Code Splitting

Code splitting breaks the bundle into smaller chunks loaded on demand.

### Route-Level Splitting with React.lazy

```tsx
import { Suspense, lazy } from 'react';

// Each route becomes its own chunk
const Dashboard = lazy(() => import('./pages/Dashboard'));
const Reports = lazy(() => import('./pages/Reports'));
const Settings = lazy(() => import('./pages/Settings'));
const Inventory = lazy(() => import('./pages/Inventory'));

function App() {
  return (
    <Suspense fallback={<LoadingSpinner />}>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/reports" element={<Reports />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/inventory" element={<Inventory />} />
      </Routes>
    </Suspense>
  );
}
```

### Feature-Level Splitting with Dynamic Import

```tsx
// Heavy feature loaded only when needed
async function exportToExcel(data: Product[]) {
  const { utils, writeFile } = await import('xlsx');
  const worksheet = utils.json_to_sheet(data);
  const workbook = utils.book_new();
  utils.book_append_sheet(workbook, worksheet, 'Products');
  writeFile(workbook, 'products.xlsx');
}

// Chart library loaded only on the reports page
function ReportsPage() {
  const [ChartComponent, setChartComponent] = useState(null);

  useEffect(() => {
    import('recharts').then((mod) => {
      setChartComponent(() => mod.BarChart);
    });
  }, []);

  if (!ChartComponent) return <LoadingSpinner />;
  return <ChartComponent data={data} />;
}
```

### Preloading Critical Chunks

```tsx
// Preload chunks the user is likely to need next
function Sidebar() {
  const handleMouseEnter = () => {
    // Start loading the Reports chunk when user hovers the nav link
    import('./pages/Reports');
  };

  return (
    <nav>
      <Link to="/reports" onMouseEnter={handleMouseEnter}>Reports</Link>
    </nav>
  );
}
```

---

## Lazy Loading Below-Fold Content

### Intersection Observer Pattern

```tsx
function LazySection({ children }: { children: React.ReactNode }) {
  const [isVisible, setIsVisible] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: '200px' } // Start loading 200px before visible
    );

    if (ref.current) observer.observe(ref.current);
    return () => observer.disconnect();
  }, []);

  return <div ref={ref}>{isVisible ? children : <Placeholder />}</div>;
}
```

---

## Chunk Analysis Tools

### vite-bundle-visualizer

```bash
npx vite-bundle-visualizer
# Opens interactive treemap showing every module and its size
# Hover to see exact gzipped sizes
# Look for: large colored blocks in node_modules
```

### source-map-explorer

```bash
npx source-map-explorer dist/assets/index-*.js
# Alternative visualizer — good for comparing before/after
```

### Import Cost (VS Code Extension)

- Shows the gzipped size of each import inline in the editor.
- Immediately visible when importing a heavy dependency.
- Install: `wix.vscode-import-cost`

---

## Common Heavy Dependencies and Lighter Alternatives

| Heavy Dep | Size (gzip) | Alternative | Size (gzip) | Notes |
|-----------|-------------|-------------|-------------|-------|
| `moment` | ~72KB | `date-fns` | ~5KB (per function) | Tree-shakable, use only what you need |
| `moment` | ~72KB | `dayjs` | ~3KB | Drop-in replacement API |
| `lodash` | ~72KB | `lodash-es` | ~5KB (per function) | ES modules, tree-shakable |
| `lodash` | ~72KB | Native JS | 0KB | `Array.map`, `Object.keys`, `structuredClone` |
| `axios` | ~14KB | `fetch` API | 0KB | Built into Electron's Chromium |
| `uuid` | ~3KB | `crypto.randomUUID()` | 0KB | Built into modern JS |
| `classnames` | ~1KB | `clsx` | ~0.5KB | Faster and smaller |
| `numeral` | ~17KB | `Intl.NumberFormat` | 0KB | Built-in internationalization API |

### Decision Process

1. Check the size: `npx bundlephobia <package-name>` or check bundlephobia.com
2. Check if a native API exists (Intl, crypto, structuredClone, fetch)
3. Check if an ES module version exists (`-es` suffix, or check package.json for `"module"`)
4. Check if you use enough of the library to justify the size
5. If you use only 1-2 functions from a utility library, consider copying the implementation

---

## Bundle Size Monitoring

### In CI/CD

```bash
# Save baseline
npm run build 2>&1 | tee build-output.txt
# Compare in PR
# Flag if total bundle size increases by >5%
```

### Quick Manual Check

```bash
# Before changes
npm run build
du -sh dist/

# After changes
npm run build
du -sh dist/

# Compare and document the delta
```
