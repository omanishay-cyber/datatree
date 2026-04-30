# Electron Desktop Patterns — Deep Reference Guide

## Overview

Electron apps must feel native while maintaining the premium glassmorphism aesthetic. This
means custom title bars, proper window management, native keyboard shortcuts, and platform
conventions that users expect from desktop software.

---

## Custom Title Bar

Frameless windows with a custom-built title bar for a unified premium look.

### Main Process Setup
```typescript
// main.ts
const mainWindow = new BrowserWindow({
  width: 1400,
  height: 900,
  frame: false,          // Remove native title bar
  titleBarStyle: 'hidden', // macOS: hide but keep traffic lights
  transparent: false,     // Set true only if you need full transparency
  backgroundColor: '#00000000',
  webPreferences: {
    preload: path.join(__dirname, 'preload.js'),
    contextIsolation: true,
    nodeIntegration: false,
  },
});
```

### Renderer Title Bar Component
```tsx
function TitleBar() {
  return (
    <div
      className="h-10 flex items-center justify-between px-3
        backdrop-blur-xl bg-white/10 dark:bg-black/20
        border-b border-white/10 select-none"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* App icon + title */}
      <div className="flex items-center gap-2">
        <img src="/icon.png" className="w-4 h-4" alt="" />
        <span className="text-sm font-medium">App Name</span>
      </div>

      {/* Window controls — must be no-drag */}
      <div
        className="flex items-center"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        <button
          onClick={() => window.electronAPI.minimize()}
          className="w-10 h-8 flex items-center justify-center hover:bg-white/10 transition-colors"
          aria-label="Minimize"
        >
          <Minus className="w-4 h-4" />
        </button>
        <button
          onClick={() => window.electronAPI.maximize()}
          className="w-10 h-8 flex items-center justify-center hover:bg-white/10 transition-colors"
          aria-label="Maximize"
        >
          <Square className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => window.electronAPI.close()}
          className="w-10 h-8 flex items-center justify-center hover:bg-red-500/80 transition-colors"
          aria-label="Close"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
```

### Preload API for Window Controls
```typescript
// preload.ts
import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('electronAPI', {
  minimize: () => ipcRenderer.invoke('window:minimize'),
  maximize: () => ipcRenderer.invoke('window:maximize'),
  close: () => ipcRenderer.invoke('window:close'),
  isMaximized: () => ipcRenderer.invoke('window:isMaximized'),
  onMaximizeChange: (callback: (maximized: boolean) => void) => {
    ipcRenderer.on('window:maximize-change', (_, maximized) => callback(maximized));
  },
});

// main.ts handlers
ipcMain.handle('window:minimize', (event) => {
  BrowserWindow.fromWebContents(event.sender)?.minimize();
});
ipcMain.handle('window:maximize', (event) => {
  const win = BrowserWindow.fromWebContents(event.sender);
  if (win?.isMaximized()) win.unmaximize();
  else win?.maximize();
});
ipcMain.handle('window:close', (event) => {
  BrowserWindow.fromWebContents(event.sender)?.close();
});
```

---

## Window Controls — Drag Region Rules

Critical rules for frameless windows:

1. **Drag region:** Set `WebkitAppRegion: 'drag'` on the title bar div
2. **No-drag for clickables:** ALL buttons, inputs, links inside the drag region need `WebkitAppRegion: 'no-drag'`
3. **Text selection:** Use `select-none` on the title bar to prevent text selection during drag
4. **Double-click:** The title bar should maximize/restore on double-click (handled by `-webkit-app-region: drag` automatically)
5. **Context menu:** Right-click on the drag area should show system menu (Electron default)

---

## Context Menus

Build native-feeling context menus that match the app theme.

```typescript
// Main process: build context menu
import { Menu, MenuItem } from 'electron';

ipcMain.handle('context-menu:show', (event, template: MenuItemTemplate[]) => {
  const menu = Menu.buildFromTemplate(
    template.map(item => ({
      label: item.label,
      type: item.type,
      enabled: item.enabled,
      accelerator: item.accelerator,
      click: () => event.sender.send('context-menu:action', item.id),
    }))
  );
  menu.popup({ window: BrowserWindow.fromWebContents(event.sender) ?? undefined });
});
```

```tsx
// Renderer: trigger context menu
function handleContextMenu(e: React.MouseEvent) {
  e.preventDefault();
  window.electronAPI.showContextMenu([
    { id: 'cut', label: 'Cut', accelerator: 'CmdOrCtrl+X' },
    { id: 'copy', label: 'Copy', accelerator: 'CmdOrCtrl+C' },
    { id: 'paste', label: 'Paste', accelerator: 'CmdOrCtrl+V' },
    { type: 'separator' },
    { id: 'delete', label: 'Delete', accelerator: 'Delete' },
  ]);
}
```

---

## System Tray

```typescript
// Main process
import { Tray, Menu, nativeImage } from 'electron';

let tray: Tray | null = null;

function createTray() {
  const icon = nativeImage.createFromPath(path.join(__dirname, 'tray-icon.png'));
  tray = new Tray(icon.resize({ width: 16, height: 16 }));

  const contextMenu = Menu.buildFromTemplate([
    { label: 'Show App', click: () => mainWindow.show() },
    { type: 'separator' },
    { label: 'Quit', click: () => app.quit() },
  ]);

  tray.setToolTip('App Name');
  tray.setContextMenu(contextMenu);

  // Double-click to show
  tray.on('double-click', () => mainWindow.show());

  // Badge count (Windows taskbar)
  mainWindow.setOverlayIcon(badgeIcon, 'New notifications');
}
```

---

