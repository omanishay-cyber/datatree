# Electron Performance — Deep Reference

> Part of the `fireworks-performance` skill. See `../SKILL.md` for the master guide.

---

## Startup Time

### Measuring Startup

```ts
// In main.ts — measure the critical path
const appStart = Date.now();

app.on('ready', () => {
  console.log(`app.ready: ${Date.now() - appStart}ms`);
});

// In renderer — measure time to first paint
window.addEventListener('DOMContentLoaded', () => {
  const timing = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming;
  console.log(`DOM ready: ${timing.domContentLoadedEventEnd}ms`);
  console.log(`First paint: ${performance.getEntriesByName('first-paint')[0]?.startTime}ms`);
});
```

### Defer Non-Critical Work

```ts
// main.ts — FAST startup pattern
async function main() {
  // CRITICAL PATH — do these first
  const mainWindow = createWindow();
  mainWindow.loadFile('index.html');

  // DEFERRED — do these after window is visible
  mainWindow.webContents.once('did-finish-load', async () => {
    // Now the user sees the app — do background work
    await initDatabase();
    await checkForUpdates();
    await loadUserPreferences();
    await registerGlobalShortcuts();
  });
}

app.whenReady().then(main);
```

### Splash Screen Pattern

```ts
// Show a lightweight splash window instantly
function createSplashWindow() {
  const splash = new BrowserWindow({
    width: 400,
    height: 300,
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    webPreferences: { nodeIntegration: false },
  });
  splash.loadFile('splash.html'); // Simple HTML with logo and spinner
  return splash;
}

async function main() {
  const splash = createSplashWindow();

  // Do all heavy initialization while splash is showing
  await initDatabase();
  await loadConfiguration();

  // Create the main window
  const mainWindow = createMainWindow();
  await mainWindow.loadFile('index.html');

  // Swap windows
  mainWindow.show();
  splash.destroy();
}
```

---

## Main Process CPU Optimization

### Profiling with --inspect

```bash
# Start Electron with inspector
electron --inspect=9229 .

# Or in package.json
"scripts": {
  "dev:debug": "electron --inspect=9229 ."
}

# Then open chrome://inspect in Chrome to connect the profiler
```

### Avoid Synchronous Operations

```ts
// BAD: Blocks the main process (and ALL windows)
const data = fs.readFileSync('large-file.json', 'utf-8');
const config = JSON.parse(data);

// GOOD: Non-blocking
const data = await fs.promises.readFile('large-file.json', 'utf-8');
const config = JSON.parse(data);

// BAD: Synchronous dialog blocks everything
const result = dialog.showMessageBoxSync(mainWindow, { /* ... */ });

// GOOD: Async dialog
const result = await dialog.showMessageBox(mainWindow, { /* ... */ });
```

### Offload to Worker Threads

```ts
// For CPU-intensive work (encryption, image processing, large data transforms)
import { Worker } from 'worker_threads';

function runHeavyTask(data: unknown): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const worker = new Worker('./workers/heavy-task.js', {
      workerData: data,
    });
    worker.on('message', resolve);
    worker.on('error', reject);
  });
}

// worker file: workers/heavy-task.js
const { workerData, parentPort } = require('worker_threads');
const result = processData(workerData); // CPU-intensive work
parentPort.postMessage(result);
```

---

## IPC Optimization

### Batch Related Calls

```ts
// BAD: Multiple round-trips
const products = await invoke('db:get-products');
const categories = await invoke('db:get-categories');
const settings = await invoke('db:get-settings');

// GOOD: Single round-trip
const { products, categories, settings } = await invoke('db:get-initial-data');

// Implementation in main process
ipcMain.handle('db:get-initial-data', async () => {
  const [products, categories, settings] = await Promise.all([
    db.getProducts(),
    db.getCategories(),
    db.getSettings(),
  ]);
  return { products, categories, settings };
});
```

### Debounce Frequent Updates

```ts
// BAD: Sends IPC on every mouse move (hundreds per second)
window.addEventListener('mousemove', (e) => {
  invoke('update-cursor-position', { x: e.clientX, y: e.clientY });
});

// GOOD: Throttle to max 60fps
let lastSend = 0;
window.addEventListener('mousemove', (e) => {
  const now = Date.now();
  if (now - lastSend >= 16) { // ~60fps
    invoke('update-cursor-position', { x: e.clientX, y: e.clientY });
    lastSend = now;
  }
});
```

### Minimize Payload Size

```ts
// BAD: Sending entire objects when only IDs are needed
invoke('delete-products', products); // Full product objects with all fields

// GOOD: Send only what's needed
invoke('delete-products', products.map(p => p.id)); // Just IDs
```

### Structured Clone Transfer

```ts
// Electron 28+ uses structured clone for IPC by default
// This is faster than JSON serialization for complex objects
// Supports: Date, RegExp, Map, Set, ArrayBuffer, TypedArrays
// Does NOT support: Functions, DOM nodes, Error objects

// For very large data, consider transferring ArrayBuffers
// (moves instead of copies — zero-copy transfer)
```

---

## ASAR Optimization

### Exclude Dev Files from ASAR

```json
// electron-builder.yml or package.json
{
  "build": {
    "asar": true,
    "asarUnpack": [
      "node_modules/sql.js/dist/sql-wasm.wasm"
    ],
    "files": [
      "dist/**/*",
      "!node_modules/**/*.md",
      "!node_modules/**/*.d.ts",
      "!node_modules/**/test/**",
      "!node_modules/**/tests/**",
      "!node_modules/**/docs/**",
      "!node_modules/**/.github/**"
    ]
  }
}
```

### Minimize Package Size

- Remove dev dependencies from production build.
- Exclude source maps from production.
- Use `electron-builder`'s `files` filter to exclude test files, docs, and type declarations.
- Consider `@electron/asar` for manual packing with custom exclusions.

---

## Window Creation Optimization

### Lazy-Create Secondary Windows

```ts
// BAD: Create all windows at startup
const mainWindow = createMainWindow();
const settingsWindow = createSettingsWindow();
const reportWindow = createReportWindow();

// GOOD: Create windows only when needed
let settingsWindow: BrowserWindow | null = null;

function getSettingsWindow() {
  if (!settingsWindow || settingsWindow.isDestroyed()) {
    settingsWindow = createSettingsWindow();
  }
  return settingsWindow;
}

ipcMain.handle('open-settings', () => {
  const win = getSettingsWindow();
  win.show();
});
```

### Reuse Hidden Windows

```ts
// Instead of destroying and recreating, hide and reuse
function createReusableWindow() {
  const win = new BrowserWindow({ show: false, /* ... */ });
  win.loadFile('index.html');

  // Override close to hide instead of destroy
  win.on('close', (e) => {
    if (!app.isQuitting) {
      e.preventDefault();
      win.hide();
    }
  });

  return win;
}
```

### BrowserWindow Creation Options for Speed

```ts
const win = new BrowserWindow({
  show: false,                    // Don't show until ready
  backgroundColor: '#1a1a2e',    // Matches app background — prevents white flash
  webPreferences: {
    preload: preloadPath,
    nodeIntegration: false,
    contextIsolation: true,
    sandbox: true,
    // Disable features you don't use
    spellcheck: false,
    enableWebSQL: false,
  },
});

// Show only when content is ready
win.once('ready-to-show', () => {
  win.show();
});
```
