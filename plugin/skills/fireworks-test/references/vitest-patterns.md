# Vitest Patterns — Deep Reference

> Part of the `fireworks-test` skill. See `../SKILL.md` for the master guide.

---

## Vitest Configuration for Electron + React 18 + TypeScript

### vitest.config.ts

```ts
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src/renderer'),
    },
  },
  test: {
    // Use jsdom for React component testing
    environment: 'jsdom',

    // Global test utilities — no need to import describe/it/expect in every file
    globals: true,

    // Setup files run before each test file
    setupFiles: ['./tests/setup.ts'],

    // Include patterns
    include: ['src/**/*.test.{ts,tsx}', 'tests/**/*.test.{ts,tsx}'],

    // Exclude patterns
    exclude: ['node_modules', 'dist', 'tests/e2e/**'],

    // TypeScript configuration
    typecheck: {
      enabled: true,
      tsconfig: './tsconfig.json',
    },

    // Coverage configuration
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/**/*.d.ts',
        'src/main/index.ts',       // Electron entry — hard to unit test
        'src/preload/index.ts',     // Preload bridge — tested via integration
      ],
      thresholds: {
        branches: 70,
        functions: 70,
        lines: 75,
        statements: 75,
      },
    },

    // Retry flaky tests (use sparingly — fix the test instead)
    // retry: 0,

    // Pool configuration
    pool: 'forks',               // 'forks' is more stable than 'threads' for jsdom
    poolOptions: {
      forks: {
        singleFork: false,       // Parallel execution
      },
    },
  },
});
```

### tests/setup.ts

```ts
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

// Automatic cleanup after each test (unmount rendered components)
afterEach(() => {
  cleanup();
});

// Mock Electron's ipcRenderer globally
vi.mock('electron', () => ({
  ipcRenderer: {
    invoke: vi.fn(),
    on: vi.fn(),
    once: vi.fn(),
    send: vi.fn(),
    removeListener: vi.fn(),
    removeAllListeners: vi.fn(),
  },
}));

// Mock window.api (exposed via contextBridge in preload)
Object.defineProperty(window, 'api', {
  value: {
    invoke: vi.fn(),
    onDataUpdated: vi.fn().mockReturnValue(vi.fn()), // Returns cleanup function
  },
  writable: true,
});

// Mock matchMedia for components that use it
Object.defineProperty(window, 'matchMedia', {
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
  writable: true,
});

// Mock IntersectionObserver
class MockIntersectionObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
Object.defineProperty(window, 'IntersectionObserver', {
  value: MockIntersectionObserver,
  writable: true,
});

// Mock ResizeObserver
class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
Object.defineProperty(window, 'ResizeObserver', {
  value: MockResizeObserver,
  writable: true,
});
```

---

## Testing React Components with @testing-library/react

### Basic Component Test

```tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import { ProductCard } from './ProductCard';

const mockProduct = {
  id: 'prod-1',
  name: 'Johnnie Walker Black',
  price: 34.99,
  quantity: 24,
  category: 'Whiskey',
};

describe('ProductCard', () => {
  it('should display product name and price', () => {
    render(<ProductCard product={mockProduct} />);

    expect(screen.getByText('Johnnie Walker Black')).toBeInTheDocument();
    expect(screen.getByText('$34.99')).toBeInTheDocument();
  });

  it('should call onEdit when edit button is clicked', async () => {
    const user = userEvent.setup();
    const handleEdit = vi.fn();

    render(<ProductCard product={mockProduct} onEdit={handleEdit} />);

    await user.click(screen.getByRole('button', { name: /edit/i }));

    expect(handleEdit).toHaveBeenCalledOnce();
    expect(handleEdit).toHaveBeenCalledWith(mockProduct.id);
  });

  it('should show low stock warning when quantity is below threshold', () => {
    const lowStockProduct = { ...mockProduct, quantity: 2 };
    render(<ProductCard product={lowStockProduct} lowStockThreshold={5} />);

    expect(screen.getByText(/low stock/i)).toBeInTheDocument();
  });

  it('should NOT show low stock warning when quantity is above threshold', () => {
    render(<ProductCard product={mockProduct} lowStockThreshold={5} />);

    expect(screen.queryByText(/low stock/i)).not.toBeInTheDocument();
  });
});
```

### Testing with Providers (Router, Store, Theme)

