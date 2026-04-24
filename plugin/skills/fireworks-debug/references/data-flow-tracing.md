# Data Flow Tracing Methodology

> When data is wrong, missing, or corrupted, trace its path through every layer.
> Start at the symptom, trace BACKWARD to find where the data goes wrong.
> Then confirm with FORWARD tracing from the source.

---

## Core Methodology

### Rule 1: Start at the Symptom, Trace Backward
The symptom is where you NOTICE the problem, not where the problem IS. Trace backward from the symptom through each layer until you find the point where correct data becomes incorrect data.

### Rule 2: Trace One Piece of Data
Do not trace the entire application. Identify one specific piece of data that is wrong (e.g., "the product name shows 'undefined' instead of 'Wine'") and follow THAT data through every layer.

### Rule 3: Log at Every Boundary
Every time data crosses a boundary (component to component, renderer to main, handler to database), add a console.log showing the data at that exact point. The bug is between the last correct log and the first incorrect log.

---

## Layer-by-Layer Tracing

### Component Layer (React)

Where to look: The React component that displays the wrong data.

```typescript
// In the component, log the data it receives:
function ProductCard({ product }: { product: Product }) {
  console.log('[COMPONENT] ProductCard received:', product);
  console.log('[COMPONENT] product.name:', product?.name);
  // ...
}
```

Questions to ask:
- What props does this component receive? Are they correct?
- Where do the props come from? Parent component? Zustand store? React context?
- Is the component re-rendering when data changes? (Use React DevTools)
- Is there a transformation between the data source and the rendered output?

### State Layer (Zustand)

Where to look: The Zustand store that holds the data.

```typescript
// Log the store state:
const products = useProductStore((state) => {
  console.log('[STORE] Full state:', state);
  console.log('[STORE] Products:', state.products);
  return state.products;
});
```

Questions to ask:
- What action populated this state? When was it called?
- What data did the action receive as arguments?
- Does the action transform the data before storing it? Is the transformation correct?
- Is the selector returning the right slice of state?
- Is the equality function preventing necessary re-renders?

```typescript
// Log store actions:
const useProductStore = create<ProductStore>((set, get) => ({
  products: [],
  setProducts: (products) => {
    console.log('[STORE ACTION] setProducts called with:', products);
    set({ products });
  },
  addProduct: (product) => {
    console.log('[STORE ACTION] addProduct called with:', product);
    const current = get().products;
    console.log('[STORE ACTION] Current products before add:', current);
    set({ products: [...current, product] });
  },
}));
```

### IPC Layer (Electron)

The IPC boundary is the most common place for data to go wrong because data is serialized (cloned) when crossing the boundary.

**Renderer side (invoke):**
```typescript
// In the renderer, before the IPC call:
console.log('[RENDERER] Calling getProducts with args:', args);
const result = await window.api.getProducts(args);
console.log('[RENDERER] getProducts returned:', result);
```

**Preload bridge:**
```typescript
// In preload.ts:
getProducts: async (...args: any[]) => {
  console.log('[PRELOAD] getProducts called with:', args);
  const result = await ipcRenderer.invoke('get-products', ...args);
  console.log('[PRELOAD] getProducts result:', result);
  return result;
},
```

**Main process handler:**
```typescript
// In the main process handler:
ipcMain.handle('get-products', async (event, ...args) => {
  console.log('[MAIN] get-products handler called with:', args);
  const result = await db.getProducts(...args);
  console.log('[MAIN] get-products returning:', result);
  return result;
});
```

### Data Layer (Database / File / Network)

Where to look: The actual data source.

```typescript
// Log the raw query and results:
function getProducts(category: string): Product[] {
  const query = 'SELECT * FROM products WHERE category = ?';
  console.log('[DB] Executing query:', query, 'with params:', [category]);
  const stmt = db.prepare(query);
  stmt.bind([category]);
  const results: Product[] = [];
  while (stmt.step()) {
    const row = stmt.getAsObject();
    console.log('[DB] Row:', row);
    results.push(row as Product);
  }
  stmt.free();
  console.log('[DB] Total results:', results.length);
  return results;
}
```

