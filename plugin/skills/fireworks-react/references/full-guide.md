# fireworks-react — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 3. Hooks Quick-Reference

### Built-in Hooks Matrix

| Hook | Purpose | Common Mistake | Correct Usage |
|------|---------|---------------|---------------|
| `useState` | Local UI state | Mutating state directly | Always return new reference |
| `useReducer` | Complex state logic | Not typing actions | Discriminated union actions |
| `useEffect` | Sync with externals | Derived state in effect | Compute inline instead |
| `useLayoutEffect` | DOM measurements | Using for non-DOM work | Only for layout reads |
| `useMemo` | Expensive computations | Memoizing cheap ops | Profile first, memo second |
| `useCallback` | Stable fn references | Wrapping everything | Only when passed to memo'd children |
| `useRef` | Mutable container | Rendering ref.current | Refs don't trigger re-renders |
| `useContext` | Read context value | Giant monolithic context | Split value/dispatch contexts |
| `useId` | Unique IDs for a11y | Manual ID generation | Let React handle SSR-safe IDs |
| `useDeferredValue` | Deferred updates | Using for critical UI | Only for non-urgent renders |
| `useTransition` | Non-blocking updates | Wrapping fast ops | Only for expensive state transitions |
| `useSyncExternalStore` | External store sync | Manual subscription | Use for non-React state sources |
| `useImperativeHandle` | Custom ref API | Exposing internals | Minimal API surface only |

### Rules of Hooks -- Enforced, Not Optional

1. Only call hooks at the **top level** -- never inside conditions, loops, or nested functions
2. Only call hooks from **React function components** or **custom hooks**
3. Custom hooks MUST start with `use` -- this enables lint enforcement
4. Never call hooks inside `try/catch` -- the hook call must be unconditional

### Wrong vs Right Hook Patterns

```tsx
// WRONG: Conditional hook
function Profile({ userId }: { userId?: string }) {
  if (userId) {
    const user = useUser(userId); // BREAKS rules of hooks
  }
}

// RIGHT: Always call, handle null
function Profile({ userId }: { userId?: string }) {
  const user = useUser(userId ?? '');
  if (!userId) return <EmptyState />;
  return <UserCard user={user} />;
}

// WRONG: useEffect for derived state
function Cart({ items }: { items: CartItem[] }) {
  const [total, setTotal] = useState(0);
  useEffect(() => {
    setTotal(items.reduce((sum, i) => sum + i.price * i.qty, 0));
  }, [items]);
}

// RIGHT: Compute inline -- zero extra renders
function Cart({ items }: { items: CartItem[] }) {
  const total = items.reduce((sum, i) => sum + i.price * i.qty, 0);
}

// WRONG: Stale closure
function Counter() {
  const [count, setCount] = useState(0);
  const increment = () => setCount(count + 1); // stale if called rapidly
}

// RIGHT: Functional updater
function Counter() {
  const [count, setCount] = useState(0);
  const increment = () => setCount(prev => prev + 1);
}
```

### Custom Hook Recipes (the user Toolkit)

```tsx
// useIpcListener -- Electron IPC bridge
function useIpcListener<T>(channel: string, handler: (data: T) => void) {
  useEffect(() => {
    const unsubscribe = window.electronAPI.on(channel, handler);
    return unsubscribe;
  }, [channel, handler]);
}

// useClickOutside
function useClickOutside(ref: RefObject<HTMLElement>, handler: () => void) {
  useEffect(() => {
    const listener = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) handler();
    };
    document.addEventListener('mousedown', listener);
    return () => document.removeEventListener('mousedown', listener);
  }, [ref, handler]);
}

// usePrevious
function usePrevious<T>(value: T): T | undefined {
  const ref = useRef<T>();
  useEffect(() => { ref.current = value; });
  return ref.current;
}
```

> Deep dive: [references/hooks-deep-dive.md](references/hooks-deep-dive.md)

---

## 4. Zustand State Management

Zustand is the user PRIMARY state library. Master it.

### Store Design Principles

1. **One store per domain** -- `useInventoryStore`, `useAuthStore`, `useSyncStore`
2. **Flat state** -- avoid deeply nested objects
3. **Selectors for performance** -- never subscribe to entire store
4. **Actions inside store** -- collocate state + logic
5. **TypeScript first** -- full type safety on store shape

