# Electron Packaging — Deep Reference

> electron-builder configuration, platform-specific builds, code signing, notarization, auto-updater, ASAR, and build optimization.

---

## electron-builder Configuration

### Package.json Config (Alternative to electron-builder.yml)

```json
{
  "build": {
    "appId": "com.company.appname",
    "productName": "App Name",
    "copyright": "Copyright (C) 2026 Company Name",
    "directories": {
      "output": "dist",
      "buildResources": "build"
    },
    "files": [
      "dist/**/*",
      "node_modules/**/*",
      "!node_modules/**/test/**",
      "!node_modules/**/tests/**",
      "!node_modules/**/*.md",
      "!node_modules/**/*.map",
      "!node_modules/**/LICENSE*",
      "!node_modules/**/.eslintrc*",
      "!node_modules/**/tsconfig*"
    ],
    "asar": true,
    "compression": "maximum"
  }
}
```

### electron-builder.yml Config (Preferred)

```yaml
appId: com.company.appname
productName: "App Name"
copyright: "Copyright (C) 2026 Company Name"

directories:
  output: dist
  buildResources: build

files:
  - "dist/**/*"
  - "node_modules/**/*"
  - "!node_modules/**/test/**"
  - "!node_modules/**/*.map"

asar: true
compression: maximum

# Rebuild native modules for Electron's Node version
npmRebuild: true
nodeGypRebuild: false
```

---

## Windows (NSIS Installer)

### Full Windows Configuration

```yaml
win:
  target:
    - target: nsis
      arch: [x64, arm64]
  icon: "build/icon.ico"
  publisherName: "Company Name"
  verifyUpdateCodeSignature: true
  requestedExecutionLevel: asInvoker
  signDlls: true

nsis:
  oneClick: false
  perMachine: false
  allowToChangeInstallationDirectory: true
  allowElevation: true
  installerIcon: "build/icon.ico"
  uninstallerIcon: "build/icon.ico"
  installerHeaderIcon: "build/icon.ico"
  createDesktopShortcut: true
  createStartMenuShortcut: true
  shortcutName: "App Name"
  deleteAppDataOnUninstall: false
  runAfterFinish: true
  installerSidebar: "build/installerSidebar.bmp"  # 164x314
  license: "LICENSE"
```

### Code Signing (Windows)

```yaml
win:
  certificateFile: "path/to/certificate.pfx"
  certificatePassword: ""  # Set via CSC_KEY_PASSWORD env var
  # Or use certificate from Windows cert store:
  certificateSubjectName: "Company Name"
  certificateSha1: "THUMBPRINT"

# Environment variables (preferred over config file):
# CSC_LINK=path/to/cert.pfx
# CSC_KEY_PASSWORD=your-password
```

### Portable Mode

```yaml
win:
  target:
    - target: nsis
    - target: portable
      arch: [x64]

portable:
  artifactName: "${productName}-portable-${version}.${ext}"
```

---

## macOS (DMG + Notarization)

### Full macOS Configuration

```yaml
mac:
  target:
    - target: dmg
      arch: [x64, arm64]
    - target: zip
      arch: [x64, arm64]
  icon: "build/icon.icns"
  category: "public.app-category.business"
  hardenedRuntime: true
  gatekeeperAssess: false
  entitlements: "build/entitlements.mac.plist"
  entitlementsInherit: "build/entitlements.mac.inherit.plist"
  darkModeSupport: true
  minimumSystemVersion: "10.15"

dmg:
  background: "build/dmg-background.png"  # 540x380
  iconSize: 80
  iconTextSize: 12
  contents:
    - x: 130
      y: 220
    - x: 410
      y: 220
      type: link
      path: /Applications
```

### Entitlements (macOS)

```xml
<!-- build/entitlements.mac.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
  <key>com.apple.security.cs.allow-jit</key>
  <true/>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
  <true/>
  <key>com.apple.security.cs.allow-dyld-environment-variables</key>
  <true/>
  <key>com.apple.security.network.client</key>
  <true/>
  <key>com.apple.security.files.user-selected.read-write</key>
  <true/>
</dict>
</plist>
```

### Notarization

```yaml
# In electron-builder config
afterSign: "scripts/notarize.js"

# Or via environment variables:
# APPLE_ID=your@email.com
# APPLE_APP_SPECIFIC_PASSWORD=xxxx-xxxx-xxxx-xxxx
# APPLE_TEAM_ID=XXXXXXXXXX
```

```javascript
// scripts/notarize.js
const { notarize } = require('@electron/notarize');

exports.default = async function notarizing(context) {
  const { electronPlatformName, appOutDir } = context;
  if (electronPlatformName !== 'darwin') return;

  const appName = context.packager.appInfo.productFilename;
  return await notarize({
    appBundleId: 'com.company.appname',
    appPath: `${appOutDir}/${appName}.app`,
    appleId: process.env.APPLE_ID,
    appleIdPassword: process.env.APPLE_APP_SPECIFIC_PASSWORD,
    teamId: process.env.APPLE_TEAM_ID,
  });
};
```

