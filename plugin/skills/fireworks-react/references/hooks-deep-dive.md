# Hooks Deep Dive -- Complete React Hook Reference

> Every React hook with advanced patterns, custom hook recipes, rules enforcement, and dependency array mastery.

---

## 1. useState -- Beyond the Basics

### Lazy Initialization

```tsx
// WRONG: expensive computation runs EVERY render
const [data, setData] = useState(parseCSV(rawData));

// RIGHT: lazy initializer runs only on mount
const [data, setData] = useState(() => parseCSV(rawData));
```

### Functional Updates (Stale Closure Prevention)

```tsx
// WRONG: stale closure when called rapidly
const increment = () => setCount(count + 1);

// RIGHT: functional updater always has latest state
const increment = () => setCount(prev => prev + 1);

// WRONG: batch issue with object state
const updateUser = () => {
  setUser({ ...user, name: 'the user' });
  setUser({ ...user, role: 'admin' }); // overwrites name change!
};

// RIGHT: functional updater chains correctly
const updateUser = () => {
  setUser(prev => ({ ...prev, name: 'the user' }));
  setUser(prev => ({ ...prev, role: 'admin' }));
};
```

### State Typing Patterns

```tsx
// Discriminated union state
type AsyncState<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'success'; data: T }
  | { status: 'error'; error: Error };

const [state, setState] = useState<AsyncState<Product[]>>({ status: 'idle' });

// Type-safe access
if (state.status === 'success') {
  console.log(state.data); // TypeScript knows data exists
}
```

---

## 2. useReducer -- Complex State Machines

### Typed Reducer Pattern

```tsx
interface InventoryState {
  items: Product[];
  filter: string;
  sortBy: 'name' | 'price' | 'stock';
  sortDir: 'asc' | 'desc';
  selection: Set<string>;
}

type InventoryAction =
  | { type: 'SET_ITEMS'; items: Product[] }
  | { type: 'SET_FILTER'; filter: string }
  | { type: 'TOGGLE_SORT'; column: 'name' | 'price' | 'stock' }
  | { type: 'TOGGLE_SELECT'; id: string }
  | { type: 'SELECT_ALL' }
  | { type: 'CLEAR_SELECTION' };

function inventoryReducer(state: InventoryState, action: InventoryAction): InventoryState {
  switch (action.type) {
    case 'SET_ITEMS':
      return { ...state, items: action.items };
    case 'SET_FILTER':
      return { ...state, filter: action.filter };
    case 'TOGGLE_SORT':
      return {
        ...state,
        sortBy: action.column,
        sortDir: state.sortBy === action.column && state.sortDir === 'asc' ? 'desc' : 'asc',
      };
    case 'TOGGLE_SELECT': {
      const next = new Set(state.selection);
      if (next.has(action.id)) next.delete(action.id); else next.add(action.id);
      return { ...state, selection: next };
    }
    case 'SELECT_ALL':
      return { ...state, selection: new Set(state.items.map(i => i.id)) };
    case 'CLEAR_SELECTION':
      return { ...state, selection: new Set() };
    default: {
      const _exhaustive: never = action;
      throw new Error(`Unhandled action: ${JSON.stringify(_exhaustive)}`);
    }
  }
}
```

### useReducer vs useState Decision

```
Is state a single primitive value? ------------> useState
Are there multiple related values? ------------> useReducer
Do state transitions have complex logic? ------> useReducer
Is the next state derived from previous? ------> useReducer (or useState with functional update)
Do you need to pass dispatch to deep children? -> useReducer + context
```

---

## 3. useEffect -- Complete Rules

### The Mental Model

useEffect is for **synchronizing with external systems**. Period. Not for derived state, not for data transformation, not for event handling.

### Effect Categories