### Basic Store Pattern

```tsx
import { create } from 'zustand';

interface InventoryState {
  items: Product[];
  searchQuery: string;
  selectedCategory: string | null;
  // Actions
  setItems: (items: Product[]) => void;
  setSearchQuery: (query: string) => void;
  selectCategory: (cat: string | null) => void;
  addItem: (item: Product) => void;
  removeItem: (id: string) => void;
}

export const useInventoryStore = create<InventoryState>((set) => ({
  items: [],
  searchQuery: '',
  selectedCategory: null,
  setItems: (items) => set({ items }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  selectCategory: (selectedCategory) => set({ selectedCategory }),
  addItem: (item) => set((state) => ({ items: [...state.items, item] })),
  removeItem: (id) => set((state) => ({ items: state.items.filter(i => i.id !== id) })),
}));
```

### Selector Pattern (Prevent Unnecessary Re-renders)

```tsx
// WRONG: subscribes to ENTIRE store -- re-renders on ANY change
function SearchBar() {
  const store = useInventoryStore();
  return <input value={store.searchQuery} onChange={e => store.setSearchQuery(e.target.value)} />;
}

// RIGHT: subscribe to exactly what you need
function SearchBar() {
  const searchQuery = useInventoryStore((s) => s.searchQuery);
  const setSearchQuery = useInventoryStore((s) => s.setSearchQuery);
  return <input value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />;
}

// ADVANCED: shallow equality for object selectors
import { useShallow } from 'zustand/react/shallow';

function ProductFilters() {
  const { searchQuery, selectedCategory } = useInventoryStore(
    useShallow((s) => ({ searchQuery: s.searchQuery, selectedCategory: s.selectedCategory }))
  );
}
```

### Persist Middleware (Electron/localStorage)

```tsx
import { persist, createJSONStorage } from 'zustand/middleware';

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: 'system' as 'light' | 'dark' | 'system',
      sidebarCollapsed: false,
      setTheme: (theme) => set({ theme }),
      toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
    }),
    {
      name: 'app-settings',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ theme: state.theme, sidebarCollapsed: state.sidebarCollapsed }),
    }
  )
);
```

### Async Actions

```tsx
export const useSyncStore = create<SyncState>((set, get) => ({
  syncing: false,
  lastSync: null as Date | null,
  error: null as string | null,

  syncNow: async () => {
    if (get().syncing) return; // debounce
    set({ syncing: true, error: null });
    try {
      await window.electronAPI.invoke('sync:push');
      set({ syncing: false, lastSync: new Date() });
    } catch (err) {
      set({ syncing: false, error: (err as Error).message });
    }
  },
}));
```

> Deep dive: [references/zustand-patterns.md](references/zustand-patterns.md)

---

## 5. React 19 Features

### Server Components (RSC)

```tsx
// Server Component -- runs on server, zero client JS
async function ProductPage({ id }: { id: string }) {
  const product = await db.query('SELECT * FROM products WHERE id = ?', [id]);
  return <ProductDetails product={product} />;
}

// Client Component -- needs 'use client' directive
'use client';
function AddToCartButton({ productId }: { productId: string }) {
  const [pending, setPending] = useState(false);
  return <button onClick={() => addToCart(productId)}>Add to Cart</button>;
}
```

### Server/Client Boundary Rules

| Can do in Server Component | Cannot do in Server Component |
|---------------------------|-------------------------------|
| Async/await directly | useState, useEffect |
| Database queries | Event handlers (onClick) |
| File system access | Browser APIs |
| Import Server Components | useContext |
| Pass serializable props to Client | Pass functions as props to Client |

### Actions (React 19)

```tsx
// Server Action
'use server';
async function updateProduct(formData: FormData) {
  const name = formData.get('name') as string;
  await db.query('UPDATE products SET name = ? WHERE id = ?', [name, id]);
  revalidatePath('/products');
}

// Client-side form using action
function EditProductForm({ product }: { product: Product }) {
  return (
    <form action={updateProduct}>
      <input name="name" defaultValue={product.name} />
      <SubmitButton />
    </form>
  );
}

function SubmitButton() {
  const { pending } = useFormStatus();
  return <button type="submit" disabled={pending}>{pending ? 'Saving...' : 'Save'}</button>;
}
```