---

## Linux

```yaml
linux:
  target:
    - target: AppImage
      arch: [x64]
    - target: deb
      arch: [x64]
    - target: snap
      arch: [x64]
  icon: "build/icons"  # Directory with 16x16, 32x32, ... 512x512 PNGs
  category: "Office"
  maintainer: "your@email.com"
  vendor: "Company Name"
  synopsis: "Short description"
  description: "Longer description of the app"

deb:
  depends: ["libgtk-3-0", "libnotify4", "libnss3", "libxss1"]

snap:
  confinement: strict
  grade: stable
```

---

## Auto-Update Flow

### electron-updater Setup

```typescript
// main/updater.ts
import { autoUpdater } from 'electron-updater';
import { BrowserWindow } from 'electron';
import log from 'electron-log';

autoUpdater.logger = log;
autoUpdater.autoDownload = false;  // Ask user before downloading

export function setupAutoUpdater(mainWindow: BrowserWindow): void {
  // Check for updates on startup (with delay)
  setTimeout(() => {
    autoUpdater.checkForUpdates();
  }, 10_000);

  // Also check periodically (every 4 hours)
  setInterval(() => {
    autoUpdater.checkForUpdates();
  }, 4 * 60 * 60 * 1000);

  autoUpdater.on('update-available', (info) => {
    mainWindow.webContents.send('update:available', {
      version: info.version,
      releaseDate: info.releaseDate,
      releaseNotes: info.releaseNotes,
    });
  });

  autoUpdater.on('download-progress', (progress) => {
    mainWindow.webContents.send('update:progress', {
      percent: progress.percent,
      bytesPerSecond: progress.bytesPerSecond,
      transferred: progress.transferred,
      total: progress.total,
    });
  });

  autoUpdater.on('update-downloaded', (info) => {
    mainWindow.webContents.send('update:downloaded', {
      version: info.version,
    });
  });

  autoUpdater.on('error', (error) => {
    log.error('Auto-updater error:', error);
    mainWindow.webContents.send('update:error', error.message);
  });
}

// IPC handler for user-initiated actions
ipcMain.handle('update:download', () => autoUpdater.downloadUpdate());
ipcMain.handle('update:install', () => autoUpdater.quitAndInstall());
```

### Update Flow

```
1. App starts → check for updates (after 10s delay)
2. If update available → notify renderer → show update banner
3. User clicks "Download" → download in background → show progress
4. Download complete → notify renderer → show "Restart to update"
5. User clicks "Restart" → quitAndInstall() → app restarts with new version
6. OR user dismisses → update installs on next natural restart
```

---

## ASAR Optimization

### What Goes Inside ASAR

```
INSIDE (compiled, bundled code):
  - dist/main/        (compiled main process code)
  - dist/renderer/    (compiled renderer code, HTML, CSS)
  - dist/preload/     (compiled preload scripts)
  - Small static assets (icons, small images < 1MB)
```

### What Goes Outside ASAR

```yaml
asarUnpack:
  - "**/*.node"              # Native Node.js addons
  - "**/sql.js/dist/*.wasm"  # WebAssembly files
  - "**/sharp/**"            # Image processing native deps
  - "**/better-sqlite3/**"   # SQLite native binding

extraResources:
  - from: "resources/database/"
    to: "database/"
  - from: "resources/templates/"
    to: "templates/"
  - from: "resources/fonts/"
    to: "fonts/"
```

### Build Size Optimization

```yaml
# Exclude unnecessary files
files:
  - "dist/**/*"
  - "node_modules/**/*"
  - "!**/*.ts"               # Source TypeScript
  - "!**/*.map"              # Source maps
  - "!**/*.md"               # Markdown docs
  - "!**/test/**"            # Test files
  - "!**/tests/**"           # Test files
  - "!**/__tests__/**"       # Test files
  - "!**/docs/**"            # Documentation
  - "!**/example/**"         # Examples
  - "!**/examples/**"        # Examples
  - "!**/.eslintrc*"         # Lint configs
  - "!**/tsconfig*"          # TS configs
  - "!**/LICENSE*"           # Licenses (keep in app, not in each module)
  - "!**/CHANGELOG*"         # Changelogs
  - "!**/*.d.ts"             # Type definitions (not needed at runtime)

# Use maximum compression
compression: maximum

# Remove locales you don't need (Electron includes all by default)
# Use electron-builder-notarize or custom afterPack script
```

### Verifying Build Contents

```bash
# List files in the ASAR archive
npx asar list dist/win-unpacked/resources/app.asar

# Extract ASAR to inspect contents
npx asar extract dist/win-unpacked/resources/app.asar ./extracted

# Check total size
du -sh dist/win-unpacked/
du -sh dist/*.exe
```