```tsx
// Category 1: Subscriptions (ALWAYS need cleanup)
useEffect(() => {
  const ws = new WebSocket(url);
  ws.onmessage = (e) => setMessages(prev => [...prev, JSON.parse(e.data)]);
  return () => ws.close();
}, [url]);

// Category 2: DOM manipulation
useEffect(() => {
  const observer = new IntersectionObserver(([entry]) => {
    setIsVisible(entry.isIntersecting);
  }, { threshold: 0.1 });
  if (ref.current) observer.observe(ref.current);
  return () => observer.disconnect();
}, []);

// Category 3: Data fetching (always with AbortController)
useEffect(() => {
  const controller = new AbortController();
  async function fetchData() {
    try {
      const response = await fetch(url, { signal: controller.signal });
      const data = await response.json();
      setData(data);
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') return;
      setError(err as Error);
    }
  }
  fetchData();
  return () => controller.abort();
}, [url]);

// Category 4: Third-party library sync
useEffect(() => {
  const chart = new Chart(canvasRef.current!, config);
  return () => chart.destroy();
}, [config]);

// Category 5: Window/document events
useEffect(() => {
  const handler = (e: KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  };
  document.addEventListener('keydown', handler);
  return () => document.removeEventListener('keydown', handler);
}, [onClose]);
```

### What NEVER Belongs in useEffect

```tsx
// NEVER: Derived state
// BAD:
useEffect(() => { setFullName(`${first} ${last}`); }, [first, last]);
// GOOD:
const fullName = `${first} ${last}`;

// NEVER: Resetting state when props change
// BAD:
useEffect(() => { setSelection(null); }, [categoryId]);
// GOOD:
<ProductList key={categoryId} /> // key change = fresh component

// NEVER: Event-driven logic
// BAD:
useEffect(() => { if (submitted) { sendAnalytics(); } }, [submitted]);
// GOOD:
const handleSubmit = () => { save(); sendAnalytics(); };

// NEVER: Transforming data for rendering
// BAD:
useEffect(() => { setFilteredItems(items.filter(i => i.active)); }, [items]);
// GOOD:
const filteredItems = useMemo(() => items.filter(i => i.active), [items]);
```

---

## 4. useLayoutEffect -- DOM Measurements

```tsx
// Measure element before browser paints
function Tooltip({ targetRef, children }: TooltipProps) {
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, left: 0 });

  useLayoutEffect(() => {
    if (!targetRef.current || !tooltipRef.current) return;
    const targetRect = targetRef.current.getBoundingClientRect();
    const tooltipRect = tooltipRef.current.getBoundingClientRect();
    setPosition({
      top: targetRect.top - tooltipRect.height - 8,
      left: targetRect.left + (targetRect.width - tooltipRect.width) / 2,
    });
  }, [targetRef]);

  return createPortal(
    <div ref={tooltipRef} style={{ position: 'fixed', ...position }}>{children}</div>,
    document.body
  );
}
```

### useLayoutEffect vs useEffect

```
Does your effect READ from the DOM (measurements, scroll position)? --> useLayoutEffect
Does your effect WRITE to the DOM in a way the user might see flash? --> useLayoutEffect
Everything else (subscriptions, fetching, non-visual side effects)? --> useEffect
```

---

## 5. useRef -- Mutable Containers

### Common Patterns

```tsx
// DOM reference
const inputRef = useRef<HTMLInputElement>(null);
useEffect(() => { inputRef.current?.focus(); }, []);

// Mutable value that persists across renders (no re-render on change)
const renderCount = useRef(0);
renderCount.current += 1;

// Previous value tracking
function usePrevious<T>(value: T): T | undefined {
  const ref = useRef<T>();
  useEffect(() => { ref.current = value; });
  return ref.current;
}

// Stable callback (latest ref pattern)
function useStableCallback<T extends (...args: unknown[]) => unknown>(callback: T): T {
  const ref = useRef(callback);
  useLayoutEffect(() => { ref.current = callback; });
  return useCallback((...args: unknown[]) => ref.current(...args), []) as T;
}

// Timer/interval ref
function useInterval(callback: () => void, delay: number | null) {
  const savedCallback = useRef(callback);
  useLayoutEffect(() => { savedCallback.current = callback; });

  useEffect(() => {
    if (delay === null) return;
    const id = setInterval(() => savedCallback.current(), delay);
    return () => clearInterval(id);
  }, [delay]);
}
```

---

## 6. useContext -- Efficient Context

### Split Value and Dispatch