### use() Hook (React 19)

```tsx
// Read a promise during render -- replaces useEffect for data fetching
function UserProfile({ userPromise }: { userPromise: Promise<User> }) {
  const user = use(userPromise); // suspends until resolved
  return <div>{user.name}</div>;
}

// Read context conditionally (only hook that allows this)
function Theme({ children }: { children: React.ReactNode }) {
  if (someCondition) {
    const theme = use(ThemeContext); // legal with use()!
    return <div className={theme}>{children}</div>;
  }
  return <>{children}</>;
}
```

### useOptimistic (React 19)

```tsx
function TodoList({ todos }: { todos: Todo[] }) {
  const [optimisticTodos, addOptimistic] = useOptimistic(
    todos,
    (state, newTodo: Todo) => [...state, { ...newTodo, pending: true }]
  );

  async function addTodo(formData: FormData) {
    const title = formData.get('title') as string;
    addOptimistic({ id: crypto.randomUUID(), title, pending: true });
    await saveTodo(title); // server action
  }

  return (
    <form action={addTodo}>
      <input name="title" />
      {optimisticTodos.map(todo => (
        <div key={todo.id} className={cn(todo.pending && 'opacity-50')}>{todo.title}</div>
      ))}
    </form>
  );
}
```

---

## 6. Memoization Decision Tree

```
Should I memoize this?
|
+-- Is it a value computed from props/state?
|   +-- Is the computation expensive (sort, filter, transform 100+ items)? --> useMemo
|   +-- Is it cheap (string concat, simple math)? --> NO, compute inline
|
+-- Is it a function?
|   +-- Passed to a React.memo'd child? --> useCallback
|   +-- Passed to a native DOM element? --> NO (React handles this efficiently)
|   +-- Used in a useEffect dependency? --> useCallback
|   +-- None of the above? --> NO
|
+-- Is it a component?
|   +-- Parent re-renders often AND component render is expensive? --> React.memo
|   +-- Component is simple/fast? --> NO (memo has overhead too)
```

### Over-Memoization Anti-Pattern

```tsx
// ANTI-PATTERN: memoizing everything
function Form() {
  const label = useMemo(() => 'Submit', []);           // POINTLESS -- string literal
  const style = useMemo(() => ({ color: 'red' }), []); // POINTLESS -- extract as constant
  const handler = useCallback(() => save(), [save]);    // POINTLESS if <button> not memo'd
  return <button style={style} onClick={handler}>{label}</button>;
}

// CORRECT: memo only what matters
const STYLE = { color: 'red' } as const; // constant outside component
function Form() {
  return <button style={STYLE} onClick={save}>Submit</button>;
}
```

---

## 7. Error Boundaries

### Route-Level Error Boundary (the user Standard)

Every route-level component MUST be wrapped in an error boundary.

```tsx
interface ErrorBoundaryProps {
  children: React.ReactNode;
  fallback?: React.ReactNode;
  onError?: (error: Error, info: React.ErrorInfo) => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('[ErrorBoundary]', error, info.componentStack);
    this.props.onError?.(error, info);
  }

  render() {
    if (this.state.hasError) {
      return this.props.fallback ?? <DefaultErrorFallback error={this.state.error} onRetry={() => this.setState({ hasError: false, error: null })} />;
    }
    return this.props.children;
  }
}

function DefaultErrorFallback({ error, onRetry }: { error: Error | null; onRetry: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center min-h-[400px] gap-4">
      <div className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-2xl p-8 text-center max-w-md">
        <h2 className="text-xl font-semibold mb-2">Something went wrong</h2>
        <p className="text-sm opacity-70 mb-4">{error?.message ?? 'An unexpected error occurred'}</p>
        <button onClick={onRetry} className="px-4 py-2 rounded-lg bg-gradient-to-r from-[#4191E1] to-[#41E1B5] text-white transition-all duration-200 hover:scale-105">
          Try Again
        </button>
      </div>
    </div>
  );
}
```

### Boundary Placement Strategy

