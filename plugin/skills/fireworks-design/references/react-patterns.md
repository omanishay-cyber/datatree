# React Patterns — Deep Reference Guide

## Overview

All projects use React 18 with TypeScript strict mode. Components are ALWAYS functional —
never class components. State management uses Zustand. Styling uses Tailwind CSS with cn()
utility. These patterns are mandatory for all React code.

---

## Functional Components Only

```tsx
// CORRECT: functional component with TypeScript props
interface CardProps {
  title: string;
  description?: string;
  variant?: 'default' | 'outlined' | 'ghost';
  className?: string;
  children: React.ReactNode;
}

export function Card({ title, description, variant = 'default', className, children }: CardProps) {
  return (
    <div className={cn(cardVariants({ variant }), className)}>
      <h3 className="text-lg font-medium">{title}</h3>
      {description && <p className="text-sm text-muted-foreground mt-1">{description}</p>}
      <div className="mt-4">{children}</div>
    </div>
  );
}

// WRONG: class component — NEVER use
class Card extends React.Component { /* ... */ }
```

---

## Custom Hooks Library

### useDebounce
```tsx
function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);

  return debouncedValue;
}

// Usage: search input that waits 300ms after typing stops
const debouncedSearch = useDebounce(searchTerm, 300);
```

### useLocalStorage
```tsx
function useLocalStorage<T>(key: string, initialValue: T): [T, (value: T | ((prev: T) => T)) => void] {
  const [storedValue, setStoredValue] = useState<T>(() => {
    try {
      const item = window.localStorage.getItem(key);
      return item ? JSON.parse(item) : initialValue;
    } catch {
      return initialValue;
    }
  });

  const setValue = useCallback((value: T | ((prev: T) => T)) => {
    setStoredValue(prev => {
      const nextValue = value instanceof Function ? value(prev) : value;
      window.localStorage.setItem(key, JSON.stringify(nextValue));
      return nextValue;
    });
  }, [key]);

  return [storedValue, setValue];
}

// Usage
const [theme, setTheme] = useLocalStorage('theme', 'dark');
```

### useMediaQuery
```tsx
function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window !== 'undefined' ? window.matchMedia(query).matches : false
  );

  useEffect(() => {
    const mediaQuery = window.matchMedia(query);
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches);
    mediaQuery.addEventListener('change', handler);
    return () => mediaQuery.removeEventListener('change', handler);
  }, [query]);

  return matches;
}

// Usage
const isMobile = useMediaQuery('(max-width: 640px)');
const prefersDark = useMediaQuery('(prefers-color-scheme: dark)');
```

### useClickOutside
```tsx
function useClickOutside<T extends HTMLElement>(
  callback: () => void
): React.RefObject<T> {
  const ref = useRef<T>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        callback();
      }
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [callback]);

  return ref;
}

// Usage
const dropdownRef = useClickOutside<HTMLDivElement>(() => setIsOpen(false));
```

---

## React.memo — When to Use

```tsx
// USE React.memo when:
// 1. Component receives complex props and parent re-renders often
const ExpensiveList = React.memo(function ExpensiveList({ items }: { items: Item[] }) {
  return items.map(item => <ListItem key={item.id} item={item} />);
});

// 2. Component is in a list rendered by a frequently-updating parent
const TableRow = React.memo(function TableRow({ row }: { row: RowData }) {
  return (
    <tr>
      {row.cells.map(cell => <td key={cell.id}>{cell.value}</td>)}
    </tr>
  );
});

// DO NOT use React.memo when:
// - Component always receives new props (defeats the purpose)
// - Component is small/cheap to render
// - Component uses children prop (children are always new references)
```

---

## useMemo and useCallback — Correct Usage

```tsx
// useMemo: ONLY for expensive computations
function ProductTable({ products, filter }: Props) {
  // GOOD: filtering a large list
  const filteredProducts = useMemo(
    () => products.filter(p => p.category === filter),
    [products, filter]
  );

  // BAD: simple derived value — no need for useMemo
  // const count = useMemo(() => products.length, [products]);
  const count = products.length; // Just compute directly

  return <Table data={filteredProducts} />;
}

// useCallback: ONLY when passing to memoized children
function Parent() {
  // GOOD: handler passed to React.memo child
  const handleSelect = useCallback((id: string) => {
    setSelectedId(id);
  }, []);

  return <MemoizedList onSelect={handleSelect} />;

  // BAD: useCallback on every handler — unnecessary
  // const handleClick = useCallback(() => { ... }, []);
  // <button onClick={handleClick}> — button is not memoized
}
```

---

## Suspense and Lazy Loading

```tsx
import { lazy, Suspense } from 'react';

// Lazy-load route components
const Dashboard = lazy(() => import('./pages/Dashboard'));
const Settings = lazy(() => import('./pages/Settings'));
const Reports = lazy(() => import('./pages/Reports'));

// Meaningful loading fallback — not just a spinner
function PageSkeleton() {
  return (
    <div className="p-6 space-y-6 animate-pulse">
      <div className="h-8 w-48 bg-muted rounded" />
      <div className="grid grid-cols-4 gap-4">
        {[...Array(4)].map((_, i) => (
          <div key={i} className="h-24 bg-muted rounded-xl" />
        ))}
      </div>
      <div className="h-64 bg-muted rounded-xl" />
    </div>
  );
}

// Wrap routes with Suspense
function App() {
  return (
    <Suspense fallback={<PageSkeleton />}>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/reports" element={<Reports />} />
      </Routes>
    </Suspense>
  );
}
```

---

## Error Boundaries

