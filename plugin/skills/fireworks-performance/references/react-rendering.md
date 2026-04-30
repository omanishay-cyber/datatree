# React Rendering — Deep Reference

> Part of the `fireworks-performance` skill. See `../SKILL.md` for the master guide.

---

## React DevTools Profiler: Complete Workflow

### Setup

1. Install React DevTools browser extension (or use standalone for Electron).
2. Open DevTools -> Profiler tab.
3. Click the gear icon -> Enable "Record why each component rendered."
4. Click "Highlight updates when components render" in Components tab for visual feedback.

### Recording a Profile

1. Click the blue record button in the Profiler tab.
2. Perform the exact interaction you want to measure (type in search, click sort, navigate).
3. Click the red stop button.
4. You now have a flame chart of every render that occurred.

### Reading the Flame Chart

- **Each bar** represents a component render.
- **Width** = render duration (wider = slower).
- **Color**:
  - Yellow/Orange = slow render (focus here)
  - Blue/Green = fast render (usually fine)
  - Gray = did not render (skipped by memo)
- **Commits** (top bar): Each commit is a batch of state updates that caused re-renders.

### Identifying Unnecessary Renders

1. **Sort by render time**: Click "Ranked" tab to see slowest components first.
2. **Check render count**: Click on a component to see how many times it rendered.
3. **Check "Why did this render?"**: With the setting enabled, you'll see:
   - "Props changed" — which prop? Was it necessary?
   - "State changed" — which state? Was the update needed?
   - "Parent rendered" — the parent re-rendered and this component wasn't memoized.
   - "Context changed" — a context value this component consumes changed.
   - "Hooks changed" — a hook dependency changed.

### Measuring Render Time

- Click on any component in the flame chart.
- Note the "Render duration" in the right panel.
- This is the self-time (excluding children).
- Compare before and after optimization.

---

## Common Re-Render Causes

### 1. Unstable References (Inline Objects/Functions)

```tsx
// PROBLEM: New object created every render -> child always re-renders
function Parent() {
  return <Child style={{ color: 'red' }} />;
  //            ^^^^^^^^^^^^^^^^^^^^^^^^^^ new reference every render
}

// FIX: Stable reference
const redStyle = { color: 'red' };
function Parent() {
  return <Child style={redStyle} />;
}

// PROBLEM: New function created every render
function Parent() {
  return <Child onClick={() => doSomething()} />;
  //            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ new reference every render
}

// FIX: useCallback (only if Child is memoized)
function Parent() {
  const handleClick = useCallback(() => doSomething(), []);
  return <MemoizedChild onClick={handleClick} />;
}
```

### 2. Missing React.memo on Expensive Children

```tsx
// PROBLEM: ExpensiveList re-renders every time Parent state changes
function Parent() {
  const [count, setCount] = useState(0);
  return (
    <div>
      <button onClick={() => setCount(c => c + 1)}>Count: {count}</button>
      <ExpensiveList items={items} /> {/* Re-renders on every count change! */}
    </div>
  );
}

// FIX: Memo the expensive child
const MemoizedExpensiveList = React.memo(ExpensiveList);
function Parent() {
  const [count, setCount] = useState(0);
  return (
    <div>
      <button onClick={() => setCount(c => c + 1)}>Count: {count}</button>
      <MemoizedExpensiveList items={items} />
    </div>
  );
}
```

### 3. Parent Re-Renders Cascade Down

```tsx
// PROBLEM: Every child re-renders when parent state changes
function Dashboard() {
  const [time, setTime] = useState(new Date()); // Updates every second
  return (
    <div>
      <Clock time={time} />
      <ProductTable />   {/* Re-renders every second for no reason! */}
      <SalesChart />     {/* Re-renders every second for no reason! */}
    </div>
  );
}

// FIX: Extract the frequently-updating part
function ClockWrapper() {
  const [time, setTime] = useState(new Date());
  return <Clock time={time} />;
}

function Dashboard() {
  return (
    <div>
      <ClockWrapper />   {/* Only this re-renders every second */}
      <ProductTable />   {/* Stable */}
      <SalesChart />     {/* Stable */}
    </div>
  );
}
```

### 4. Context Value Changes Cascade to All Consumers

```tsx
// PROBLEM: All consumers re-render when ANY value in context changes
const AppContext = createContext({ theme: 'dark', user: null, settings: {} });

// FIX: Split context by update frequency
const ThemeContext = createContext('dark');        // Rarely changes
const UserContext = createContext(null);           // Changes on login/logout
const SettingsContext = createContext({});         // Changes on settings page

// BETTER FIX: Use Zustand selectors instead of Context
const useTheme = () => useAppStore((s) => s.theme);
// Only re-renders when theme specifically changes
```

---

## Fix Patterns Summary

### Extract Stable References

Move object/array/function creation outside the component or into useMemo/useCallback.

### Memoize Zustand Selectors

```tsx
// BAD: selects entire store -> re-renders on any store change
const store = useAppStore();

// GOOD: selects only what's needed -> re-renders only when this slice changes
const products = useAppStore((s) => s.products);
const theme = useAppStore((s) => s.theme);
```

### Split Context into Separate Providers

One context per update frequency. Theme context doesn't need to re-render when user data changes.

### Component Composition (Children Pattern)

```tsx
// The children pattern prevents re-renders of passed children
function ScrollTracker({ children }: { children: React.ReactNode }) {
  const [scrollY, setScrollY] = useState(0);
  // children don't re-render when scrollY changes because they're created by the PARENT
  return <div onScroll={...}>{children}</div>;
}
```

---

## Profiling Workflow Checklist

1. [ ] Open React DevTools Profiler
2. [ ] Enable "Record why each component rendered"
3. [ ] Record the slow interaction
4. [ ] Switch to "Ranked" view -> find the slowest component
5. [ ] Click on it -> read "Why did this render?"
6. [ ] Identify the root cause (unstable ref, missing memo, context, parent)
7. [ ] Apply the appropriate fix pattern
8. [ ] Re-record the same interaction
9. [ ] Compare render time and render count
10. [ ] Document: before time, after time, percentage improvement