```tsx
// WRONG: single context causes all consumers to re-render
const AppContext = createContext<{ theme: Theme; user: User; setTheme: (t: Theme) => void }>();

// RIGHT: split into granular contexts
const ThemeContext = createContext<Theme>(defaultTheme);
const ThemeDispatchContext = createContext<(t: Theme) => void>(() => {});
const UserContext = createContext<User | null>(null);

// Typed context hook with safety check
function useTheme() {
  const theme = useContext(ThemeContext);
  if (theme === undefined) throw new Error('useTheme must be inside ThemeProvider');
  return theme;
}
```

### Context + Reducer Pattern

```tsx
function AppProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(appReducer, initialState);

  // Memoize to prevent provider value from changing every render
  const stateValue = useMemo(() => state, [state]);
  const dispatchValue = useMemo(() => dispatch, [dispatch]);

  return (
    <AppStateContext.Provider value={stateValue}>
      <AppDispatchContext.Provider value={dispatchValue}>
        {children}
      </AppDispatchContext.Provider>
    </AppStateContext.Provider>
  );
}
```

---

## 7. Concurrent Hooks (React 18+)

### useTransition -- Non-Blocking Updates

```tsx
function SearchWithTransition() {
  const [query, setQuery] = useState('');
  const [isPending, startTransition] = useTransition();

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setQuery(e.target.value); // urgent: update input immediately
    startTransition(() => {
      setSearchResults(filterLargeDataset(e.target.value)); // non-urgent: can be interrupted
    });
  };

  return (
    <>
      <input value={query} onChange={handleChange} />
      {isPending && <Spinner />}
      <ResultsList results={searchResults} />
    </>
  );
}
```

### useDeferredValue -- Deferred Rendering

```tsx
function SearchResults({ query }: { query: string }) {
  const deferredQuery = useDeferredValue(query);
  const isStale = query !== deferredQuery;

  const results = useMemo(() => filterProducts(deferredQuery), [deferredQuery]);

  return (
    <div className={cn(isStale && 'opacity-50 transition-opacity duration-200')}>
      {results.map(r => <ProductRow key={r.id} product={r} />)}
    </div>
  );
}
```

### useSyncExternalStore -- External Store Integration

```tsx
// Subscribe to browser APIs or non-React stores
function useOnlineStatus(): boolean {
  return useSyncExternalStore(
    (callback) => {
      window.addEventListener('online', callback);
      window.addEventListener('offline', callback);
      return () => {
        window.removeEventListener('online', callback);
        window.removeEventListener('offline', callback);
      };
    },
    () => navigator.onLine,
    () => true // server snapshot
  );
}

// Subscribe to window dimensions
function useWindowSize() {
  return useSyncExternalStore(
    (callback) => {
      window.addEventListener('resize', callback);
      return () => window.removeEventListener('resize', callback);
    },
    () => ({ width: window.innerWidth, height: window.innerHeight }),
    () => ({ width: 1920, height: 1080 })
  );
}
```

---

## 8. Custom Hook Library (the user Enterprise Toolkit)

### useIpcInvoke -- Electron IPC

```tsx
function useIpcInvoke<TArgs extends unknown[], TResult>(
  channel: string
): { invoke: (...args: TArgs) => Promise<TResult>; loading: boolean; error: Error | null } {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const invoke = useCallback(async (...args: TArgs): Promise<TResult> => {
    setLoading(true);
    setError(null);
    try {
      const result = await window.electronAPI.invoke(channel, ...args);
      return result as TResult;
    } catch (err) {
      const e = err instanceof Error ? err : new Error(String(err));
      setError(e);
      throw e;
    } finally {
      setLoading(false);
    }
  }, [channel]);

  return { invoke, loading, error };
}
```

### useDebounce / useDebouncedCallback

```tsx
function useDebounce<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return debounced;
}

function useDebouncedCallback<T extends (...args: unknown[]) => void>(
  callback: T,
  delay: number
): T {
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const callbackRef = useRef(callback);
  useLayoutEffect(() => { callbackRef.current = callback; });

  return useCallback((...args: unknown[]) => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => callbackRef.current(...args), delay);
  }, [delay]) as unknown as T;
}
```

### useClickOutside

