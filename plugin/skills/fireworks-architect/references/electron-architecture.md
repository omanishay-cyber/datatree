# Electron + React + TypeScript Architecture Patterns

## Process Model

Electron applications run in three distinct contexts. Understanding their boundaries is the foundation of secure, performant Electron architecture.

```
Main Process (Node.js)
  |-- Window Management (BrowserWindow lifecycle)
  |-- IPC Handlers (ipcMain.handle)
  |-- System Integration (tray, menu, global shortcuts)
  |-- Database (sql.js — in-memory SQLite)
  |-- File System (read/write app data, exports)
  |-- Auto-Updater (electron-updater)
  |-- Encryption (envelope encryption for sensitive data)
  +-- Background Tasks (sync, backup, scheduled operations)

Preload Script (Bridge)
  +-- contextBridge.exposeInMainWorld('api', {
        invoke: (channel, ...args) => ipcRenderer.invoke(channel, ...args),
        on: (channel, callback) => ipcRenderer.on(channel, callback),
        // ... typed method wrappers
      })

Renderer Process (Chromium)
  |-- React App (entry point)
  |-- Router (React Router or TanStack Router)
  |-- Zustand Stores (UI and domain state)
  |-- Components (functional, with Error Boundaries)
  |-- Hooks (custom hooks for IPC, data fetching)
  +-- window.api.* calls (typed bridge to main process)
```

### Process Isolation Rules

1. **Renderer NEVER accesses Node.js APIs** — no `fs`, `path`, `child_process`, `crypto`
2. **Main NEVER accesses DOM or React** — no `document`, `window`, `useStore`
3. **Preload is minimal** — only exposes the API surface, no business logic
4. **All communication goes through IPC** — no shared memory, no global variables

---

## IPC Architecture

### Channel Naming Convention

Use `domain:action` format for all IPC channels:

```
db:query          — Database queries
db:insert         — Database inserts
db:update         — Database updates
db:delete         — Database deletes
auth:login        — Authentication
auth:logout       — Authentication
sync:push         — Sync operations
sync:pull         — Sync operations
export:excel      — File exports
export:pdf        — File exports
app:get-version   — App metadata
app:check-update  — Auto-updater
window:minimize   — Window management
window:maximize   — Window management
```

### Type Definition Pattern

Define shared types in a file accessible to both main and renderer:

```typescript
// src/shared/ipc-types.ts

export interface IpcChannels {
  'db:query': {
    params: { table: string; where?: Record<string, unknown>; limit?: number };
    result: Record<string, unknown>[];
  };
  'db:insert': {
    params: { table: string; data: Record<string, unknown> };
    result: { id: number };
  };
  'auth:login': {
    params: { username: string; password: string };
    result: { token: string; user: User };
  };
  'sync:push': {
    params: { since: string };
    result: { pushed: number; conflicts: string[] };
  };
}

export type IpcChannel = keyof IpcChannels;
```

### Preload Bridge Pattern

```typescript
// src/preload/index.ts
import { contextBridge, ipcRenderer } from 'electron';
import type { IpcChannels, IpcChannel } from '../shared/ipc-types';

const api = {
  invoke: <C extends IpcChannel>(
    channel: C,
    params: IpcChannels[C]['params']
  ): Promise<IpcChannels[C]['result']> => {
    return ipcRenderer.invoke(channel, params);
  },
  on: (channel: string, callback: (...args: unknown[]) => void) => {
    ipcRenderer.on(channel, (_event, ...args) => callback(...args));
    return () => ipcRenderer.removeListener(channel, callback);
  },
};

contextBridge.exposeInMainWorld('api', api);
```

### Handler Pattern with Validation

```typescript
// src/main/handlers/db-handlers.ts
import { ipcMain } from 'electron';
import { z } from 'zod';

const querySchema = z.object({
  table: z.string().min(1),
  where: z.record(z.unknown()).optional(),
  limit: z.number().positive().optional(),
});

ipcMain.handle('db:query', async (_event, params) => {
  const validated = querySchema.parse(params);
  // Now 'validated' is type-safe and sanitized
  return db.query(validated.table, validated.where, validated.limit);
});
```

### Error Propagation

