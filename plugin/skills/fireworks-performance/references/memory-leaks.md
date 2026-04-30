# Memory Leak Detection and Prevention — Deep Reference

> Part of the `fireworks-performance` skill. See `../SKILL.md` for the master guide.

---

## Detection Tools

### Chrome DevTools Memory Tab

1. **Heap Snapshot** — Captures all objects in memory at a point in time.
   - Open DevTools -> Memory tab -> Select "Heap snapshot" -> Click "Take snapshot"
   - Use "Comparison" view between two snapshots to find growing objects
   - Sort by "Size Delta" to find what's accumulating

2. **Allocation Timeline** — Records allocations over time.
   - Select "Allocation instrumentation on timeline" -> Start recording
   - Perform the action you suspect leaks -> Stop recording
   - Blue bars = allocated and still alive (potential leaks)
   - Gray bars = allocated and garbage collected (normal)

3. **Allocation Sampling** — Lightweight profiling for production-like conditions.
   - Select "Allocation sampling" -> Start -> Use the app -> Stop
   - Shows which functions are allocating the most memory

### Heap Snapshot Comparison Workflow

```
1. Take Snapshot A (baseline — after page load, before interaction)
2. Perform the suspected leaking action 5 times
3. Force garbage collection (click the trash can icon in DevTools)
4. Take Snapshot B
5. Select Snapshot B -> Change view to "Comparison" -> Compare with Snapshot A
6. Sort by "Size Delta" descending
7. Look for objects that grew by exactly 5x the single action's allocation
   (If you did the action 5 times, leaked objects appear in multiples of 5)
```

### Quick Memory Check

```ts
// Log memory usage from renderer process
console.log('Memory:', JSON.stringify(process.memoryUsage(), null, 2));

// Periodic monitoring
setInterval(() => {
  const mem = process.memoryUsage();
  console.log(`Heap: ${(mem.heapUsed / 1024 / 1024).toFixed(1)}MB`);
}, 5000);

// If heapUsed grows steadily over time without plateauing, you have a leak
```

---

## Common Causes in Electron + React

### 1. Event Listeners Not Cleaned Up in useEffect

```tsx
// LEAK: listener accumulates on every mount/remount
useEffect(() => {
  window.addEventListener('resize', handleResize);
  // Missing cleanup!
}, []);

// FIXED: cleanup removes listener on unmount
useEffect(() => {
  window.addEventListener('resize', handleResize);
  return () => window.removeEventListener('resize', handleResize);
}, []);
```

### 2. Zustand Subscriptions Without Unsubscribe

```tsx
// LEAK: subscription persists after component unmounts
useEffect(() => {
  useAppStore.subscribe((state) => {
    console.log('Store changed:', state);
  });
}, []);

// FIXED: store unsubscribe function returned by cleanup
useEffect(() => {
  const unsubscribe = useAppStore.subscribe((state) => {
    console.log('Store changed:', state);
  });
  return unsubscribe;
}, []);
```

### 3. IPC Listeners Accumulating

```tsx
// LEAK: new listener added on every render/mount
useEffect(() => {
  window.electron.ipcRenderer.on('data-updated', handleUpdate);
  // Missing cleanup!
}, []);

// FIXED: remove specific listener on unmount
useEffect(() => {
  const handler = (_event: unknown, data: unknown) => handleUpdate(data);
  window.electron.ipcRenderer.on('data-updated', handler);
  return () => {
    window.electron.ipcRenderer.removeListener('data-updated', handler);
  };
}, []);
```

### 4. Closures Holding Old DOM References

```tsx
// LEAK: closure captures DOM element, prevents GC after removal
useEffect(() => {
  const element = document.getElementById('dynamic-content');
  const observer = new MutationObserver(() => {
    console.log('Changed:', element?.innerHTML); // element ref held in closure
  });
  if (element) observer.observe(element, { childList: true });
  return () => observer.disconnect();
}, []);
```

### 5. Timers Not Cleared

```tsx
// LEAK: interval runs forever after component unmounts
useEffect(() => {
  setInterval(() => {
    fetchLatestData();
  }, 5000);
}, []);

// FIXED: clear interval on unmount
useEffect(() => {
  const id = setInterval(() => {
    fetchLatestData();
  }, 5000);
  return () => clearInterval(id);
}, []);
```