```tsx
interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

// Note: error boundaries MUST be class components (React limitation)
// This is the ONE exception to the "no class components" rule
class ErrorBoundary extends React.Component<
  { children: React.ReactNode; fallback?: React.ReactNode },
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('Error boundary caught:', error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return this.props.fallback || (
        <div className="flex flex-col items-center justify-center p-8 text-center">
          <AlertTriangle className="w-12 h-12 text-destructive mb-4" />
          <h2 className="text-xl font-semibold mb-2">Something went wrong</h2>
          <p className="text-sm text-muted-foreground mb-4">
            {this.state.error?.message}
          </p>
          <button
            onClick={() => this.setState({ hasError: false, error: null })}
            className="px-4 py-2 bg-primary text-primary-foreground rounded-lg"
          >
            Try Again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

// Usage: one per route, one for the app
function App() {
  return (
    <ErrorBoundary fallback={<AppCrashScreen />}>
      <Routes>
        <Route path="/" element={
          <ErrorBoundary>
            <Dashboard />
          </ErrorBoundary>
        } />
      </Routes>
    </ErrorBoundary>
  );
}
```

---

## Compound Components Pattern

Share implicit state between related components using Context:

```tsx
// Context for the compound component
interface TabsContextValue {
  value: string;
  onValueChange: (value: string) => void;
}

const TabsContext = React.createContext<TabsContextValue | null>(null);

function useTabsContext() {
  const ctx = React.useContext(TabsContext);
  if (!ctx) throw new Error('Tab components must be used within Tabs');
  return ctx;
}

// Root component
function Tabs({ value, onValueChange, children }: TabsProps) {
  return (
    <TabsContext.Provider value={{ value, onValueChange }}>
      <div className="space-y-2">{children}</div>
    </TabsContext.Provider>
  );
}

// List of tab triggers
function TabsList({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div role="tablist" className={cn("flex gap-1 p-1 backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-lg", className)}>
      {children}
    </div>
  );
}

// Individual tab trigger
function TabsTrigger({ value, children }: { value: string; children: React.ReactNode }) {
  const { value: activeValue, onValueChange } = useTabsContext();
  const isActive = activeValue === value;

  return (
    <button
      role="tab"
      aria-selected={isActive}
      onClick={() => onValueChange(value)}
      className={cn(
        "px-3 py-1.5 text-sm font-medium rounded-md transition-all duration-200",
        isActive
          ? "bg-primary text-primary-foreground shadow-sm"
          : "text-muted-foreground hover:text-foreground hover:bg-white/10"
      )}
    >
      {children}
    </button>
  );
}

// Tab content panel
function TabsContent({ value, children }: { value: string; children: React.ReactNode }) {
  const { value: activeValue } = useTabsContext();
  if (activeValue !== value) return null;

  return (
    <div role="tabpanel" className="mt-2">
      {children}
    </div>
  );
}

// Export as compound
Tabs.List = TabsList;
Tabs.Trigger = TabsTrigger;
Tabs.Content = TabsContent;
```

---

## cn() Utility — The Foundation

```tsx
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

### Usage Patterns
```tsx
// Conditional classes
cn("base-class", isActive && "active-class")

// Variant selection
cn("base", variant === "primary" && "bg-primary", variant === "ghost" && "bg-transparent")

// Spreading user className (always last)
cn("internal-styles", className)

// Complex conditions
cn(
  "flex items-center gap-2 rounded-lg transition-all duration-200",
  size === "sm" && "px-2 py-1 text-sm",
  size === "md" && "px-3 py-2 text-base",
  size === "lg" && "px-4 py-3 text-lg",
  disabled && "opacity-50 pointer-events-none",
  className
)
```

---

## Controlled vs Uncontrolled Components

```tsx
// CONTROLLED: use for forms with validation, cross-field dependencies
function ControlledInput({ value, onChange, error }: ControlledInputProps) {
  return (
    <input
      value={value}
      onChange={e => onChange(e.target.value)}
      aria-invalid={!!error}
      className={cn("input-base", error && "border-destructive")}
    />
  );
}

// UNCONTROLLED: use for simple inputs, file uploads, one-time reads
function UncontrolledInput({ defaultValue, name }: UncontrolledInputProps) {
  return <input defaultValue={defaultValue} name={name} className="input-base" />;
}
```

---

## Key Prop Best Practices

```tsx
// GOOD: stable unique ID
{items.map(item => <Card key={item.id} data={item} />)}

// GOOD: composite key when no single unique field
{items.map(item => <Card key={`${item.category}-${item.name}`} data={item} />)}

// BAD: array index for dynamic lists (causes bugs on reorder/delete)
{items.map((item, index) => <Card key={index} data={item} />)}

// ACCEPTABLE: array index for static, never-reordered lists
{staticMenuItems.map((item, index) => <MenuItem key={index} {...item} />)}
```

---

## forwardRef for Reusable Components

```tsx
interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, label, error, id, ...props }, ref) => {
    const inputId = id || useId();

    return (
      <div className="space-y-1.5">
        {label && (
          <label htmlFor={inputId} className="text-sm font-medium">
            {label}
          </label>
        )}
        <input
          ref={ref}
          id={inputId}
          className={cn(
            "w-full px-3 py-2 rounded-lg",
            "backdrop-blur-sm bg-white/5 dark:bg-black/10",
            "border border-white/20 focus:border-primary",
            "focus-visible:ring-2 focus-visible:ring-primary/50",
            "transition-all duration-200",
            "placeholder:text-muted-foreground/50",
            error && "border-destructive focus:border-destructive",
            className
          )}
          aria-invalid={!!error}
          {...props}
        />
        {error && (
          <p className="text-xs text-destructive flex items-center gap-1">
            {error}
          </p>
        )}
      </div>
    );
  }
);
Input.displayName = 'Input';
```