```tsx
function App() {
  return (
    <ErrorBoundary fallback={<AppCrashScreen />}>     {/* App-level: catches everything */}
      <Layout>
        <Suspense fallback={<PageSkeleton />}>
          <ErrorBoundary fallback={<PageError />}>    {/* Route-level: isolates pages */}
            <Routes>
              <Route path="/inventory" element={
                <ErrorBoundary fallback={<InventoryError />}>  {/* Feature-level: granular */}
                  <InventoryPage />
                </ErrorBoundary>
              } />
            </Routes>
          </ErrorBoundary>
        </Suspense>
      </Layout>
    </ErrorBoundary>
  );
}
```

### Recovery Patterns

```tsx
// Reset boundary when route changes
function RouteErrorBoundary({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  return <ErrorBoundary key={location.pathname}>{children}</ErrorBoundary>;
}
```

---

## 8. Performance Optimization

### Re-render Detection Checklist

When a component re-renders unexpectedly:

1. **Check parent** -- is the parent re-rendering? (most common cause)
2. **Check props** -- are object/array/function props creating new references?
3. **Check context** -- is a context provider value changing?
4. **Check Zustand selector** -- subscribing to entire store instead of slices?

### React DevTools Profiler Workflow

```
1. Open React DevTools -> Profiler tab
2. Click Record
3. Perform the action that feels slow
4. Stop recording
5. Look for: yellow/red flamegraph bars (slow components)
6. Check "Why did this render?" for each slow component
7. Fix the top offender first -- usually 1 component causes 80% of issues
```

### Programmatic Profiler

```tsx
<Profiler id="InventoryTable" onRender={(id, phase, actualDuration) => {
  if (actualDuration > 16) {
    console.warn(`[Perf] ${id} ${phase} took ${actualDuration.toFixed(1)}ms`);
  }
}}>
  <InventoryTable />
</Profiler>
```

### Lazy Loading Strategy

```tsx
// Route-level splitting (always do this)
const Inventory = lazy(() => import('./pages/Inventory'));
const Reports = lazy(() => import('./pages/Reports'));
const Settings = lazy(() => import('./pages/Settings'));

// Component-level splitting (for heavy components)
const RichTextEditor = lazy(() => import('./components/RichTextEditor'));
const ChartDashboard = lazy(() => import('./components/ChartDashboard'));

// Preload on hover (premium UX)
function NavLink({ to, label, loader }: NavLinkProps) {
  return (
    <Link to={to} onMouseEnter={() => loader()}>
      {label}
    </Link>
  );
}
// Usage: <NavLink to="/reports" label="Reports" loader={() => import('./pages/Reports')} />
```

### Virtualization for Large Lists

```tsx
import { useVirtualizer } from '@tanstack/react-virtual';

function VirtualInventoryList({ items }: { items: Product[] }) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 64,
    overscan: 5,
  });

  return (
    <div ref={parentRef} className="h-[600px] overflow-auto">
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map(row => (
          <div key={row.key} style={{
            position: 'absolute', top: 0, left: 0, width: '100%',
            height: row.size, transform: `translateY(${row.start}px)`,
          }}>
            <ProductRow product={items[row.index]} />
          </div>
        ))}
      </div>
    </div>
  );
}
```

> Deep dive: [references/performance-react.md](references/performance-react.md)

---

## 9. TypeScript + React Patterns

### Typed Props with Discriminated Unions

```tsx
type ModalProps =
  | { variant: 'confirm'; onConfirm: () => void; onCancel: () => void; message: string }
  | { variant: 'alert'; onDismiss: () => void; message: string }
  | { variant: 'form'; onSubmit: (data: FormData) => void; children: React.ReactNode };

function Modal(props: ModalProps) {
  switch (props.variant) {
    case 'confirm':
      return <ConfirmModal {...props} />;
    case 'alert':
      return <AlertModal {...props} />;
    case 'form':
      return <FormModal {...props} />;
  }
}
```

### Generic Components

