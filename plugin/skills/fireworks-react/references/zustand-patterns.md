# Zustand Patterns -- Enterprise State Management

> Store architecture, slices, persist/devtools/immer middleware, subscribeWithSelector, testing stores, and migration patterns for the maintainer's projects.

---

## 1. Store Architecture Principles

### One Store Per Domain

```
useAuthStore        -- user, tokens, login/logout
useInventoryStore   -- products, categories, filters, CRUD
useSyncStore        -- sync status, queue, conflict resolution
useSettingsStore    -- theme, sidebar, preferences (persisted)
useUIStore          -- modals, toasts, navigation state
```

### Store Anatomy

```tsx
import { create } from 'zustand';

interface StoreShape {
  // 1. State (data)
  items: Product[];
  loading: boolean;
  error: string | null;

  // 2. Computed (derived from state -- use selectors outside, not in store)

  // 3. Actions (functions that modify state)
  setItems: (items: Product[]) => void;
  addItem: (item: Product) => void;
  removeItem: (id: string) => void;
  fetchItems: () => Promise<void>;
  reset: () => void;
}

const initialState = {
  items: [],
  loading: false,
  error: null,
};

export const useInventoryStore = create<StoreShape>((set, get) => ({
  ...initialState,

  setItems: (items) => set({ items }),

  addItem: (item) => set((state) => ({
    items: [...state.items, item],
  })),

  removeItem: (id) => set((state) => ({
    items: state.items.filter(i => i.id !== id),
  })),

  fetchItems: async () => {
    if (get().loading) return;
    set({ loading: true, error: null });
    try {
      const items = await window.electronAPI.invoke('inventory:getAll');
      set({ items, loading: false });
    } catch (err) {
      set({ error: (err as Error).message, loading: false });
    }
  },

  reset: () => set(initialState),
}));
```

---

## 2. Selector Patterns (Performance-Critical)

### Basic Selectors

```tsx
// WRONG: subscribes to entire store
const { items, loading } = useInventoryStore();

// RIGHT: individual selectors -- re-render only when that slice changes
const items = useInventoryStore((s) => s.items);
const loading = useInventoryStore((s) => s.loading);

// RIGHT: action selectors are stable (never cause re-render)
const addItem = useInventoryStore((s) => s.addItem);
```

### Shallow Equality for Object Selectors

```tsx
import { useShallow } from 'zustand/react/shallow';

// Multiple values without individual selectors
const { items, loading, error } = useInventoryStore(
  useShallow((s) => ({
    items: s.items,
    loading: s.loading,
    error: s.error,
  }))
);
```

### Derived/Computed Selectors

```tsx
// Compute derived state in the selector -- NOT in the store
const activeProducts = useInventoryStore((s) =>
  s.items.filter(i => i.active)
);

// Memoized derived selector for expensive computations
const selectSortedProducts = (s: InventoryState) =>
  [...s.items].sort((a, b) => a.name.localeCompare(b.name));

// Use with useMemo to prevent re-computation
function ProductList() {
  const sortedProducts = useInventoryStore(selectSortedProducts);
  // This re-computes when items changes, but the selector is stable
  return <>{sortedProducts.map(p => <ProductRow key={p.id} product={p} />)}</>;
}

// For truly expensive derived state, combine with useMemo
function ExpensiveView() {
  const items = useInventoryStore((s) => s.items);
  const processed = useMemo(() => expensiveTransform(items), [items]);
  return <Chart data={processed} />;
}
```

---

## 3. Slices Pattern (Large Stores)

### Splitting a Store into Slices

```tsx
// types.ts
interface InventorySlice {
  items: Product[];
  addItem: (item: Product) => void;
  removeItem: (id: string) => void;
}

interface FilterSlice {
  searchQuery: string;
  selectedCategory: string | null;
  setSearchQuery: (q: string) => void;
  selectCategory: (c: string | null) => void;
}

interface SortSlice {
  sortBy: keyof Product;
  sortDir: 'asc' | 'desc';
  toggleSort: (column: keyof Product) => void;
}

type StoreState = InventorySlice & FilterSlice & SortSlice;

// slices/inventorySlice.ts
const createInventorySlice: StateCreator<StoreState, [], [], InventorySlice> = (set) => ({
  items: [],
  addItem: (item) => set((s) => ({ items: [...s.items, item] })),
  removeItem: (id) => set((s) => ({ items: s.items.filter(i => i.id !== id) })),
});

// slices/filterSlice.ts
const createFilterSlice: StateCreator<StoreState, [], [], FilterSlice> = (set) => ({
  searchQuery: '',
  selectedCategory: null,
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  selectCategory: (selectedCategory) => set({ selectedCategory }),
});

// slices/sortSlice.ts
const createSortSlice: StateCreator<StoreState, [], [], SortSlice> = (set) => ({
  sortBy: 'name' as keyof Product,
  sortDir: 'asc' as const,
  toggleSort: (column) => set((s) => ({
    sortBy: column,
    sortDir: s.sortBy === column && s.sortDir === 'asc' ? 'desc' : 'asc',
  })),
});

// store.ts -- combine slices
export const useStore = create<StoreState>()((...args) => ({
  ...createInventorySlice(...args),
  ...createFilterSlice(...args),
  ...createSortSlice(...args),
}));
```

