# Mocking Strategies — Deep Reference

> Part of the `fireworks-test` skill. See `../SKILL.md` for the master guide.

---

## vi.mock Patterns for Common Modules

### Mocking Electron (ipcRenderer)

```ts
// In test setup (tests/setup.ts) or at the top of a test file
vi.mock('electron', () => ({
  ipcRenderer: {
    invoke: vi.fn(),
    on: vi.fn(),
    once: vi.fn(),
    send: vi.fn(),
    removeListener: vi.fn(),
    removeAllListeners: vi.fn(),
  },
  ipcMain: {
    handle: vi.fn(),
    on: vi.fn(),
    removeHandler: vi.fn(),
  },
  BrowserWindow: vi.fn().mockImplementation(() => ({
    loadFile: vi.fn(),
    loadURL: vi.fn(),
    show: vi.fn(),
    hide: vi.fn(),
    close: vi.fn(),
    destroy: vi.fn(),
    isDestroyed: vi.fn().mockReturnValue(false),
    webContents: {
      send: vi.fn(),
      on: vi.fn(),
      once: vi.fn(),
      openDevTools: vi.fn(),
    },
    on: vi.fn(),
    once: vi.fn(),
  })),
  app: {
    getPath: vi.fn().mockReturnValue('/fake/user/data'),
    whenReady: vi.fn().mockResolvedValue(undefined),
    on: vi.fn(),
    quit: vi.fn(),
    isQuitting: false,
  },
  dialog: {
    showOpenDialog: vi.fn(),
    showSaveDialog: vi.fn(),
    showMessageBox: vi.fn(),
  },
}));
```

### Mocking Zustand Stores

#### Strategy 1: Partial Mock (Override Specific Values)

```ts
import { useProductStore } from '@/stores/productStore';

// Override specific state values for this test
beforeEach(() => {
  useProductStore.setState({
    products: [
      { id: '1', name: 'Test Vodka', price: 25.99, quantity: 10 },
      { id: '2', name: 'Test Whiskey', price: 34.99, quantity: 5 },
    ],
    loading: false,
    error: null,
  });
});

// The store's actions (addProduct, removeProduct, etc.) still work normally
// Only the state is overridden
```

#### Strategy 2: Full Mock (Replace Entire Store)

```ts
vi.mock('@/stores/productStore', () => ({
  useProductStore: vi.fn((selector) => {
    const mockState = {
      products: [],
      loading: false,
      error: null,
      addProduct: vi.fn(),
      removeProduct: vi.fn(),
      fetchProducts: vi.fn(),
    };
    return selector ? selector(mockState) : mockState;
  }),
}));
```

#### Strategy 3: Spy on Actions

```ts
import { useProductStore } from '@/stores/productStore';

it('should call addProduct when form is submitted', async () => {
  const addProductSpy = vi.fn();
  useProductStore.setState({ addProduct: addProductSpy });

  const user = userEvent.setup();
  render(<AddProductForm />);

  await user.type(screen.getByLabelText(/name/i), 'New Product');
  await user.type(screen.getByLabelText(/price/i), '29.99');
  await user.click(screen.getByRole('button', { name: /add/i }));

  expect(addProductSpy).toHaveBeenCalledWith(
    expect.objectContaining({ name: 'New Product', price: 29.99 })
  );
});
```

### Mocking sql.js Database

```ts
function createMockDatabase() {
  const mockStatements = new Map<string, unknown[][]>();

  const mockStmt = {
    bind: vi.fn().mockReturnThis(),
    step: vi.fn().mockReturnValue(false),
    getAsObject: vi.fn().mockReturnValue({}),
    get: vi.fn().mockReturnValue([]),
    free: vi.fn(),
    reset: vi.fn(),
  };

  const mockDb = {
    run: vi.fn(),
    exec: vi.fn().mockReturnValue([]),
    prepare: vi.fn().mockReturnValue(mockStmt),
    export: vi.fn().mockReturnValue(new Uint8Array()),
    close: vi.fn(),
    getRowsModified: vi.fn().mockReturnValue(0),
  };

  return { db: mockDb, stmt: mockStmt };
}

// Usage in tests
describe('ProductRepository', () => {
  const { db, stmt } = createMockDatabase();

  it('should query products', () => {
    db.exec.mockReturnValue([{
      columns: ['id', 'name', 'price', 'quantity'],
      values: [
        ['1', 'Hennessy', 39.99, 12],
        ['2', 'Grey Goose', 29.99, 8],
      ],
    }]);

    const products = getAllProducts(db);

    expect(db.exec).toHaveBeenCalledWith('SELECT * FROM products');
    expect(products).toHaveLength(2);
    expect(products[0].name).toBe('Hennessy');
  });

  it('should insert a product with prepared statement', () => {
    insertProduct(db, { name: 'New Product', price: 19.99, quantity: 20 });

    expect(db.prepare).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO products')
    );
    expect(stmt.bind).toHaveBeenCalledWith(['New Product', 19.99, 20]);
    expect(stmt.step).toHaveBeenCalled();
    expect(stmt.free).toHaveBeenCalled();
  });
});
```