```tsx
function useClickOutside<T extends HTMLElement>(
  ref: RefObject<T | null>,
  handler: () => void
) {
  const handlerRef = useRef(handler);
  useLayoutEffect(() => { handlerRef.current = handler; });

  useEffect(() => {
    const listener = (e: MouseEvent | TouchEvent) => {
      if (!ref.current || ref.current.contains(e.target as Node)) return;
      handlerRef.current();
    };
    document.addEventListener('mousedown', listener);
    document.addEventListener('touchstart', listener);
    return () => {
      document.removeEventListener('mousedown', listener);
      document.removeEventListener('touchstart', listener);
    };
  }, [ref]);
}
```

### useKeyboard

```tsx
function useKeyboard(keyMap: Record<string, () => void>) {
  const mapRef = useRef(keyMap);
  useLayoutEffect(() => { mapRef.current = keyMap; });

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const key = [
        e.ctrlKey && 'Ctrl',
        e.shiftKey && 'Shift',
        e.altKey && 'Alt',
        e.key,
      ].filter(Boolean).join('+');

      if (mapRef.current[key]) {
        e.preventDefault();
        mapRef.current[key]();
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);
}

// Usage
useKeyboard({
  'Ctrl+s': () => save(),
  'Ctrl+z': () => undo(),
  'Escape': () => close(),
});
```

### useMediaQuery

```tsx
function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window !== 'undefined' ? window.matchMedia(query).matches : false
  );

  useEffect(() => {
    const mql = window.matchMedia(query);
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches);
    mql.addEventListener('change', handler);
    setMatches(mql.matches);
    return () => mql.removeEventListener('change', handler);
  }, [query]);

  return matches;
}

// Convenience hooks
const useIsMobile = () => useMediaQuery('(max-width: 768px)');
const useIsTablet = () => useMediaQuery('(min-width: 769px) and (max-width: 1024px)');
const useIsDesktop = () => useMediaQuery('(min-width: 1025px)');
const usePrefersDark = () => useMediaQuery('(prefers-color-scheme: dark)');
```

---

## 9. Dependency Array Mastery

### The Golden Rules

1. **Include every value from component scope used inside the effect**
2. **Objects and arrays create new references every render** -- depend on primitives
3. **Functions create new references every render** -- wrap in useCallback or move inside effect
4. **Refs are stable** -- you can omit them from dependencies (but include .current reads)

### Common Traps

```tsx
// TRAP 1: Object in dependency
const options = { threshold: 0.5 }; // new object every render!
useEffect(() => { observe(options); }, [options]); // runs every render

// FIX: extract to module scope or useMemo
const OPTIONS = { threshold: 0.5 } as const; // outside component
useEffect(() => { observe(OPTIONS); }, [OPTIONS]); // stable

// TRAP 2: Exhaustive deps with unstable function
useEffect(() => {
  fetchData(onSuccess); // onSuccess changes every render
}, [onSuccess]); // infinite loop potential

// FIX: useCallback on the handler, or ref pattern
const onSuccessRef = useRef(onSuccess);
onSuccessRef.current = onSuccess;
useEffect(() => {
  fetchData((...args) => onSuccessRef.current(...args));
}, []); // stable -- ref pattern

// TRAP 3: Missing dep causes stale data
useEffect(() => {
  const id = setInterval(() => {
    console.log(count); // always logs initial count!
  }, 1000);
  return () => clearInterval(id);
}, []); // missing count dependency

// FIX: functional setState doesn't need dep
useEffect(() => {
  const id = setInterval(() => {
    setCount(prev => prev + 1); // always has latest
  }, 1000);
  return () => clearInterval(id);
}, []);
```

---

## 10. Rules of Hooks Enforcement

### ESLint Configuration

```json
{
  "plugins": ["react-hooks"],
  "rules": {
    "react-hooks/rules-of-hooks": "error",
    "react-hooks/exhaustive-deps": "warn"
  }
}
```

### Manual Verification Checklist

Before committing any component with hooks:

- [ ] No hooks inside conditions, loops, or early returns
- [ ] All custom hooks prefixed with `use`
- [ ] Every useEffect has a cleanup function (if subscribing to anything)
- [ ] Every useEffect dependency array is complete (no missing deps)
- [ ] No derived state in useEffect (computed inline instead)
- [ ] Functional updaters used where stale closures are possible
- [ ] useLayoutEffect only for DOM measurements
- [ ] No hooks called inside try/catch blocks