---

## 4. Middleware Stack

### Persist Middleware

```tsx
import { persist, createJSONStorage } from 'zustand/middleware';

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: 'system' as const,
      sidebarWidth: 250,
      recentFiles: [] as string[],
      setTheme: (theme) => set({ theme }),
      setSidebarWidth: (w) => set({ sidebarWidth: w }),
      addRecentFile: (f) => set((s) => ({
        recentFiles: [f, ...s.recentFiles.filter(r => r !== f)].slice(0, 10),
      })),
    }),
    {
      name: 'settings-storage',
      storage: createJSONStorage(() => localStorage),
      // Only persist specific fields (not functions, not transient state)
      partialize: (state) => ({
        theme: state.theme,
        sidebarWidth: state.sidebarWidth,
        recentFiles: state.recentFiles,
      }),
      // Version for migrations
      version: 2,
      migrate: (persistedState, version) => {
        const state = persistedState as Record<string, unknown>;
        if (version === 0) {
          // v0 -> v1: added sidebarWidth
          state.sidebarWidth = 250;
        }
        if (version < 2) {
          // v1 -> v2: renamed theme values
          if (state.theme === 'auto') state.theme = 'system';
        }
        return state as SettingsState;
      },
    }
  )
);
```

### Electron Persist (Custom Storage)

```tsx
// For Electron apps -- persist to main process file system
const electronStorage = createJSONStorage(() => ({
  getItem: async (name: string): Promise<string | null> => {
    return window.electronAPI.invoke('store:get', name);
  },
  setItem: async (name: string, value: string): Promise<void> => {
    await window.electronAPI.invoke('store:set', name, value);
  },
  removeItem: async (name: string): Promise<void> => {
    await window.electronAPI.invoke('store:remove', name);
  },
}));
```

### DevTools Middleware

```tsx
import { devtools } from 'zustand/middleware';

export const useInventoryStore = create<InventoryState>()(
  devtools(
    (set) => ({
      items: [],
      addItem: (item) => set(
        (s) => ({ items: [...s.items, item] }),
        false,
        'inventory/addItem' // action name in DevTools
      ),
    }),
    { name: 'InventoryStore', enabled: import.meta.env.DEV }
  )
);
```

### Immer Middleware (Mutable-Style Updates)

```tsx
import { immer } from 'zustand/middleware/immer';

export const useInventoryStore = create<InventoryState>()(
  immer(
    (set) => ({
      items: [],
      categories: {},

      addItem: (item) => set((state) => {
        state.items.push(item); // mutate directly -- immer handles immutability
      }),

      updateItem: (id, updates) => set((state) => {
        const item = state.items.find(i => i.id === id);
        if (item) Object.assign(item, updates);
      }),

      removeItem: (id) => set((state) => {
        const index = state.items.findIndex(i => i.id === id);
        if (index !== -1) state.items.splice(index, 1);
      }),
    })
  )
);
```

### Combining Middleware

```tsx
// Order matters: outermost wrapper is first in the chain
export const useAppStore = create<AppState>()(
  devtools(           // outermost: enables DevTools
    persist(          // middle: persistence
      immer(          // innermost: immer mutations
        (set, get) => ({
          // store definition
        })
      ),
      { name: 'app-store' }
    ),
    { name: 'AppStore', enabled: import.meta.env.DEV }
  )
);
```

---

## 5. subscribeWithSelector

### Reacting to State Changes Outside React

```tsx
import { subscribeWithSelector } from 'zustand/middleware';

export const useSyncStore = create<SyncState>()(
  subscribeWithSelector(
    (set) => ({
      syncStatus: 'idle' as 'idle' | 'syncing' | 'error',
      lastSync: null as Date | null,
      // ...actions
    })
  )
);

// Subscribe to specific slice changes
const unsub = useSyncStore.subscribe(
  (state) => state.syncStatus,
  (status, prevStatus) => {
    if (status === 'error' && prevStatus === 'syncing') {
      showToast('Sync failed', 'error');
    }
    if (status === 'idle' && prevStatus === 'syncing') {
      showToast('Sync complete', 'success');
    }
  },
  { equalityFn: Object.is, fireImmediately: false }
);
```

### Connecting Stores (Cross-Store Communication)

```tsx
// When auth state changes, reset inventory
useAuthStore.subscribe(
  (state) => state.isAuthenticated,
  (isAuth) => {
    if (!isAuth) {
      useInventoryStore.getState().reset();
      useSyncStore.getState().reset();
    }
  }
);
```

---

## 6. Accessing Store Outside React