Questions to ask:
- Is the query correct? Does it return the expected rows?
- Are the column names correct? (SQL is case-insensitive, JS is not.)
- Is the data in the database correct? (Check with a direct SQL query.)
- Is there a type mismatch between the database column type and the TypeScript type?

---

## Forward Tracing

After finding the bug with backward tracing, confirm your understanding with forward tracing:

1. **Start at the user action**: What does the user do? (click, type, navigate)
2. **Event handler**: What function handles the user action?
3. **Store action**: What store action is called? With what data?
4. **IPC call**: What IPC channel is invoked? With what arguments?
5. **Main handler**: What does the handler do? What does it return?
6. **Database/file**: What is read or written?
7. **Response path**: What data flows back through IPC -> store -> component?
8. **Render**: What does the component display?

At each step, the data should be correct. The step where it becomes incorrect is the bug location.

---

## Breakpoints: Where to Add console.log

Place logs at every boundary crossing. Here is the standard set:

| Location | Log Message Pattern | What to Check |
|----------|-------------------|---------------|
| Component render | `[COMPONENT] <Name> render, props:` | Props are correct |
| Store selector | `[STORE] <StoreName> selector returning:` | Selected state is correct |
| Store action | `[STORE ACTION] <actionName> called with:` | Action args are correct |
| Store mutation | `[STORE ACTION] <actionName> new state:` | State after mutation is correct |
| Renderer IPC call | `[RENDERER] invoking <channel> with:` | Args sent to IPC are correct |
| Preload bridge | `[PRELOAD] <method> forwarding:` | Data passes through unchanged |
| Main handler entry | `[MAIN] <channel> handler received:` | Main receives correct args |
| Database query | `[DB] query: <sql>, params: <params>` | Query is correct |
| Database result | `[DB] result: <data>` | Raw data is correct |
| Main handler exit | `[MAIN] <channel> handler returning:` | Return data is correct |
| Preload return | `[PRELOAD] <method> returning:` | Data passes through unchanged |
| Renderer IPC result | `[RENDERER] <channel> returned:` | Renderer receives correct data |

---

## Common Data Corruption Points

### Serialization Across IPC

The Electron IPC bridge uses the structured clone algorithm. Some types do NOT survive cloning:

| Type | Survives IPC? | What Happens |
|------|---------------|-------------|
| string, number, boolean | Yes | Cloned correctly |
| null, undefined | Yes | Cloned correctly |
| Array, plain Object | Yes | Deep cloned |
| Date | Yes | Cloned as Date object |
| Map, Set | Yes | Cloned correctly |
| Buffer | No | Becomes empty object. Use Uint8Array instead |
| BigInt | No | Throws DataCloneError. Convert to string |
| Function | No | Throws DataCloneError |
| Symbol | No | Throws DataCloneError |
| Class instance | Partially | Loses prototype, becomes plain object |
| Error | Partially | message preserved, stack may be lost |
| RegExp | Yes | Cloned correctly |
| Circular reference | No | Throws DataCloneError |

### SQL Query String Building

**Never concatenate strings to build SQL queries:**
```typescript
// DANGEROUS:
const query = `SELECT * FROM products WHERE name = '${userInput}'`;

// SAFE:
const query = 'SELECT * FROM products WHERE name = ?';
db.run(query, [userInput]);
```

### JSON.parse Without Validation

```typescript
// DANGEROUS:
const data = JSON.parse(rawString) as Product; // Crashes if rawString is invalid JSON

// SAFE:
let data: unknown;
try {
  data = JSON.parse(rawString);
} catch {
  console.error('Invalid JSON:', rawString);
  return null;
}
if (!isProduct(data)) {
  console.error('Invalid product data:', data);
  return null;
}
// Now data is safely typed as Product
```

### Number Precision
```typescript
// Floating point math:
0.1 + 0.2 === 0.3 // false! It equals 0.30000000000000004

// For financial calculations, use integers (cents):
const priceInCents = 2550; // $25.50
const total = priceInCents * quantity;
const displayPrice = (total / 100).toFixed(2); // "25.50"
```