```tsx
interface TableProps<T> {
  data: T[];
  columns: ColumnDef<T>[];
  keyExtractor: (row: T) => string;
  onRowClick?: (row: T) => void;
  emptyState?: React.ReactNode;
}

export function Table<T>({ data, columns, keyExtractor, onRowClick, emptyState }: TableProps<T>) {
  if (data.length === 0) return <>{emptyState ?? <EmptyTable />}</>;
  return (
    <table className="w-full">
      <thead>
        <tr>{columns.map(col => <th key={col.key}>{col.header}</th>)}</tr>
      </thead>
      <tbody>
        {data.map(row => (
          <tr key={keyExtractor(row)} onClick={() => onRowClick?.(row)} className="cursor-pointer hover:bg-white/5 transition-all duration-200">
            {columns.map(col => <td key={col.key}>{col.render(row)}</td>)}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
```

### Event Typing Reference

```tsx
const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {};
const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => { e.preventDefault(); };
const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {};
const handleMouseEnter = (e: React.MouseEvent<HTMLDivElement>) => {};
const handleFocus = (e: React.FocusEvent<HTMLInputElement>) => {};
const handleDrag = (e: React.DragEvent<HTMLDivElement>) => {};
```

### Ref Typing

```tsx
const inputRef = useRef<HTMLInputElement>(null);
const divRef = useRef<HTMLDivElement>(null);
const canvasRef = useRef<HTMLCanvasElement>(null);
const timerRef = useRef<ReturnType<typeof setTimeout>>(null);
```

### Strict Mode Patterns

```tsx
// No `any` -- use `unknown` + type guard
function processApiResponse(data: unknown): Product {
  if (!isProduct(data)) throw new Error('Invalid product data');
  return data;
}

function isProduct(data: unknown): data is Product {
  return typeof data === 'object' && data !== null && 'id' in data && 'name' in data;
}

// Exhaustive switch with never
function getStatusColor(status: OrderStatus): string {
  switch (status) {
    case 'pending': return 'text-yellow-400';
    case 'shipped': return 'text-blue-400';
    case 'delivered': return 'text-green-400';
    case 'cancelled': return 'text-red-400';
    default: {
      const _exhaustive: never = status;
      throw new Error(`Unhandled status: ${_exhaustive}`);
    }
  }
}
```

---

## 10. Framer Motion Integration

the user uses Framer Motion for all premium animations.

### Page Transitions

```tsx
import { motion, AnimatePresence } from 'framer-motion';

const pageVariants = {
  initial: { opacity: 0, y: 20 },
  animate: { opacity: 1, y: 0, transition: { duration: 0.3, ease: 'easeOut' } },
  exit: { opacity: 0, y: -20, transition: { duration: 0.2 } },
};

function AnimatedRoutes() {
  const location = useLocation();
  return (
    <AnimatePresence mode="wait">
      <motion.div key={location.pathname} variants={pageVariants} initial="initial" animate="animate" exit="exit">
        <Routes location={location}>
          <Route path="/inventory" element={<Inventory />} />
          <Route path="/reports" element={<Reports />} />
        </Routes>
      </motion.div>
    </AnimatePresence>
  );
}
```

### Layout Animations

```tsx
// Smooth reordering
function SortableList({ items }: { items: Item[] }) {
  return (
    <div className="flex flex-col gap-2">
      <AnimatePresence>
        {items.map(item => (
          <motion.div
            key={item.id}
            layout
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            transition={{ type: 'spring', stiffness: 500, damping: 30 }}
            className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-xl p-4"
          >
            {item.name}
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
```

### Gesture Responses

```tsx
function DraggableCard({ children }: { children: React.ReactNode }) {
  return (
    <motion.div
      drag
      dragConstraints={{ left: 0, right: 0, top: 0, bottom: 0 }}
      dragElastic={0.1}
      whileHover={{ scale: 1.02, transition: { duration: 0.2 } }}
      whileTap={{ scale: 0.98 }}
      className="cursor-grab active:cursor-grabbing"
    >
      {children}
    </motion.div>
  );
}
```

### Staggered Children

```tsx
const containerVariants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.05, delayChildren: 0.1 },
  },
};

const childVariants = {
  hidden: { opacity: 0, y: 20 },
  visible: { opacity: 1, y: 0, transition: { type: 'spring', stiffness: 300, damping: 24 } },
};

function StaggeredGrid({ items }: { items: Product[] }) {
  return (
    <motion.div variants={containerVariants} initial="hidden" animate="visible" className="grid grid-cols-3 gap-4">
      {items.map(item => (
        <motion.div key={item.id} variants={childVariants} className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-xl p-4">
          <ProductCard product={item} />
        </motion.div>
      ))}
    </motion.div>
  );
}
```

