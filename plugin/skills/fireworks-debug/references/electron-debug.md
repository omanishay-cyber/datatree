# Electron Debugging Reference

> Debugging patterns specific to Electron apps: IPC, preload, main process,
> packaging, window management, auto-updater, native modules, and CSP.

---

## 1. IPC Failures

### Debugging invoke/handle Mismatches

The most common IPC failure is a channel name mismatch. The renderer invokes a channel the main process does not handle, or vice versa.

**Diagnostic Steps:**
1. Check the renderer call: `window.api.methodName(args)`
2. Check the preload bridge: what channel does `methodName` map to?
3. Check the main handler: is there an `ipcMain.handle('channel-name', ...)` registered?
4. Compare channel names character-by-character (case-sensitive, hyphen vs underscore).

**Common Mismatches:**
```typescript
// Preload says:
ipcRenderer.invoke('get-products')
// Main says:
ipcMain.handle('getProducts')  // MISMATCH: hyphen vs camelCase
```

**Debugging Template:**
```typescript
// Add to main process startup to log ALL registered handlers:
const originalHandle = ipcMain.handle.bind(ipcMain);
ipcMain.handle = (channel: string, handler: any) => {
  console.log(`[IPC] Registered handler: ${channel}`);
  return originalHandle(channel, handler);
};
```

### Checking the Preload Bridge

```typescript
// In renderer console, check what APIs are exposed:
console.log('Available API:', Object.keys(window.api));
console.log('Has method:', typeof window.api.someMethod);
```

### Verifying contextBridge Exposure

```typescript
// preload.ts — verify this structure:
contextBridge.exposeInMainWorld('api', {
  // Each method must be explicitly listed:
  getProducts: (...args: any[]) => ipcRenderer.invoke('get-products', ...args),
  saveProduct: (...args: any[]) => ipcRenderer.invoke('save-product', ...args),
  // Missing method here = undefined in renderer
});
```

**Key Rule:** contextBridge does NOT proxy automatically. Every method must be explicitly defined.

---

## 2. Preload Issues

### Preload Script Errors

If the preload script has a syntax error or throws during loading, the renderer's `window.api` will be undefined, but the error only appears in the main process console.

**Diagnostic Steps:**
1. Check the terminal where `npm run dev` is running — preload errors appear here.
2. Verify the preload path in `BrowserWindow` options:
```typescript
new BrowserWindow({
  webPreferences: {
    preload: path.join(__dirname, 'preload.js'), // Check this path exists
    contextIsolation: true,
    nodeIntegration: false,
  },
});
```
3. In dev mode, the preload file might be at a different path than in production.

### contextIsolation Gotchas

When `contextIsolation: true` (which it should always be):
- The preload script runs in an isolated context.
- You CANNOT access `window.api` from the preload script itself — it is only available in the renderer.
- You CANNOT attach methods directly to `window` — you must use `contextBridge.exposeInMainWorld`.
- Objects passed through contextBridge are cloned, not referenced. Functions, Promises, and basic types work. Class instances, Symbols, and prototypes do NOT.

### Missing API Exposure

If `window.api` is undefined in the renderer:
1. Preload script failed to load (check path).
2. Preload script threw an error (check main process logs).
3. `contextIsolation` is false but code assumes true (or vice versa).
4. The `exposeInMainWorld` call uses a different key than expected.

---

## 3. Main Process Crashes

### uncaughtException Handler
```typescript
process.on('uncaughtException', (error) => {
  console.error('[MAIN] Uncaught exception:', error);
  // Log to file for post-crash analysis:
  const logPath = path.join(app.getPath('userData'), 'crash.log');
  fs.appendFileSync(logPath, `${new Date().toISOString()}: ${error.stack}\n`);
  // Show dialog to user:
  dialog.showErrorBox('Application Error', error.message);
});

process.on('unhandledRejection', (reason) => {
  console.error('[MAIN] Unhandled rejection:', reason);
});
```

### Render Process Gone
```typescript
mainWindow.webContents.on('render-process-gone', (event, details) => {
  console.error('[MAIN] Renderer crashed:', details.reason, details.exitCode);
  // reasons: 'clean-exit', 'abnormal-exit', 'killed', 'crashed', 'oom', 'launch-failed'
  if (details.reason === 'crashed' || details.reason === 'oom') {
    // Offer to reload:
    dialog.showMessageBox(mainWindow, {
      type: 'error',
      title: 'Application Crashed',
      message: 'The application encountered an error. Would you like to reload?',
      buttons: ['Reload', 'Close'],
    }).then(({ response }) => {
      if (response === 0) mainWindow.reload();
      else mainWindow.close();
    });
  }
});
```

### Diagnostic Approaches
- Check if the main process is running via Task Manager
- Verify Electron version: `npx electron --version`
- Run with verbose logging via the `ELECTRON_ENABLE_LOGGING=1` environment variable

---

## 4. ASAR Packaging

### Files Not Found in ASAR

In production, your app is packaged into an `app.asar` file. Some file operations do not work inside ASAR:
- `fs.readFileSync` works for reading.
- Spawning executables does NOT work for executables inside ASAR.
- Native modules cannot be loaded from inside ASAR.
- SQLite database files should NOT be inside ASAR (they need to be writable).

### extraResources Configuration

Files that must exist outside ASAR go in `extraResources`:
```json
{
  "extraResources": [
    {
      "from": "resources/",
      "to": "resources/",
      "filter": ["**/*"]
    }
  ]
}
```

Access extraResources at runtime:
```typescript
const resourcePath = app.isPackaged
  ? path.join(process.resourcesPath, 'resources')
  : path.join(__dirname, '..', 'resources');
```

