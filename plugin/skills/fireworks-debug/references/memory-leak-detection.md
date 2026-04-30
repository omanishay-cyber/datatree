# Memory Leak Detection Workflow

> Full workflow for detecting, isolating, and fixing memory leaks in Electron apps.
> Covers JavaScript heap leaks, native memory leaks, and Electron-specific patterns.

---

## Phase 1: Confirm the Leak

Before investigating, confirm that memory is actually leaking (growing without bound).

### Step 1: Establish Baseline
```typescript
// Add to main process for monitoring:
setInterval(() => {
  const mem = process.memoryUsage();
  console.log('[MEMORY]', {
    rss: `${(mem.rss / 1024 / 1024).toFixed(1)} MB`,
    heapUsed: `${(mem.heapUsed / 1024 / 1024).toFixed(1)} MB`,
    heapTotal: `${(mem.heapTotal / 1024 / 1024).toFixed(1)} MB`,
    external: `${(mem.external / 1024 / 1024).toFixed(1)} MB`,
  });
}, 10000); // Every 10 seconds
```

### Step 2: Force Garbage Collection
Before measuring, force GC to eliminate noise from uncollected garbage:
```typescript
// Launch with --expose-gc flag:
// electron --expose-gc .
// Then:
if (global.gc) {
  global.gc();
}
```

### Step 3: Measure Over Time
- Record memory at T=0 (app startup)
- Perform a repeatable user action 10 times
- Record memory at T=1
- Perform the same action 10 more times
- Record memory at T=2
- Force GC between measurements

**Leak Confirmed If**: Memory at T=2 is significantly higher than T=1, which is significantly higher than T=0, AND garbage collection does not reclaim it.

### Step 4: Quantify the Leak Rate
```
Leak rate = (T2 - T1) / number_of_actions
```
If each action leaks 0.5 MB, after 1000 actions you will leak 500 MB.

---

## Phase 2: Isolate the Leak Type

### JavaScript Heap Leak
- **DevTools Memory tab**: Take Heap Snapshot, look for growing retained sizes
- **Signature**: `heapUsed` grows, `heapTotal` grows
- **Cause**: JavaScript objects not being garbage collected

### Native Memory Leak
- **Task Manager / Activity Monitor**: RSS grows but heap stays flat
- **Signature**: `rss` grows, `heapUsed` stays stable
- **Cause**: Native modules, Buffers, or Electron internals leaking

### Electron-Specific Leak
- **Signature**: Memory grows when opening/closing windows, switching views, or navigating
- **Cause**: BrowserWindow not properly destroyed, IPC listeners accumulating, webContents not released

---

## Phase 3: Common Leak Patterns and Fixes

### Pattern 1: Event Listeners Not Removed

**Detection**: Event fires multiple times for a single action. Memory grows on mount/unmount cycles.

```typescript
// LEAKING:
useEffect(() => {
  window.addEventListener('resize', handleResize);
  // No cleanup! Listener accumulates on every mount
}, []);

// FIXED:
useEffect(() => {
  window.addEventListener('resize', handleResize);
  return () => window.removeEventListener('resize', handleResize);
}, []);
```

### Pattern 2: Detached DOM Nodes

**Detection**: DevTools Heap Snapshot > filter by "Detached" > look for detached HTMLDivElement trees.

**Cause**: A JavaScript variable still references a DOM node that was removed from the document.

```typescript
// LEAKING:
const nodeRef = useRef<HTMLDivElement>(null);
// If nodeRef.current is stored elsewhere and the component unmounts,
// the DOM node cannot be garbage collected

// FIXED: Clear refs in cleanup
useEffect(() => {
  return () => {
    nodeRef.current = null;
  };
}, []);
```

### Pattern 3: Closures Holding Large Objects

**Detection**: Heap snapshot shows large retained objects in closure scopes.

```typescript
// LEAKING:
function processData(largeDataset: Product[]) {
  const results = heavyComputation(largeDataset);

  // This closure captures largeDataset even though it only needs results
  return () => {
    displayResults(results);
  };
}

// FIXED: Release reference to large data
function processData(largeDataset: Product[]) {
  const results = heavyComputation(largeDataset);
  // largeDataset is not captured by the returned closure
  return () => {
    displayResults(results);
  };
}
// Ensure largeDataset goes out of scope after processData returns
```

### Pattern 4: IPC Listener Accumulation

**Detection**: `ipcRenderer.on` or `ipcMain.on` listeners pile up after window reloads or HMR.