---

## 11. Tailwind + cn() Patterns

### The cn() Utility (clsx + twMerge)

```tsx
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
```

### Conditional Classes

```tsx
function Button({ variant = 'primary', size = 'md', disabled, className, children }: ButtonProps) {
  return (
    <button
      disabled={disabled}
      className={cn(
        // Base
        'inline-flex items-center justify-center rounded-lg font-medium transition-all duration-200',
        // Variants
        variant === 'primary' && 'bg-gradient-to-r from-[#4191E1] to-[#41E1B5] text-white hover:scale-105',
        variant === 'secondary' && 'backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 hover:bg-white/20',
        variant === 'ghost' && 'hover:bg-white/10',
        variant === 'danger' && 'bg-red-500/20 text-red-400 border border-red-500/30 hover:bg-red-500/30',
        // Sizes
        size === 'sm' && 'px-3 py-1.5 text-sm',
        size === 'md' && 'px-4 py-2 text-base',
        size === 'lg' && 'px-6 py-3 text-lg',
        // States
        disabled && 'opacity-50 cursor-not-allowed pointer-events-none',
        // Override
        className,
      )}
    >
      {children}
    </button>
  );
}
```

### Glassmorphism Components (the user Standard)

```tsx
// Card with glassmorphism
function GlassCard({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn(
      'backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-2xl p-6',
      'shadow-lg shadow-black/5',
      className,
    )}>
      {children}
    </div>
  );
}

// Input with glassmorphism
function GlassInput({ className, ...props }: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        'w-full px-4 py-2.5 rounded-xl',
        'backdrop-blur-xl bg-white/5 dark:bg-black/10',
        'border border-white/20 focus:border-[#4191E1]/50',
        'outline-none transition-all duration-200',
        'placeholder:text-white/40',
        className,
      )}
      {...props}
    />
  );
}
```

### Dark Mode Patterns

```tsx
// ALWAYS include dark: variants
// Colors MUST use CSS custom properties var(--color-*) EXCEPT brand gradient
<div className={cn(
  'bg-[var(--color-surface)] text-[var(--color-text)]',
  'border border-[var(--color-border)]',
  'hover:bg-[var(--color-surface-hover)]',
)}>
```

---

## 12. Verification Gates

Before declaring ANY React work complete, pass ALL gates:

| Gate | Check | How |
|------|-------|-----|
| TypeScript | Zero type errors | `tsc --noEmit` |
| Light Theme | Visually correct | Run app, check manually |
| Dark Theme | Visually correct | Toggle theme, check manually |
| Error Boundary | Route wrapped | Verify in component tree |
| Keyboard Navigation | Tab order works | Tab through interactive elements |
| Loading States | Skeleton/spinner shown | Throttle network, verify |
| Empty States | Handles zero data | Pass empty array, verify |
| Error States | Handles failures | Force error, verify fallback |
| Responsive | Works at all widths | Resize viewport |
| Animations | Smooth, no jank | Check 60fps in DevTools |

---

## 13. Wrong vs Right Patterns

| Anti-Pattern | Why It's Wrong | Correct Pattern |
|-------------|---------------|-----------------|
| `useEffect(() => setDerived(...), [deps])` | Extra render cycle | Compute inline: `const derived = ...` |
| `<div onClick={() => fn(id)}>` on memo'd child | New function ref every render | `useCallback` + stable ref |
| `{list.map((item, i) => <X key={i} />)}` | Index keys break reconciliation | `key={item.id}` with stable IDs |
| `const store = useMyStore()` | Subscribes to entire store | `useMyStore(s => s.field)` selector |
| Prop drilling 3+ levels deep | Fragile, hard to refactor | Zustand store or Context |
| `style={{ margin: 10 }}` inline | New object every render | Tailwind class or extracted constant |
| `any` type on props | Bypasses TypeScript safety | Proper interface with generics |
| Missing error boundary on route | Crash takes down entire app | Wrap every route-level component |
| `useEffect` with no dependency array | Runs every render | Add deps or use event handler |
| Missing cleanup in useEffect | Memory leaks, stale state | Return cleanup function always |
| Direct state mutation: `state.push(item)` | React won't detect change | `setState([...state, item])` |
| Fetching without AbortController | Race conditions on nav | Always use AbortController cleanup |