```tsx
import { render, RenderOptions } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { ReactElement } from 'react';

// Custom render function that wraps components with necessary providers
function renderWithProviders(
  ui: ReactElement,
  options?: RenderOptions & { route?: string }
) {
  const { route = '/', ...renderOptions } = options ?? {};

  return render(
    <MemoryRouter initialEntries={[route]}>
      {ui}
    </MemoryRouter>,
    renderOptions
  );
}

// Usage
describe('Navigation', () => {
  it('should navigate to reports page', async () => {
    const user = userEvent.setup();
    renderWithProviders(<App />, { route: '/' });

    await user.click(screen.getByRole('link', { name: /reports/i }));

    expect(screen.getByText(/sales report/i)).toBeInTheDocument();
  });
});
```

### Testing Async Operations

```tsx
import { render, screen, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';

describe('ProductList', () => {
  beforeEach(() => {
    vi.mocked(window.api.invoke).mockResolvedValue([
      { id: '1', name: 'Hennessy VS', price: 39.99 },
      { id: '2', name: 'Grey Goose', price: 29.99 },
    ]);
  });

  it('should display loading state then products', async () => {
    render(<ProductList />);

    // Initially shows loading
    expect(screen.getByText(/loading/i)).toBeInTheDocument();

    // After data loads, shows products
    await waitFor(() => {
      expect(screen.getByText('Hennessy VS')).toBeInTheDocument();
    });
    expect(screen.getByText('Grey Goose')).toBeInTheDocument();

    // Loading indicator should be gone
    expect(screen.queryByText(/loading/i)).not.toBeInTheDocument();
  });

  it('should display error message when fetch fails', async () => {
    vi.mocked(window.api.invoke).mockRejectedValue(new Error('Database error'));

    render(<ProductList />);

    await waitFor(() => {
      expect(screen.getByText(/error/i)).toBeInTheDocument();
    });
  });
});
```

---

## Testing Zustand Stores

### Direct Store Testing (Preferred)

Test the store logic directly without rendering components. This is faster and more focused.

```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { useProductStore } from './productStore';

describe('productStore', () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useProductStore.setState({
      products: [],
      loading: false,
      error: null,
      searchQuery: '',
    });
  });

  it('should add a product', () => {
    const product = { id: '1', name: 'Test Product', price: 10, quantity: 5 };

    useProductStore.getState().addProduct(product);

    expect(useProductStore.getState().products).toHaveLength(1);
    expect(useProductStore.getState().products[0]).toEqual(product);
  });

  it('should remove a product by id', () => {
    useProductStore.setState({
      products: [
        { id: '1', name: 'Product A', price: 10, quantity: 5 },
        { id: '2', name: 'Product B', price: 20, quantity: 10 },
      ],
    });

    useProductStore.getState().removeProduct('1');

    expect(useProductStore.getState().products).toHaveLength(1);
    expect(useProductStore.getState().products[0].id).toBe('2');
  });

  it('should update product quantity', () => {
    useProductStore.setState({
      products: [{ id: '1', name: 'Product A', price: 10, quantity: 5 }],
    });

    useProductStore.getState().updateQuantity('1', 15);

    expect(useProductStore.getState().products[0].quantity).toBe(15);
  });

  it('should filter products by search query', () => {
    useProductStore.setState({
      products: [
        { id: '1', name: 'Johnnie Walker', price: 35, quantity: 10 },
        { id: '2', name: 'Jack Daniels', price: 28, quantity: 8 },
        { id: '3', name: 'Grey Goose', price: 30, quantity: 12 },
      ],
    });

    useProductStore.getState().setSearchQuery('john');

    const filtered = useProductStore.getState().filteredProducts;
    expect(filtered).toHaveLength(1);
    expect(filtered[0].name).toBe('Johnnie Walker');
  });

  it('should calculate total inventory value', () => {
    useProductStore.setState({
      products: [
        { id: '1', name: 'A', price: 10, quantity: 5 },   // $50
        { id: '2', name: 'B', price: 20, quantity: 3 },   // $60
      ],
    });

    expect(useProductStore.getState().totalInventoryValue).toBe(110);
  });
});
```

### Testing Store with Async Actions

```ts
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useProductStore } from './productStore';

describe('productStore async actions', () => {
  beforeEach(() => {
    useProductStore.setState({ products: [], loading: false, error: null });
    vi.clearAllMocks();
  });

  it('should set loading true while fetching, then populate products', async () => {
    vi.mocked(window.api.invoke).mockResolvedValue([
      { id: '1', name: 'Test', price: 10, quantity: 5 },
    ]);

    const fetchPromise = useProductStore.getState().fetchProducts();

    // Should be loading immediately after calling fetch
    expect(useProductStore.getState().loading).toBe(true);

    await fetchPromise;

    // After fetch completes
    expect(useProductStore.getState().loading).toBe(false);
    expect(useProductStore.getState().products).toHaveLength(1);
    expect(useProductStore.getState().error).toBeNull();
  });

  it('should set error when fetch fails', async () => {
    vi.mocked(window.api.invoke).mockRejectedValue(new Error('DB connection failed'));

    await useProductStore.getState().fetchProducts();

    expect(useProductStore.getState().loading).toBe(false);
    expect(useProductStore.getState().error).toBe('DB connection failed');
    expect(useProductStore.getState().products).toEqual([]);
  });
});
```

