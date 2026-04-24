---
name: fireworks-performance
description: Performance optimization superbrain — React rendering, bundle analysis, memory leaks, Vite config, Electron startup
version: 2.0.0
author: mneme
tags: [performance, optimization, rendering, bundle, memory, startup, query, profiling]
triggers: [performance, slow, memory leak, bundle size, render, optimize, startup, query, profiling, lag, jank]
---

# Fireworks Performance — Master Skill

> "Measure first. Optimize second. Verify third. Document always."

This skill turns Claude into a performance optimization expert for Electron + React + Vite + TypeScript applications. It covers the full stack: React rendering, bundle size, memory leaks, Vite configuration, Electron startup, IPC throughput, and sql.js query performance.

---

## 1. Performance Investigation Protocol

Every performance task MUST follow this workflow. No exceptions.

### The Golden Loop

```
MEASURE -> IDENTIFY -> OPTIMIZE -> MEASURE AGAIN -> DOCUMENT
```

### Step-by-Step

1. **Measure First** — Establish a baseline BEFORE touching any code.
   - What metric? (render time, bundle size, memory usage, startup time, query duration)
   - What tool? (React DevTools Profiler, Vite build output, Chrome DevTools Memory, Performance tab)
   - Write down the baseline number.

2. **Identify the Bottleneck** — Do NOT guess. Use profiling data.
   - React: Which component renders most often or takes longest?
   - Bundle: Which dependency is the largest?
   - Memory: What is growing without being collected?
   - Startup: What blocks the critical path?

3. **Optimize** — Apply the minimum change that addresses the identified bottleneck.
   - One change at a time. Never stack multiple optimizations before measuring.
   - Prefer algorithmic improvements over memoization hacks.
   - Prefer removing code over adding caching layers.

4. **Measure Again** — Same metric, same tool, same conditions.
   - Did the number improve? By how much?
   - If no improvement: revert and try a different approach.
   - If regression: revert immediately.

5. **Document the Improvement** — Every optimization MUST have:
   - What was the problem?
   - What was the baseline measurement?
   - What change was made?
   - What is the new measurement?
   - Percentage improvement.

### Example Documentation

```
## Optimization: Product list rendering
- Problem: ProductTable re-renders on every keystroke in search bar
- Baseline: 47ms render time for 500 products, 12 unnecessary re-renders per second
- Fix: Extracted search input into separate component, memoized ProductTable with React.memo
- Result: 8ms render time, 0 unnecessary re-renders
- Improvement: 83% reduction in render time
```

---

## 2. React Rendering Quick-Reference

### React.memo

**USE when:**
- Component receives complex props (objects, arrays) and re-renders often
- Component is expensive to render (large lists, charts, complex DOM)
- Parent re-renders frequently but child props rarely change

**DO NOT use when:**
- Props change on every render (defeats the purpose)
- Component is cheap to render (the memo comparison costs more than re-rendering)
- Component receives children as props (children create new references every render)
- On every component "just in case" (adds overhead without benefit)

```tsx
// GOOD: Expensive component with stable props
const ProductTable = React.memo(({ products, sortBy }: Props) => {
  // Renders 500+ rows with complex formatting
  return <table>...</table>;
});

// BAD: Trivial component
const Label = React.memo(({ text }: { text: string }) => {
  return <span>{text}</span>; // So cheap that memo comparison costs more
});
```

### useMemo

**USE when:**
- Expensive computations: sorting 1000+ items, complex math, data transformations
- Creating derived data that would be expensive to recompute
- Stabilizing object/array references passed to React.memo children

**DO NOT use when:**
- Simple derived values: `const fullName = first + ' ' + last`
- The computation is trivial (fewer than ~100 items)
- The dependency array changes every render

```tsx
// GOOD: Sorting a large dataset
const sortedProducts = useMemo(
  () => products.sort((a, b) => a.price - b.price),
  [products]
);

// BAD: Simple concatenation
const fullName = useMemo(() => `${first} ${last}`, [first, last]);
// Just do: const fullName = `${first} ${last}`;
```

### useCallback

**USE ONLY when:**
- Passing a callback to a React.memo child component
- Passing a callback to a dependency array of useEffect/useMemo
- Creating a stable reference for event handlers used in third-party libraries

**DO NOT use when:**
- The handler is used only in the same component
- The child component is not wrapped in React.memo
- "Just in case" on every handler (adds complexity, rarely helps)

```tsx
// GOOD: Callback passed to memoized child
const handleSort = useCallback((column: string) => {
  dispatch({ type: 'SORT', column });
}, [dispatch]);

return <MemoizedTable onSort={handleSort} />;

// BAD: Handler used in same component
const handleClick = useCallback(() => {
  setCount(c => c + 1);
}, []); // Unnecessary — no memoized child uses this
```

### Decision Tree

```
"Is the UI slow?"
  -> No: STOP. Do not optimize.
  -> Yes: Profile with React DevTools
    -> "Which component is slow?"
      -> Check render count in Profiler
        -> "Too many renders?"
          -> Check WHY: unstable refs? parent re-renders? context?
          -> Fix ROOT CAUSE, not symptoms
        -> "Each render is slow?"
          -> Check WHAT: expensive computation? large DOM? layout thrashing?
          -> Optimize the expensive operation
```

> See `references/react-rendering.md` for detailed profiling workflow.

---

## 3. Bundle Size Analysis

### Investigation Workflow

```bash
# 1. Build and check output
npm run build
# Note the total dist/ size

# 2. Visualize the bundle
npx vite-bundle-visualizer
# Opens interactive treemap in browser

# 3. Identify large chunks
# Look for: node_modules taking >50% of bundle
# Look for: single dependency >100KB gzipped
# Look for: duplicate dependencies

# 4. Apply fixes (in order of impact)
# a. Tree-shaking: switch to ES module versions
# b. Code splitting: lazy-load routes and heavy features
# c. Replace heavy deps with lighter alternatives
# d. Remove unused dependencies
```

### Size Budgets

| Category | Budget | Action if exceeded |
|----------|--------|--------------------|
| Total bundle (gzipped) | <500KB | Mandatory optimization |
| Single vendor chunk | <200KB | Split or replace |
| Route chunk | <50KB | Lazy-load sub-components |
| CSS | <50KB | Purge unused styles |
| Single dependency | <100KB | Evaluate alternatives |

### Quick Wins

1. **Import only what you use**: `import { map } from 'lodash-es'` not `import _ from 'lodash'`
2. **Lazy-load routes**: `React.lazy(() => import('./pages/Reports'))`
3. **Dynamic imports for heavy features**: `const chart = await import('recharts')`
4. **Check for duplicates**: `npx depcheck` to find unused deps

> See `references/bundle-optimization.md` for detailed strategies.

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