---

## 14. Iron Law

```
NO REACT CODE WITHOUT PROPER COMPONENT BOUNDARIES AND ERROR HANDLING.

Every route-level component gets an ErrorBoundary.
Every async operation gets loading + error states.
Every list gets an empty state.
Every form gets validation.
Every theme has both light AND dark variants.
Every interactive element has hover + focus + active states.

This is the maintainer's standard. Premium work only.
```

---

## 15. Rationalization Prevention

When tempted to skip quality, check this table:

| Excuse | Reality | Do This Instead |
|--------|---------|-----------------|
| "It's just a quick component" | Quick becomes permanent | Add error boundary + types anyway |
| "I'll add types later" | You won't. Tech debt compounds. | Type it now, `tsc --noEmit` now |
| "Dark mode can wait" | the user tests both. Every time. | Add `dark:` variants immediately |
| "No one will hit this edge case" | the user customers will | Handle empty + error states |
| "useMemo everywhere for safety" | Over-memoization hurts readability | Profile first, memo only where needed |
| "useEffect is fine for this" | Is it derived state? Compute inline. | Ask: "Is this syncing with externals?" |
| "I'll skip the error boundary" | One crash = entire app down | Wrap it. 3 lines of JSX. |
| "Index keys are fine here" | Dynamic lists WILL break | Use stable unique IDs always |
| "I'll just use any" | TypeScript becomes useless | `unknown` + type guard |
| "The animation can be basic" | the user does premium work | Framer Motion with spring physics |

---

## 16. Compound Skill Chaining

This skill chains with the following fireworks skills for maximum coverage:

| Chain To | When | What It Adds |
|----------|------|-------------|
| `fireworks-test` | After implementing components | Testing strategies, RTL patterns, mock patterns |
| `fireworks-performance` | When optimizing renders | Bundle analysis, Lighthouse, Core Web Vitals |
| `fireworks-design` | For UI/UX decisions | Design system, spacing, color theory, accessibility |
| `fireworks-vscode` | For developer experience | Snippets, extensions, debugging configs |
| `fireworks-typescript` | For complex type patterns | Advanced generics, conditional types, mapped types |
| `fireworks-electron` | For Electron+React patterns | IPC typing, preload bridge, window management |

### Auto-Chain Rules

- Building a new component? Chain: `fireworks-react` + `fireworks-design` + `fireworks-test`
- Performance issue? Chain: `fireworks-react` + `fireworks-performance`
- Electron feature? Chain: `fireworks-react` + `fireworks-electron`
- Complex state? Chain: `fireworks-react` (Zustand section) + `fireworks-typescript`

---

## 17. Cross-References

| Resource | Path | Purpose |
|----------|------|---------|
| Base React Skill | `~/.claude/skills/react/SKILL.md` | Original patterns (absorbed into this skill) |
| Hooks Deep Dive | `./references/hooks-deep-dive.md` | Every hook + advanced custom hooks |
| Zustand Patterns | `./references/zustand-patterns.md` | Store architecture, middleware, testing |
| Performance React | `./references/performance-react.md` | Profiling, Suspense, streaming, optimization |
| Instructive Memory | `~/.claude/projects/*/memory/instructive-memory.md` | your project/MP3 project-specific React patterns |
| Enterprise Config | `~/.claude/projects/*/memory/enterprise-config.md` | Build tooling, Vite config |

---

## 18. Reference Files Index

| File | Lines | Coverage |
|------|-------|---------|
| [references/hooks-deep-dive.md](references/hooks-deep-dive.md) | ~300 | Every React hook, advanced patterns, custom hook library, rules enforcement, dependency array mastery |
| [references/zustand-patterns.md](references/zustand-patterns.md) | ~280 | Store architecture, slices pattern, persist/devtools/immer middleware, subscribeWithSelector, testing stores, migration patterns |
| [references/performance-react.md](references/performance-react.md) | ~280 | React DevTools profiling workflow, wasted render identification, Suspense for data fetching, streaming SSR, bundle optimization, Core Web Vitals |