```typescript
// In main process handler:
ipcMain.handle('db:query', async (_event, params) => {
  try {
    const validated = querySchema.parse(params);
    return { success: true, data: await db.query(validated) };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
      code: error instanceof ZodError ? 'VALIDATION_ERROR' : 'INTERNAL_ERROR',
    };
  }
});

// In renderer:
const result = await window.api.invoke('db:query', params);
if (!result.success) {
  showToast({ type: 'error', message: result.error });
}
```

---

## State Sync Pattern

### What Lives Where

| State Type | Location | Example |
|-----------|----------|---------|
| UI-only state | Zustand (renderer) | sidebar open, selected tab, filter text |
| Domain data | Main process (database) | products, invoices, customers |
| Cached domain data | Zustand (renderer) | fetched products list, current user |
| App settings | Main process (config file) | theme, language, window size |

### Data Flow: Read

```
User Action (click, navigate)
  -> React Component calls store action
    -> Store action calls window.api.invoke('db:query', params)
      -> IPC -> Main Process Handler
        -> Zod validation -> Database query -> Return result
      -> IPC response
    -> Store updates state
  -> React re-renders with new data
```

### Data Flow: Write

```
User Action (form submit, button click)
  -> React Component calls store action
    -> Store sets loading: true
    -> Store calls window.api.invoke('db:insert', data)
      -> IPC -> Main Process Handler
        -> Zod validation -> Database insert -> Return result
      -> IPC response
    -> Store updates state (add to list, set loading: false)
    -> Main process emits event if other windows need notification
  -> React re-renders
```

### Main-to-Renderer Notifications

For changes initiated by the main process (auto-sync, background tasks):

```typescript
// Main process:
mainWindow.webContents.send('sync:update', { table: 'products', count: 5 });

// Renderer (via preload):
window.api.on('sync:update', (data) => {
  useProductStore.getState().fetchProducts(); // Refresh from DB
});
```

---

## Window Management

### BrowserWindow Lifecycle

```typescript
const mainWindow = new BrowserWindow({
  width: 1280,
  height: 800,
  minWidth: 1024,
  minHeight: 600,
  webPreferences: {
    preload: path.join(__dirname, 'preload.js'),
    contextIsolation: true,    // ALWAYS true
    sandbox: true,             // ALWAYS true
    nodeIntegration: false,    // ALWAYS false
    webSecurity: true,         // ALWAYS true
  },
  show: false, // Show after ready-to-show to prevent flash
});

mainWindow.once('ready-to-show', () => {
  mainWindow.show();
});
```

### Window State Persistence

```typescript
// Save on close
mainWindow.on('close', () => {
  const bounds = mainWindow.getBounds();
  config.set('windowBounds', bounds);
});

// Restore on create
const savedBounds = config.get('windowBounds');
if (savedBounds) {
  mainWindow.setBounds(savedBounds);
}
```

---

## Security Model

### Defense in Depth

1. **contextIsolation: true** — renderer cannot access Electron internals
2. **sandbox: true** — renderer restricted to web platform APIs
3. **nodeIntegration: false** — no Node.js APIs in renderer
4. **webSecurity: true** — enforces same-origin policy
5. **Content Security Policy** — restrict script sources, prevent inline scripts
6. **Zod validation** — validate all IPC inputs in main process
7. **Parameterized queries** — prevent SQL injection
8. **Envelope encryption** — encrypt sensitive data at rest

### CSP Configuration

```typescript
session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
  callback({
    responseHeaders: {
      ...details.responseHeaders,
      'Content-Security-Policy': [
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self';"
      ],
    },
  });
});
```

---

## Application Data Patterns

### App Data Location

```typescript
import { app } from 'electron';

const appDataPath = app.getPath('userData');
// Windows: C:\Users\{user}\AppData\Roaming\{appName}

const dbPath = path.join(appDataPath, 'database.sqlite');
const configPath = path.join(appDataPath, 'config.json');
const logsPath = path.join(appDataPath, 'logs');
```

### Graceful Shutdown

```typescript
app.on('before-quit', async () => {
  // Save database to disk
  const data = db.export();
  fs.writeFileSync(dbPath, Buffer.from(data));

  // Flush any pending sync operations
  await syncManager.flush();

  // Close all windows
  BrowserWindow.getAllWindows().forEach(w => w.destroy());
});
```