### Mocking File System Operations

```ts
vi.mock('fs/promises', () => ({
  readFile: vi.fn(),
  writeFile: vi.fn(),
  mkdir: vi.fn(),
  rm: vi.fn(),
  access: vi.fn(),
  readdir: vi.fn(),
  stat: vi.fn(),
}));

import { readFile, writeFile } from 'fs/promises';

describe('ConfigService', () => {
  it('should read and parse config file', async () => {
    vi.mocked(readFile).mockResolvedValue(
      JSON.stringify({ theme: 'dark', language: 'en' })
    );

    const config = await loadConfig('/path/to/config.json');

    expect(readFile).toHaveBeenCalledWith('/path/to/config.json', 'utf-8');
    expect(config.theme).toBe('dark');
  });

  it('should return default config when file does not exist', async () => {
    vi.mocked(readFile).mockRejectedValue(
      Object.assign(new Error('ENOENT'), { code: 'ENOENT' })
    );

    const config = await loadConfig('/nonexistent/config.json');

    expect(config).toEqual(DEFAULT_CONFIG);
  });
});
```

### Mocking fetch / Network Calls

```ts
// Simple approach: mock global fetch
beforeEach(() => {
  global.fetch = vi.fn();
});

afterEach(() => {
  vi.restoreAllMocks();
});

it('should fetch products from API', async () => {
  vi.mocked(fetch).mockResolvedValue({
    ok: true,
    status: 200,
    json: vi.fn().mockResolvedValue([
      { id: '1', name: 'Product A' },
    ]),
  } as unknown as Response);

  const products = await fetchProducts();

  expect(fetch).toHaveBeenCalledWith('/api/products', expect.objectContaining({
    method: 'GET',
  }));
  expect(products).toHaveLength(1);
});

it('should throw on non-OK response', async () => {
  vi.mocked(fetch).mockResolvedValue({
    ok: false,
    status: 500,
    statusText: 'Internal Server Error',
  } as Response);

  await expect(fetchProducts()).rejects.toThrow('500');
});
```

---

## vi.fn() vs vi.spyOn() Decision Tree

```
Do you want to REPLACE the implementation?
  YES -> vi.fn()
    - Creates a standalone mock function
    - Use for: callback props, injected dependencies, factory functions
    - Example: const handleClick = vi.fn();
    - Example: vi.mock('electron', () => ({ ipcRenderer: { invoke: vi.fn() } }));

  NO -> Do you want to OBSERVE calls while keeping the original implementation?
    YES -> vi.spyOn(object, 'method')
      - Wraps the real method — still calls the original
      - Use for: verifying a method was called, logging
      - Example: const spy = vi.spyOn(console, 'error');
      - Remember: call spy.mockRestore() or use restoreAllMocks in afterEach

    MAYBE -> Do you want to OBSERVE calls but REPLACE the implementation?
      -> vi.spyOn(object, 'method').mockImplementation(...)
      - Best of both worlds: can restore the original later
      - Use for: temporarily replacing a method in one test
      - Example: vi.spyOn(Date, 'now').mockReturnValue(1700000000000);
```

### Quick Reference

| Method | Creates New? | Calls Original? | Restorable? | Use Case |
|--------|-------------|-----------------|-------------|----------|
| `vi.fn()` | Yes | No | N/A | Callbacks, injected deps |
| `vi.fn(impl)` | Yes | No (runs impl) | N/A | Custom mock implementation |
| `vi.spyOn(obj, 'method')` | No (wraps) | Yes | Yes | Observe real calls |
| `vi.spyOn(...).mockImplementation(impl)` | No (wraps) | No (runs impl) | Yes | Replace temporarily |
| `vi.spyOn(...).mockReturnValue(val)` | No (wraps) | No (returns val) | Yes | Stub return value |