```typescript
// LEAKING:
useEffect(() => {
  window.api.onUpdate((data) => {
    setProducts(data);
  });
  // No cleanup! Each HMR reload adds another listener
}, []);

// FIXED:
useEffect(() => {
  const unsubscribe = window.api.onUpdate((data) => {
    setProducts(data);
  });
  return () => unsubscribe();
}, []);
```

**In preload, expose cleanup:**
```typescript
contextBridge.exposeInMainWorld('api', {
  onUpdate: (callback: (data: Product[]) => void) => {
    ipcRenderer.on('products-updated', (_, data) => callback(data));
    return () => {
      ipcRenderer.removeAllListeners('products-updated');
    };
  },
});
```

### Pattern 5: setInterval Without clearInterval

**Detection**: Memory grows at a steady rate over time, even when the user is idle.

```typescript
// LEAKING:
useEffect(() => {
  setInterval(() => {
    fetchLatestData().then(setData);
  }, 5000);
  // No cleanup!
}, []);

// FIXED:
useEffect(() => {
  const id = setInterval(() => {
    fetchLatestData().then(setData);
  }, 5000);
  return () => clearInterval(id);
}, []);
```

### Pattern 6: Zustand Store Subscriptions

**Detection**: Components unmount but their subscriptions to the store remain active.

```typescript
// Zustand handles cleanup automatically when using hooks (useStore).
// But manual subscriptions MUST be cleaned up:

// LEAKING:
useEffect(() => {
  useProductStore.subscribe((state) => {
    console.log('Products changed:', state.products);
  });
}, []);

// FIXED:
useEffect(() => {
  const unsubscribe = useProductStore.subscribe((state) => {
    console.log('Products changed:', state.products);
  });
  return () => unsubscribe();
}, []);
```

### Pattern 7: BrowserWindow Not Destroyed

**Detection**: Opening and closing child windows causes memory to grow.

```typescript
// LEAKING:
function openChild() {
  const child = new BrowserWindow({ parent: mainWindow });
  child.loadFile('child.html');
  // child is never destroyed or nulled
}

// FIXED:
let childWindow: BrowserWindow | null = null;

function openChild() {
  if (childWindow && !childWindow.isDestroyed()) {
    childWindow.focus();
    return;
  }
  childWindow = new BrowserWindow({ parent: mainWindow });
  childWindow.loadFile('child.html');
  childWindow.on('closed', () => {
    childWindow = null; // Release reference
  });
}
```

### Pattern 8: Unbounded Cache Growth

**Detection**: A Map, Set, or object used as cache grows without limit.

```typescript
// LEAKING:
const cache = new Map<string, Product>();
function getProduct(id: string): Product {
  if (cache.has(id)) return cache.get(id)!;
  const product = fetchProduct(id);
  cache.set(id, product); // Cache grows forever
  return product;
}

// FIXED: Use LRU cache with size limit
class LRUCache<K, V> {
  private cache = new Map<K, V>();
  constructor(private maxSize: number) {}

  get(key: K): V | undefined {
    const value = this.cache.get(key);
    if (value !== undefined) {
      this.cache.delete(key);
      this.cache.set(key, value); // Move to end (most recent)
    }
    return value;
  }

  set(key: K, value: V): void {
    this.cache.delete(key);
    this.cache.set(key, value);
    if (this.cache.size > this.maxSize) {
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }
  }
}
```

---

## Phase 4: Ongoing Monitoring

Add memory monitoring that runs in development to catch leaks early:

```typescript
// In main process, add to dev mode only:
if (!app.isPackaged) {
  let lastHeapUsed = 0;
  setInterval(() => {
    if (global.gc) global.gc();
    const mem = process.memoryUsage();
    const heapMB = mem.heapUsed / 1024 / 1024;
    const delta = heapMB - lastHeapUsed;
    if (delta > 5) {
      console.warn(`[MEMORY WARNING] Heap grew by ${delta.toFixed(1)} MB in 30s`);
    }
    lastHeapUsed = heapMB;
  }, 30000);
}
```

### DevTools Memory Tab Workflow
1. Open DevTools > Memory tab
2. Take Heap Snapshot (baseline)
3. Perform the suspected leaking action 5 times
4. Take another Heap Snapshot
5. Select Snapshot 2, change view to "Comparison" with Snapshot 1
6. Sort by "Delta" column descending
7. The objects with the largest positive delta are the leaking objects
8. Click on them to see their retaining tree — this shows WHY they cannot be garbage collected