### 6. Growing Arrays Without Limits

```tsx
// LEAK: history grows without bound
const [history, setHistory] = useState<Action[]>([]);

function addAction(action: Action) {
  setHistory(prev => [...prev, action]); // Grows forever
}

// FIXED: cap the history size
function addAction(action: Action) {
  setHistory(prev => {
    const next = [...prev, action];
    return next.length > 100 ? next.slice(-100) : next; // Keep last 100
  });
}
```

### 7. Forgotten AbortController

```tsx
// LEAK: fetch continues after unmount, callback updates unmounted component
useEffect(() => {
  fetch('/api/data')
    .then(r => r.json())
    .then(data => setData(data)); // May set state on unmounted component
}, []);

// FIXED: abort fetch on unmount
useEffect(() => {
  const controller = new AbortController();
  fetch('/api/data', { signal: controller.signal })
    .then(r => r.json())
    .then(data => setData(data))
    .catch(err => {
      if (err.name !== 'AbortError') throw err;
    });
  return () => controller.abort();
}, []);
```

---

## Fix Pattern: The Universal Cleanup Template

Every useEffect that sets up resources MUST return a cleanup function:

```tsx
useEffect(() => {
  // 1. Abort controller for fetch requests
  const controller = new AbortController();

  // 2. Event listeners
  const handleResize = () => { /* ... */ };
  window.addEventListener('resize', handleResize);

  // 3. Timers
  const intervalId = setInterval(() => { /* ... */ }, 1000);
  const timeoutId = setTimeout(() => { /* ... */ }, 5000);

  // 4. Store subscriptions
  const unsubscribe = useAppStore.subscribe((state) => { /* ... */ });

  // 5. IPC listeners
  const ipcHandler = (_e: unknown, data: unknown) => { /* ... */ };
  window.electron.ipcRenderer.on('channel', ipcHandler);

  // 6. Mutation/Intersection/Resize observers
  const observer = new IntersectionObserver(callback);
  if (ref.current) observer.observe(ref.current);

  // CLEANUP: Remove EVERYTHING
  return () => {
    controller.abort();
    window.removeEventListener('resize', handleResize);
    clearInterval(intervalId);
    clearTimeout(timeoutId);
    unsubscribe();
    window.electron.ipcRenderer.removeListener('channel', ipcHandler);
    observer.disconnect();
  };
}, []);
```

---

## Prevention Strategies

### ESLint Rules

```json
{
  "rules": {
    "react-hooks/exhaustive-deps": "warn",
    "no-restricted-globals": ["error", "setInterval", "setTimeout"]
  }
}
```

Restricting `setInterval`/`setTimeout` as globals forces developers to use wrapper hooks that handle cleanup automatically.

### Custom Hook: useSafeInterval

```tsx
function useSafeInterval(callback: () => void, delay: number | null) {
  const savedCallback = useRef(callback);

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    if (delay === null) return;
    const id = setInterval(() => savedCallback.current(), delay);
    return () => clearInterval(id);
  }, [delay]);
}
```

### Memory Budget Per Feature

Establish limits for memory-intensive features:

| Feature | Max Memory | Action if Exceeded |
|---------|------------|--------------------|
| Product cache | 50MB | Evict oldest entries |
| Undo history | 20 actions | Drop oldest action |
| Search results | 500 items | Paginate, don't load all |
| Image thumbnails | 100MB | LRU cache with eviction |
| Log buffer | 1000 entries | Ring buffer |

### Code Review Checklist for Memory

- [ ] Every `useEffect` with setup returns a cleanup function
- [ ] Every `addEventListener` has a matching `removeEventListener`
- [ ] Every `setInterval` has a matching `clearInterval`
- [ ] Every `ipcRenderer.on` has a matching `removeListener`
- [ ] Every `.subscribe()` stores and calls its unsubscribe function
- [ ] Arrays and maps have growth limits
- [ ] Fetch requests use AbortController
- [ ] No synchronous heavy operations that could cause GC pressure