---

## Testing IPC Handlers (Main Process)

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock electron before importing the handler
vi.mock('electron', () => ({
  ipcMain: {
    handle: vi.fn(),
  },
  app: {
    getPath: vi.fn().mockReturnValue('/fake/path'),
  },
}));

import { ipcMain } from 'electron';
import { registerProductHandlers } from './productHandlers';

describe('Product IPC Handlers', () => {
  let handlers: Record<string, (...args: unknown[]) => unknown>;

  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};

    // Capture registered handlers
    vi.mocked(ipcMain.handle).mockImplementation((channel, handler) => {
      handlers[channel] = handler;
      return undefined as unknown as Electron.IpcMain;
    });

    registerProductHandlers();
  });

  it('should register db:get-products handler', () => {
    expect(handlers['db:get-products']).toBeDefined();
  });

  it('should return products from database', async () => {
    // Mock the database layer
    const mockDb = {
      exec: vi.fn().mockReturnValue([{
        columns: ['id', 'name', 'price'],
        values: [['1', 'Test Product', 29.99]],
      }]),
    };

    // Inject mock db (depends on your architecture)
    const result = await handlers['db:get-products']({} as Electron.IpcMainInvokeEvent, mockDb);

    expect(result).toEqual([{ id: '1', name: 'Test Product', price: 29.99 }]);
  });
});
```

---

## Snapshot Testing

### When to Use Snapshots

- UI components with complex rendered output where you want to catch unexpected changes
- Serialized data structures (API responses, database query results)
- Configuration objects

### When NOT to Use Snapshots

- Logic that returns simple values (use `toBe`/`toEqual` instead)
- Rapidly changing UI during development (snapshots will always be outdated)
- Anything where the snapshot is so large you cannot meaningfully review changes

```tsx
it('should match the rendered snapshot', () => {
  const { container } = render(<ProductCard product={mockProduct} />);
  expect(container.firstChild).toMatchSnapshot();
});

// Inline snapshots are better — the expected value is right in the test
it('should format the price correctly', () => {
  expect(formatPrice(1234.5)).toMatchInlineSnapshot('"$1,234.50"');
});
```

---

## Custom Matchers

```ts
// tests/matchers.ts
import { expect } from 'vitest';

expect.extend({
  toBeValidPrice(received: unknown) {
    const pass = typeof received === 'number' && received >= 0 && isFinite(received);
    return {
      pass,
      message: () =>
        `expected ${received} ${pass ? 'not ' : ''}to be a valid price (non-negative finite number)`,
    };
  },

  toBeWithinRange(received: number, floor: number, ceiling: number) {
    const pass = received >= floor && received <= ceiling;
    return {
      pass,
      message: () =>
        `expected ${received} ${pass ? 'not ' : ''}to be within range ${floor} - ${ceiling}`,
    };
  },
});

// Type augmentation
declare module 'vitest' {
  interface Assertion<T> {
    toBeValidPrice(): T;
    toBeWithinRange(floor: number, ceiling: number): T;
  }
  interface AsymmetricMatchersContaining {
    toBeValidPrice(): unknown;
    toBeWithinRange(floor: number, ceiling: number): unknown;
  }
}

// Usage
it('should return a valid price', () => {
  expect(calculateTotal(items)).toBeValidPrice();
});

it('should calculate tax within expected range', () => {
  expect(calculateTax(100)).toBeWithinRange(6, 10); // NJ tax range
});
```

---

## Fake Timers

```ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

describe('debounced search', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('should debounce search input by 300ms', async () => {
    const searchFn = vi.fn();
    const debouncedSearch = debounce(searchFn, 300);

    debouncedSearch('a');
    debouncedSearch('ab');
    debouncedSearch('abc');

    // Nothing called yet — still within 300ms window
    expect(searchFn).not.toHaveBeenCalled();

    // Advance time by 300ms
    vi.advanceTimersByTime(300);

    // Only the last call should have fired
    expect(searchFn).toHaveBeenCalledOnce();
    expect(searchFn).toHaveBeenCalledWith('abc');
  });
});
```