### ASAR Debugging
```typescript
// Check if running from ASAR:
console.log('Is packaged:', app.isPackaged);
console.log('App path:', app.getAppPath()); // Ends with .asar if packaged
console.log('Resource path:', process.resourcesPath);
```

---

## 5. Window Management

### BrowserWindow Events for Debugging
```typescript
mainWindow.on('ready-to-show', () => console.log('[WIN] ready-to-show'));
mainWindow.on('show', () => console.log('[WIN] show'));
mainWindow.on('focus', () => console.log('[WIN] focus'));
mainWindow.on('blur', () => console.log('[WIN] blur'));
mainWindow.on('close', (e) => console.log('[WIN] close'));
mainWindow.on('closed', () => console.log('[WIN] closed'));
mainWindow.on('unresponsive', () => console.log('[WIN] UNRESPONSIVE'));
mainWindow.on('responsive', () => console.log('[WIN] responsive again'));
```

### webContents Debugging
```typescript
mainWindow.webContents.on('did-fail-load', (event, errorCode, errorDescription) => {
  console.error('[WEB] Failed to load:', errorCode, errorDescription);
});

mainWindow.webContents.on('did-finish-load', () => {
  console.log('[WEB] Finished loading');
});

mainWindow.webContents.on('console-message', (event, level, message, line, sourceId) => {
  console.log(`[RENDERER] ${message} (${sourceId}:${line})`);
});
```

### Opening DevTools in Production
```typescript
// Add a hidden shortcut for production debugging:
globalShortcut.register('CommandOrControl+Shift+I', () => {
  const focusedWindow = BrowserWindow.getFocusedWindow();
  if (focusedWindow) {
    focusedWindow.webContents.toggleDevTools();
  }
});
```

---

## 6. Auto-Updater

### electron-updater Debugging
```typescript
import { autoUpdater } from 'electron-updater';

// Enable verbose logging:
autoUpdater.logger = require('electron-log');
(autoUpdater.logger as any).transports.file.level = 'debug';

// Listen to all events:
autoUpdater.on('checking-for-update', () => console.log('[UPDATE] Checking...'));
autoUpdater.on('update-available', (info) => console.log('[UPDATE] Available:', info.version));
autoUpdater.on('update-not-available', () => console.log('[UPDATE] Not available'));
autoUpdater.on('download-progress', (progress) => console.log('[UPDATE] Progress:', progress.percent));
autoUpdater.on('update-downloaded', (info) => console.log('[UPDATE] Downloaded:', info.version));
autoUpdater.on('error', (error) => console.error('[UPDATE] Error:', error));
```

### Common Update Issues
- **Update server unreachable**: Check the `publish` config in `electron-builder.yml`. Verify the URL is accessible.
- **Signature mismatch**: Code signing certificate changed between versions. Users must reinstall.
- **Differential update fails**: Set `autoUpdater.autoInstallOnAppQuit = true` and fall back to full download.
- **NSIS installer error**: Check Windows event logs. Run installer with `/LOG` flag.
- **Permission denied**: App installed in Program Files requires admin rights to update. Use per-user install.

---

## 7. Native Module Issues

### node-gyp / electron-rebuild

Native modules compiled for Node.js will not work with Electron — they must be recompiled for Electron's ABI.

```bash
# Rebuild all native modules for current Electron version:
npx electron-rebuild

# Rebuild a specific module:
npx electron-rebuild -m node_modules/better-sqlite3

# Check ABI compatibility:
npx electron -e "console.log(process.versions)"
```

### Architecture Mismatches
- x64 module on arm64 Electron (or vice versa) will crash on load.
- Check: `process.arch` should match the native module's compiled architecture.
- Fix: rebuild with the correct `--arch` flag:
```bash
npx electron-rebuild --arch=x64
```

### sql.js Specific
sql.js uses WASM, not native modules, so electron-rebuild is NOT needed. Instead:
- Ensure the WASM file is accessible at runtime (not inside ASAR).
- Configure Vite to copy the WASM file to the output:
```typescript
// vite.config.ts
{
  plugins: [
    viteStaticCopy({
      targets: [{
        src: 'node_modules/sql.js/dist/sql-wasm.wasm',
        dest: '.'
      }]
    })
  ]
}
```

---

## 8. CSP Violations

### Content-Security-Policy Debugging

Electron apps should have a strict CSP. When CSP blocks something, it appears as an error in the renderer console.

**Common CSP Errors:**
- `Refused to run inline script`: Add a nonce or hash, or move script to a file.
- `Refused to evaluate a string as JavaScript`: Dynamic code evaluation is blocked. Refactor the code to avoid eval patterns.
- `Refused to load the image`: Add the image source domain to `img-src`.

**Setting CSP in Electron:**
```typescript
// In main process, set CSP via session:
session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
  callback({
    responseHeaders: {
      ...details.responseHeaders,
      'Content-Security-Policy': [
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';"
      ],
    },
  });
});
```

**Debugging CSP:**
```typescript
// Listen for CSP violations in the renderer:
document.addEventListener('securitypolicyviolation', (e) => {
  console.error('[CSP] Violation:', {
    directive: e.violatedDirective,
    blockedURI: e.blockedURI,
    sourceFile: e.sourceFile,
    lineNumber: e.lineNumber,
  });
});
```

**Dev vs Production:**
- In development, you may need a looser CSP (e.g., allow Vite HMR WebSocket).
- In production, tighten CSP to the minimum required.
- Avoid `unsafe-eval` in production unless absolutely necessary (and document why).