## Window Management

### Remember Window Position and Size
```typescript
import Store from 'electron-store';

const store = new Store();

function createWindow() {
  const bounds = store.get('windowBounds', {
    width: 1400,
    height: 900,
    x: undefined,
    y: undefined,
  }) as Electron.Rectangle;

  const win = new BrowserWindow({ ...bounds, /* other options */ });

  // Save bounds on move/resize
  const saveBounds = () => {
    if (!win.isMaximized() && !win.isMinimized()) {
      store.set('windowBounds', win.getBounds());
    }
  };

  win.on('resize', saveBounds);
  win.on('move', saveBounds);

  // Remember maximized state
  win.on('maximize', () => store.set('isMaximized', true));
  win.on('unmaximize', () => store.set('isMaximized', false));

  if (store.get('isMaximized', false)) {
    win.maximize();
  }
}
```

### Multi-Monitor Support
```typescript
import { screen } from 'electron';

function ensureWindowOnScreen(bounds: Electron.Rectangle): Electron.Rectangle {
  const displays = screen.getAllDisplays();
  const isOnScreen = displays.some(display => {
    const { x, y, width, height } = display.workArea;
    return bounds.x >= x && bounds.y >= y
      && bounds.x + bounds.width <= x + width
      && bounds.y + bounds.height <= y + height;
  });

  if (!isOnScreen) {
    const primary = screen.getPrimaryDisplay().workArea;
    return {
      x: primary.x + Math.round((primary.width - bounds.width) / 2),
      y: primary.y + Math.round((primary.height - bounds.height) / 2),
      width: bounds.width,
      height: bounds.height,
    };
  }

  return bounds;
}
```

---

## Native Desktop Feel

### Respect OS Accent Color
```typescript
// Main process: get system accent color
const accentColor = systemPreferences.getAccentColor(); // Returns hex like '0078d4ff'
mainWindow.webContents.send('system:accent-color', `#${accentColor.slice(0, 6)}`);

// Renderer: apply as CSS variable
document.documentElement.style.setProperty('--system-accent', accentColor);
```

### System Font
```css
/* Always use system UI font for native feel */
body {
  font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
}
```

---

## Drag and Drop

```tsx
function DropZone({ onDrop }: { onDrop: (files: File[]) => void }) {
  const [isDragging, setIsDragging] = useState(false);

  return (
    <div
      onDragOver={e => { e.preventDefault(); setIsDragging(true); }}
      onDragLeave={() => setIsDragging(false)}
      onDrop={e => {
        e.preventDefault();
        setIsDragging(false);
        const files = Array.from(e.dataTransfer.files);
        onDrop(files);
      }}
      className={cn(
        "border-2 border-dashed rounded-xl p-8 text-center transition-all duration-200",
        isDragging
          ? "border-primary bg-primary/10 scale-[1.02]"
          : "border-white/20 hover:border-white/30"
      )}
    >
      <Upload className="w-8 h-8 mx-auto mb-3 text-muted-foreground" />
      <p className="text-sm text-muted-foreground">
        {isDragging ? "Drop files here" : "Drag files here or click to browse"}
      </p>
    </div>
  );
}
```

### Prevent Accidental Navigation
```typescript
// Main process: prevent file drops from navigating the window
mainWindow.webContents.on('will-navigate', (event) => {
  event.preventDefault();
});
```

---

## Keyboard Shortcuts

### Global Shortcuts (App-Wide)
```typescript
import { globalShortcut } from 'electron';

app.whenReady().then(() => {
  // Global shortcut (works even when app is not focused)
  globalShortcut.register('CommandOrControl+Shift+Space', () => {
    mainWindow.show();
    mainWindow.focus();
  });
});

app.on('will-quit', () => {
  globalShortcut.unregisterAll();
});
```

### Menu Accelerators
```typescript
const menu = Menu.buildFromTemplate([
  {
    label: 'File',
    submenu: [
      { label: 'New', accelerator: 'CmdOrCtrl+N', click: handleNew },
      { label: 'Open', accelerator: 'CmdOrCtrl+O', click: handleOpen },
      { label: 'Save', accelerator: 'CmdOrCtrl+S', click: handleSave },
      { type: 'separator' },
      { label: 'Quit', accelerator: 'CmdOrCtrl+Q', role: 'quit' },
    ],
  },
]);
```

---

## Splash Screen

Show a loading screen immediately while the main window loads heavy content.

```typescript
function createSplashScreen(): BrowserWindow {
  const splash = new BrowserWindow({
    width: 400,
    height: 300,
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    resizable: false,
    skipTaskbar: true,
  });

  splash.loadFile('splash.html');
  return splash;
}

async function createMainWindow(splash: BrowserWindow) {
  const main = new BrowserWindow({ show: false, /* ... */ });
  await main.loadFile('index.html');

  main.once('ready-to-show', () => {
    splash.destroy();
    main.show();
  });
}
```

---

## Frameless Window Gotchas

1. **Drag regions must be explicit** — nothing is draggable by default in frameless mode
2. **Resizable borders:** Add `resizable: true` in BrowserWindow options (works even frameless)
3. **Window snapping:** Windows Aero Snap works with frameless windows automatically
4. **macOS traffic lights:** Use `titleBarStyle: 'hiddenInset'` + `trafficLightPosition` to keep native buttons
5. **Linux:** Frameless windows may not support server-side decorations — test on target distro
6. **Min/max size:** Set `minWidth`, `minHeight` in BrowserWindow options to prevent unusable sizes
7. **Rounded corners:** On Windows 11, `transparent: true` + CSS `border-radius` on root element enables rounded window corners
