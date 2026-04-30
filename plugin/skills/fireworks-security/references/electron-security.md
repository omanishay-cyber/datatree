# Electron Security Hardening Reference

## BrowserWindow Security Options

Every BrowserWindow MUST use these security defaults:

```typescript
const win = new BrowserWindow({
  webPreferences: {
    nodeIntegration: false,          // MANDATORY: false
    contextIsolation: true,          // MANDATORY: true
    sandbox: true,                   // RECOMMENDED: true
    webSecurity: true,               // MANDATORY: true
    allowRunningInsecureContent: false,
    experimentalFeatures: false,
    preload: path.join(__dirname, 'preload.js'),
  },
});
```

### Why Each Setting Matters
| Setting | Value | Risk if Wrong |
|---|---|---|
| nodeIntegration | false | XSS = full system access (RCE) |
| contextIsolation | true | Renderer can access Node.js APIs |
| sandbox | true | Compromised renderer has OS access |
| webSecurity | true | Disables same-origin policy |
| allowRunningInsecureContent | false | HTTP content in HTTPS pages |

---

## Content Security Policy (CSP)

### Meta Tag (Renderer)
```html
<meta http-equiv="Content-Security-Policy" content="
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  font-src 'self';
  connect-src 'self' https://api.github.com;
  object-src 'none';
  base-uri 'self';
">
```

### CSP Rules
- Never use `unsafe-eval` — blocks eval(), new Function()
- Minimize `unsafe-inline` for styles — prefer CSS files
- Never use `*` wildcards in script-src
- Allowlist specific domains for connect-src
- Set `object-src 'none'` to block plugins

---

## Preload Script Security

### Principle: Minimal API Surface

```typescript
// BAD — exposes too much
contextBridge.exposeInMainWorld('api', {
  fs: require('fs'),
  exec: require('child_process').exec,
  ipcRenderer: ipcRenderer,
});

// GOOD — minimal, typed, validated
contextBridge.exposeInMainWorld('api', {
  getProducts: (filter: ProductFilter) =>
    ipcRenderer.invoke('db:products:list', filter),
  saveProduct: (product: ProductInput) =>
    ipcRenderer.invoke('db:products:save', product),
  onSyncStatus: (callback: (status: SyncStatus) => void) => {
    const handler = (_e: IpcRendererEvent, status: SyncStatus) => callback(status);
    ipcRenderer.on('sync:status', handler);
    return () => ipcRenderer.removeListener('sync:status', handler);
  },
});
```

### Preload Checklist
- [ ] No require() exposed to renderer
- [ ] No process object exposed
- [ ] All IPC through ipcRenderer.invoke() (not send)
- [ ] Event listeners return cleanup functions
- [ ] No dynamic channel names from renderer

---

## IPC Security

### Validate ALL Inputs with Zod
```typescript
import { z } from 'zod';

const querySchema = z.object({
  search: z.string().max(200).optional(),
  limit: z.number().int().min(1).max(1000).default(50),
});

ipcMain.handle('db:products:list', async (_event, params: unknown) => {
  try {
    const validated = querySchema.parse(params);
    return { success: true, data: await db.listProducts(validated) };
  } catch (error) {
    return { success: false, error: 'Invalid input' };
  }
});
```

### IPC Rules
1. Never trust renderer — validate every input
2. Use invoke/handle — not send/on
3. Never use sendSync — blocks renderer
4. Wrap handlers in try/catch
5. Don't expose channel names to renderer

---

## shell.openExternal() Security

```typescript
function safeOpenExternal(url: string): boolean {
  try {
    const parsed = new URL(url);
    const allowedProtocols = ['https:'];
    const allowedDomains = ['github.com', 'docs.google.com'];
    if (!allowedProtocols.includes(parsed.protocol)) return false;
    if (!allowedDomains.some(d => parsed.hostname.endsWith(d))) return false;
    shell.openExternal(url);
    return true;
  } catch {
    return false;
  }
}
```

---

## eval() and new Function()

Never use in renderer. CSP should block, but defense in depth:

```typescript
// BAD
eval(userInput);
new Function('return ' + userInput)();
setTimeout(userInput, 1000); // String form

// Detection
// grep -rn "eval\(|new Function\(" src/renderer/
```

---

## Remote Module

Deprecated. Never use.

```typescript
// BAD
const { BrowserWindow } = require('@electron/remote');

// GOOD — use IPC
// Renderer: ipcRenderer.invoke('window:open-settings')
// Main: ipcMain.handle('window:open-settings', () => createSettingsWindow())
```

---

## Protocol Handler Security

```typescript
protocol.registerFileProtocol('app', (request, callback) => {
  const url = request.url.replace('app://', '');
  const filePath = path.normalize(path.join(__dirname, url));
  if (!filePath.startsWith(__dirname)) {
    callback({ statusCode: 403 });
    return;
  }
  callback({ path: filePath });
});
```

---

## File System Access Restriction

```typescript
function validateFilePath(userPath: string): string | null {
  const appDataDir = app.getPath('userData');
  const resolved = path.resolve(appDataDir, userPath);
  if (!resolved.startsWith(appDataDir)) return null;
  return resolved;
}
```

---

## Auto-Updater Security

- Code signing certificate required
- Updates over HTTPS only
- Signature verification enabled (default with electron-updater)
- Downgrade prevention via version comparison

---

## Electron Fuses (Production)

```bash
npx @electron/fuses write \
  --app path/to/app \
  --enable-cookie-encryption \
  --enable-node-options-environment-variable=false \
  --enable-node-cli-inspect=false \
  --enable-embedded-asar-integrity-validation \
  --only-load-app-from-asar
```

| Fuse | Effect |
|---|---|
| onlyLoadAppFromAsar | Blocks code injection from filesystem |
| enableCookieEncryption | Encrypts cookies at rest |
| nodeOptionsEnvironmentVariable=false | Blocks NODE_OPTIONS |
| nodeCLIInspect=false | Blocks --inspect |

---

## Security Audit Checklist

```
[ ] nodeIntegration: false on ALL windows
[ ] contextIsolation: true on ALL windows
[ ] sandbox: true on ALL windows
[ ] webSecurity: true on ALL windows
[ ] CSP meta tag in index.html
[ ] No eval() or new Function() in renderer
[ ] No @electron/remote usage
[ ] All IPC handlers validate input with Zod
[ ] shell.openExternal() validates URLs
[ ] File paths validated against app directory
[ ] No secrets in source code
[ ] Auto-updater uses code signing
[ ] Electron fuses set for production
[ ] npm audit shows no critical vulnerabilities
[ ] Preload exposes minimal API surface
```