---

## Mock Cleanup Patterns

### Standard Cleanup

```ts
beforeEach(() => {
  vi.clearAllMocks();    // Resets call counts and results, keeps implementation
});

afterEach(() => {
  vi.restoreAllMocks();  // Restores original implementations (for spyOn)
});
```

### When to Use Each

| Method | What It Does | When to Use |
|--------|-------------|-------------|
| `vi.clearAllMocks()` | Resets `.mock.calls`, `.mock.results`, `.mock.instances` | Between tests — fresh call history |
| `vi.resetAllMocks()` | clearAllMocks + removes mock implementation | When you need each test to set its own mock impl |
| `vi.restoreAllMocks()` | resetAllMocks + restores original (for spyOn) | When tests use spyOn and you want original behavior restored |

### Per-Mock Cleanup

```ts
const mockFn = vi.fn();
mockFn.mockClear();      // Clear call history
mockFn.mockReset();      // Clear + remove implementation
mockFn.mockRestore();    // Clear + remove implementation + restore original (spyOn only)
```

---

## Partial Mocking (Mock Only What You Need)

```ts
// Mock only specific exports from a module
vi.mock('@/utils/helpers', async () => {
  const actual = await vi.importActual<typeof import('@/utils/helpers')>('@/utils/helpers');
  return {
    ...actual,                          // Keep all real implementations
    generateId: vi.fn(() => 'mock-id'), // Override only this one
  };
});

// This is useful when:
// - Most of the module's functions work fine in tests
// - Only one function has side effects or external dependencies
// - You want to test that other functions call the mocked one
```

---

## Mock Implementation vs Mock Return Value

### mockReturnValue — When the Return is Simple

```ts
const getId = vi.fn().mockReturnValue('fixed-id');
// Always returns 'fixed-id', no matter what arguments

const getId = vi.fn()
  .mockReturnValueOnce('id-1')  // First call
  .mockReturnValueOnce('id-2')  // Second call
  .mockReturnValue('id-default'); // All subsequent calls
```

### mockResolvedValue — For Async Functions

```ts
const fetchData = vi.fn().mockResolvedValue({ products: [] });
// Returns Promise.resolve({ products: [] })

const fetchData = vi.fn().mockRejectedValue(new Error('Network error'));
// Returns Promise.reject(new Error('Network error'))
```

### mockImplementation — When Logic is Needed

```ts
// When the return depends on the input
const calculate = vi.fn().mockImplementation((price: number, qty: number) => {
  return price * qty;
});

// When you need to throw conditionally
const getProduct = vi.fn().mockImplementation((id: string) => {
  if (id === 'not-found') throw new Error('Product not found');
  return { id, name: 'Mock Product' };
});

// When you need side effects
let callCount = 0;
const trackCall = vi.fn().mockImplementation(() => {
  callCount++;
  return callCount;
});
```

---

## Common Pitfalls

### Pitfall 1: Mocking the Module You Are Testing

```ts
// WRONG: You are testing productStore but mocking it — you are testing the mock
vi.mock('@/stores/productStore');
import { useProductStore } from '@/stores/productStore';
// Tests pass but verify nothing about real store behavior

// RIGHT: Import the real module and test it directly
import { useProductStore } from '@/stores/productStore';
// Mock only its DEPENDENCIES, not the module itself
```

### Pitfall 2: Forgetting to Clear Mocks Between Tests

```ts
// Test 1 calls mockFn three times
// Test 2 checks mockFn.toHaveBeenCalledOnce() — FAILS because count is 4

// FIX: Always clear in beforeEach
beforeEach(() => {
  vi.clearAllMocks();
});
```

### Pitfall 3: Mock Hoisting Surprises

```ts
// vi.mock is HOISTED to the top of the file by Vitest
// This means it runs BEFORE any imports

// This works even though the mock is defined after the import
import { something } from './module';
vi.mock('./module', () => ({ something: vi.fn() }));

// But variables defined before vi.mock are NOT available inside it
const myValue = 42;
vi.mock('./module', () => ({
  something: vi.fn().mockReturnValue(myValue), // myValue is undefined here!
}));

// FIX: Use vi.hoisted() for values needed inside vi.mock()
const { myValue } = vi.hoisted(() => ({ myValue: 42 }));
vi.mock('./module', () => ({
  something: vi.fn().mockReturnValue(myValue), // Works
}));
```
