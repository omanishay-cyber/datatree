# State Management — Zustand Architecture Patterns

## Overview

Zustand is the state management library for all React-based Electron applications. It provides a minimal, unopinionated API that works naturally with React's rendering model and Electron's process isolation.

---

## Store Design

### Standard Store Structure

Every Zustand store follows this pattern: state properties, async actions that communicate with the main process via IPC, and synchronous actions for UI-only state.

```typescript
import { create } from 'zustand';

interface ProductStore {
  // State
  products: Product[];
  loading: boolean;
  error: string | null;

  // Actions
  fetchProducts: () => Promise<void>;
  addProduct: (product: Omit<Product, 'id'>) => Promise<void>;
  updateProduct: (id: string, data: Partial<Product>) => Promise<void>;
  deleteProduct: (id: string) => Promise<void>;

  // Derived (via selectors outside store)
}

export const useProductStore = create<ProductStore>((set, get) => ({
  // Initial state
  products: [],
  loading: false,
  error: null,

  // Actions
  fetchProducts: async () => {
    set({ loading: true, error: null });
    try {
      const products = await window.api.invoke('db:query', {
        table: 'products',
      });
      set({ products, loading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch',
        loading: false,
      });
    }
  },

  addProduct: async (product) => {
    set({ loading: true, error: null });
    try {
      const result = await window.api.invoke('db:insert', {
        table: 'products',
        data: product,
      });
      const newProduct = { ...product, id: result.id } as Product;
      set((state) => ({
        products: [...state.products, newProduct],
        loading: false,
      }));
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to add',
        loading: false,
      });
    }
  },

  updateProduct: async (id, data) => {
    try {
      await window.api.invoke('db:update', {
        table: 'products',
        id,
        data,
      });
      set((state) => ({
        products: state.products.map((p) =>
          p.id === id ? { ...p, ...data } : p
        ),
      }));
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to update',
      });
    }
  },

  deleteProduct: async (id) => {
    try {
      await window.api.invoke('db:delete', {
        table: 'products',
        id,
      });
      set((state) => ({
        products: state.products.filter((p) => p.id !== id),
      }));
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete',
      });
    }
  },
}));
```

### Selectors (Outside Store, Memoized)

Selectors are defined outside the store to enable memoization and prevent unnecessary re-renders:

```typescript
// Selectors — defined outside the store
export const selectActiveProducts = (state: ProductStore) =>
  state.products.filter((p) => p.active);

export const selectProductById = (id: string) => (state: ProductStore) =>
  state.products.find((p) => p.id === id);

export const selectProductCount = (state: ProductStore) =>
  state.products.length;

export const selectLowStockProducts = (state: ProductStore) =>
  state.products.filter((p) => p.quantity <= p.reorderPoint);
```

### Usage in Components

```typescript
// Good: subscribe to specific slice
const products = useProductStore(selectActiveProducts);
const loading = useProductStore((s) => s.loading);
const fetchProducts = useProductStore((s) => s.fetchProducts);

// Bad: subscribe to entire store (causes re-renders on any change)
const store = useProductStore(); // AVOID THIS
```

---

## Middleware

### Persist Middleware

For persisting state across app restarts:

```typescript
import { persist, createJSONStorage } from 'zustand/middleware';

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      theme: 'system',
      language: 'en',
      sidebarOpen: true,
      setTheme: (theme) => set({ theme }),
      setLanguage: (language) => set({ language }),
      toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
    }),
    {
      name: 'settings-storage',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        theme: state.theme,
        language: state.language,
        sidebarOpen: state.sidebarOpen,
      }),
    }
  )
);
```

### Devtools Middleware (Development Only)

```typescript
import { devtools } from 'zustand/middleware';

export const useProductStore = create<ProductStore>()(
  devtools(
    (set, get) => ({
      // ... store definition
    }),
    { name: 'ProductStore', enabled: import.meta.env.DEV }
  )
);
```

### Immer Middleware (Complex Nested Updates)

