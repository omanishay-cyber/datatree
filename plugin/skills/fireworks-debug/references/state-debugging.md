# State Debugging — Zustand Patterns

> Debugging state management issues in Zustand stores.
> Covers DevTools setup, custom logging, stale closures, and snapshot tools.

---

## 1. DevTools Middleware Setup

Add the devtools middleware to see store state in Redux DevTools:

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';

const useProductStore = create<ProductStore>()(
  devtools(
    (set, get) => ({
      products: [],
      loading: false,
      setProducts: (products) => set({ products }, false, 'setProducts'),
      addProduct: (product) =>
        set(
          (state) => ({ products: [...state.products, product] }),
          false,
          'addProduct'
        ),
    }),
    { name: 'ProductStore' }
  )
);
```

The third argument to `set()` is the action name — it appears in Redux DevTools for tracking.

Install Redux DevTools Extension in your browser to see the state tree, action history, and time-travel debugging.

---

## 2. Custom Logger Middleware

For console-based debugging without DevTools:

```typescript
import { StateCreator, StoreMutatorIdentifier } from 'zustand';

type Logger = <
  T,
  Mps extends [StoreMutatorIdentifier, unknown][] = [],
  Mcs extends [StoreMutatorIdentifier, unknown][] = [],
>(
  f: StateCreator<T, Mps, Mcs>,
  name?: string,
) => StateCreator<T, Mps, Mcs>;

type LoggerImpl = <T>(
  f: StateCreator<T, [], []>,
  name?: string,
) => StateCreator<T, [], []>;

const loggerImpl: LoggerImpl = (f, name) => (set, get, store) => {
  const loggedSet: typeof set = (...args) => {
    const prevState = get();
    set(...(args as Parameters<typeof set>));
    const nextState = get();
    console.group(`[STORE${name ? ` ${name}` : ''}] State Update`);
    console.log('Previous:', prevState);
    console.log('Next:', nextState);
    console.log('Changed keys:', Object.keys(nextState as object).filter(
      key => (prevState as any)[key] !== (nextState as any)[key]
    ));
    console.groupEnd();
  };
  return f(loggedSet, get, store);
};

export const logger = loggerImpl as Logger;

// Usage:
const useProductStore = create<ProductStore>()(
  logger(
    (set) => ({
      products: [],
      setProducts: (products) => set({ products }),
    }),
    'ProductStore'
  )
);
```

---

## 3. State Debugging Checklist

When a component shows wrong/stale data, check these 5 things in order:

### Check 1: Is the Store State Correct?
```typescript
// Log the raw store state:
console.log('Store state:', useProductStore.getState());
```
If the store state is wrong, the bug is in the action that updated the store (trace the action).
If the store state is correct, the bug is in how the component reads it (check selector).

### Check 2: Is the Selector Returning the Right Data?
```typescript
// Log what the selector returns:
const products = useProductStore((state) => {
  console.log('[SELECTOR] Full state:', state);
  console.log('[SELECTOR] Returning:', state.products);
  return state.products;
});
```

### Check 3: Is the Component Re-rendering When State Changes?
```typescript
// Add a render counter:
const renderCount = useRef(0);
renderCount.current++;
console.log(`[ProductList] Render #${renderCount.current}`);
```
If render count does not increase after a state change, the component is not subscribed properly.

### Check 4: Is the Equality Function Blocking Updates?
```typescript
// By default, Zustand uses Object.is for comparison.
// If your selector returns a new object/array each time, it will re-render every time.
// If you use shallow, it compares each key — but deeply nested changes are missed.

// Test: force re-render by removing equality function
const data = useProductStore((state) => state.products); // No equality = Object.is
```

### Check 5: Is There a Stale Closure?
```typescript
// If a callback or effect captures store state, it may be stale:
useEffect(() => {
  const handler = () => {
    // This captures 'products' at the time the effect ran
    console.log('Products:', products); // May be stale!
  };
  window.addEventListener('keydown', handler);
  return () => window.removeEventListener('keydown', handler);
}, [products]); // Must include products in deps!

// Alternative: use getState() for always-fresh reads:
useEffect(() => {
  const handler = () => {
    const current = useProductStore.getState().products; // Always fresh
    console.log('Products:', current);
  };
  window.addEventListener('keydown', handler);
  return () => window.removeEventListener('keydown', handler);
}, []); // No deps needed
```

---

## 4. State Snapshot for Bug Reports

Capture a full state snapshot when a bug occurs:

```typescript
function captureStateSnapshot(): string {
  const snapshot = {
    timestamp: new Date().toISOString(),
    stores: {
      products: useProductStore.getState(),
      settings: useSettingsStore.getState(),
      ui: useUIStore.getState(),
    },
    url: window.location.href,
    windowSize: { width: window.innerWidth, height: window.innerHeight },
  };
  return JSON.stringify(snapshot, null, 2);
}

// Attach to error boundary:
componentDidCatch(error: Error, errorInfo: ErrorInfo) {
  const snapshot = captureStateSnapshot();
  console.error('State at time of error:', snapshot);
  // Optionally save to file or send to logging service
}
```

---

## 5. Stale Closure Detection

The most common state bug in React + Zustand is the stale closure. Detect it with this pattern:

```typescript
// Detection helper: logs when a value is stale
function useStaleDetector<T>(name: string, value: T): void {
  const ref = useRef(value);
  useEffect(() => {
    if (ref.current !== value) {
      console.warn(`[STALE?] ${name} changed from`, ref.current, 'to', value);
      ref.current = value;
    }
  });
}

// Usage in component:
function ProductList() {
  const products = useProductStore((s) => s.products);
  useStaleDetector('products', products);
  // If "products" in a callback is stale, you will see the warning
  // showing the value changed but the callback still has the old one
}
```

---

## 6. Common State Bugs Quick Reference

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| Component does not update when store changes | Selector returns same reference | Use shallow comparison or atomic selectors |
| Component shows old data in callback | Stale closure | Use useRef or getState() |
| Component re-renders too often | Selector creates new object each call | Use useShallow or memoize selector |
| State resets on page navigation | Store re-created on mount | Ensure store is created outside component |
| Multiple components show different state | Different store instances | Ensure single store export, not factory |
| State update does not persist | Not saving to database after store update | Add persist middleware or manual save |