```tsx
// Get current state (snapshot)
const currentItems = useInventoryStore.getState().items;

// Call an action
useInventoryStore.getState().addItem(newProduct);

// Subscribe to changes (for non-React code like IPC handlers)
const unsubscribe = useInventoryStore.subscribe(
  (state) => console.log('Store changed:', state.items.length)
);

// Use in Electron IPC handlers
window.electronAPI.on('inventory:updated', (items: Product[]) => {
  useInventoryStore.getState().setItems(items);
});
```

---

## 7. Testing Zustand Stores

### Reset Store Between Tests

```tsx
// test-utils.ts
function resetAllStores() {
  useInventoryStore.setState(initialInventoryState);
  useSettingsStore.setState(initialSettingsState);
  useSyncStore.setState(initialSyncState);
}

beforeEach(() => {
  resetAllStores();
});
```

### Testing Store Logic Directly

```tsx
import { act } from '@testing-library/react';

describe('useInventoryStore', () => {
  beforeEach(() => {
    useInventoryStore.setState({ items: [], loading: false, error: null });
  });

  it('adds an item', () => {
    const product: Product = { id: '1', name: 'Test Vodka', price: 29.99 };
    act(() => {
      useInventoryStore.getState().addItem(product);
    });
    expect(useInventoryStore.getState().items).toHaveLength(1);
    expect(useInventoryStore.getState().items[0].name).toBe('Test Vodka');
  });

  it('removes an item', () => {
    useInventoryStore.setState({
      items: [
        { id: '1', name: 'Vodka', price: 29.99 },
        { id: '2', name: 'Whiskey', price: 39.99 },
      ],
    });
    act(() => {
      useInventoryStore.getState().removeItem('1');
    });
    expect(useInventoryStore.getState().items).toHaveLength(1);
    expect(useInventoryStore.getState().items[0].name).toBe('Whiskey');
  });

  it('handles fetch error', async () => {
    vi.spyOn(window.electronAPI, 'invoke').mockRejectedValueOnce(new Error('Network error'));
    await act(async () => {
      await useInventoryStore.getState().fetchItems();
    });
    expect(useInventoryStore.getState().error).toBe('Network error');
    expect(useInventoryStore.getState().loading).toBe(false);
  });
});
```

### Testing Components with Stores

```tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

describe('InventoryList', () => {
  beforeEach(() => {
    useInventoryStore.setState({
      items: [
        { id: '1', name: 'Vodka', price: 29.99 },
        { id: '2', name: 'Whiskey', price: 39.99 },
      ],
    });
  });

  it('renders items from store', () => {
    render(<InventoryList />);
    expect(screen.getByText('Vodka')).toBeInTheDocument();
    expect(screen.getByText('Whiskey')).toBeInTheDocument();
  });

  it('removes item on delete click', async () => {
    render(<InventoryList />);
    await userEvent.click(screen.getAllByRole('button', { name: /delete/i })[0]);
    expect(useInventoryStore.getState().items).toHaveLength(1);
  });
});
```

---

## 8. Migration Patterns

### Moving from useState to Zustand

```tsx
// BEFORE: local state scattered across components
function Inventory() {
  const [items, setItems] = useState<Product[]>([]);
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState<string | null>(null);
  // passed down as props...
}

// AFTER: centralized Zustand store
// 1. Create store with same shape
// 2. Replace useState with store selectors
// 3. Remove prop drilling
// 4. Components directly subscribe to what they need
function Inventory() {
  const items = useInventoryStore((s) => s.items);
  // search and category live in their own components now
}

function SearchBar() {
  const search = useInventoryStore((s) => s.searchQuery);
  const setSearch = useInventoryStore((s) => s.setSearchQuery);
  return <input value={search} onChange={e => setSearch(e.target.value)} />;
}
```

### Moving from Context to Zustand

```tsx
// BEFORE: Context with frequent updates causing cascading re-renders
const InventoryContext = createContext<InventoryContextType>(/* ... */);

// AFTER: Zustand with granular selectors
// 1. Move state + actions into Zustand store
// 2. Replace useContext calls with store selectors
// 3. Remove Provider wrapper (Zustand doesn't need one)
// 4. Benefit: only components using changed data re-render
```

---

## 9. Anti-Patterns to Avoid

| Anti-Pattern | Problem | Fix |
|-------------|---------|-----|
| `const store = useMyStore()` | Subscribes to entire store | Use selectors: `useMyStore(s => s.field)` |
| Storing derived state | Stale data, sync bugs | Compute in selectors |
| Huge monolithic store | Hard to reason about | Split into domain stores or slices |
| Async logic outside actions | Inconsistent state updates | Put async in store actions |
| Missing TypeScript types | Runtime errors | Full interface for store shape |
| Persisting everything | Storage bloat, stale functions | Use `partialize` to pick fields |
| No version on persist | Breaking changes crash app | Always version, always migrate |
| Mutating state directly | Store doesn't detect changes | Use immer middleware or spread |