For deeply nested state that is awkward to update immutably:

```typescript
import { immer } from 'zustand/middleware/immer';

export const useInvoiceStore = create<InvoiceStore>()(
  immer((set) => ({
    invoices: [],
    updateLineItem: (invoiceId, lineIdx, data) =>
      set((state) => {
        const invoice = state.invoices.find((i) => i.id === invoiceId);
        if (invoice) {
          Object.assign(invoice.lineItems[lineIdx], data);
        }
      }),
  }))
);
```

---

## Performance Patterns

### Use Selectors to Prevent Re-Renders

```typescript
// Component only re-renders when 'loading' changes
const loading = useProductStore((s) => s.loading);

// Component only re-renders when the filtered list changes
const activeProducts = useProductStore(
  (s) => s.products.filter((p) => p.active),
  shallow // Use shallow equality for array/object comparisons
);
```

### Shallow Equality for Object Comparisons

```typescript
import { useShallow } from 'zustand/react/shallow';

// Re-renders only when either products or loading changes
const { products, loading } = useProductStore(
  useShallow((s) => ({ products: s.products, loading: s.loading }))
);
```

### Transient Updates (No Re-Render)

For high-frequency updates that should not trigger re-renders:

```typescript
// Access state outside React
const currentProducts = useProductStore.getState().products;

// Subscribe outside React
const unsubscribe = useProductStore.subscribe(
  (state) => state.products,
  (products) => {
    console.log('Products changed:', products.length);
  }
);
```

---

## Testing Patterns

### Setup with setState

```typescript
beforeEach(() => {
  useProductStore.setState({
    products: [
      { id: '1', name: 'Test Product', active: true, quantity: 10 },
      { id: '2', name: 'Inactive', active: false, quantity: 0 },
    ],
    loading: false,
    error: null,
  });
});
```

### Assert with getState

```typescript
test('deleteProduct removes product from store', async () => {
  await useProductStore.getState().deleteProduct('1');
  const products = useProductStore.getState().products;
  expect(products).toHaveLength(1);
  expect(products[0].id).toBe('2');
});
```

---

## Anti-Patterns

### Do NOT put derived state in the store

```typescript
// BAD: computed value stored in state
interface BadStore {
  products: Product[];
  activeCount: number; // This is derived!
}

// GOOD: use a selector
const selectActiveCount = (s: ProductStore) =>
  s.products.filter((p) => p.active).length;
```

### Do NOT use store for ephemeral UI state

```typescript
// BAD: modal open state in global store
const useStore = create(() => ({
  isDeleteModalOpen: false, // This is local UI state
}));

// GOOD: use React's useState
const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
```

### Do NOT call store actions during render

```typescript
// BAD: side effect in render
function ProductList() {
  useProductStore.getState().fetchProducts(); // Called every render!
  // ...
}

// GOOD: use useEffect
function ProductList() {
  const fetchProducts = useProductStore((s) => s.fetchProducts);
  useEffect(() => {
    fetchProducts();
  }, [fetchProducts]);
  // ...
}
```

### Do NOT prop-drill more than 2 levels

If a value is needed more than 2 components deep, put it in a Zustand store or use React context. Prop drilling beyond 2 levels creates fragile, hard-to-refactor component trees.

---

## Store Organization

### File Structure

```
src/renderer/stores/
  |-- useProductStore.ts      — Products domain
  |-- useInvoiceStore.ts      — Invoices domain
  |-- useAuthStore.ts         — Authentication
  |-- useSettingsStore.ts     — App settings (persisted)
  |-- useUIStore.ts           — Shared UI state (sidebar, theme)
  +-- index.ts                — Re-exports all stores
```

### One Store Per Domain

Each business domain gets its own store. Do not create a single "god store" that holds everything. Separate stores:
- Reduce re-render scope (changes in products do not re-render invoice components)
- Improve code organization (each store file is self-contained)
- Enable parallel development (two developers can work on different stores)
