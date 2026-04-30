# Build Errors Reference

> Common build errors for TypeScript, Vite, Electron Builder, WASM, imports,
> and Node.js version issues. Each error includes the code, message, cause, and fix.

---

## 1. TypeScript Errors by Code

### TS2304: Cannot find name 'X'

**Message:** `error TS2304: Cannot find name 'MyComponent'.`

**Cause:** The identifier is used but never imported or declared. Common scenarios:
- Forgot to import a component, type, or function.
- Typo in the identifier name.
- The declaration is in a different scope or file that is not included in tsconfig.

**Fix:**
```typescript
// Add the missing import:
import { MyComponent } from './MyComponent';

// Or if it's a global type, add it to a .d.ts file:
declare global {
  interface Window {
    api: PreloadAPI;
  }
}
```

**Check:** Verify the source file is included in `tsconfig.json`'s `include` array.

---

### TS2345: Argument of type 'X' is not assignable to parameter of type 'Y'

**Message:** `error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.`

**Cause:** You are passing an argument of the wrong type to a function. Common scenarios:
- Passing a string where a number is expected (e.g., from form input).
- Passing `null` or `undefined` to a non-nullable parameter.
- Object is missing required properties.

**Fix:**
```typescript
// Convert the type:
const quantity = parseInt(inputValue, 10);
doSomething(quantity);

// Or update the function signature to accept the actual type:
function doSomething(value: string | number): void { /* ... */ }

// Or use a type guard:
if (typeof value === 'string') {
  doSomething(parseInt(value, 10));
}
```

---

### TS2322: Type 'X' is not assignable to type 'Y'

**Message:** `error TS2322: Type 'string | undefined' is not assignable to type 'string'.`

**Cause:** The value might be undefined or null, but the target requires a definite type. This is TypeScript's strict null checking protecting you from runtime errors.

**Fix:**
```typescript
// Option A: Provide a default value:
const name: string = product.name ?? 'Unknown';

// Option B: Narrow the type with a check:
if (product.name !== undefined) {
  setName(product.name); // Now TypeScript knows it's string
}

// Option C: Mark the property as optional in the target type:
interface Props {
  name?: string; // Allow undefined
}

// AVOID: Non-null assertion (hides the problem):
const name: string = product.name!; // Dangerous — might crash at runtime
```

---

### TS7006: Parameter 'X' implicitly has an 'any' type

**Message:** `error TS7006: Parameter 'event' implicitly has an 'any' type.`

**Cause:** TypeScript's `noImplicitAny` is enabled (which it should be) and a function parameter has no type annotation.

**Fix:**
```typescript
// Add the type annotation:
function handleClick(event: React.MouseEvent<HTMLButtonElement>): void { /* ... */ }

// For callbacks where you don't need the type:
array.map((item: Product) => item.name);

// For event handlers from Electron:
ipcMain.handle('channel', async (event: IpcMainInvokeEvent, arg: string) => { /* ... */ });
```

---

### TS2307: Cannot find module 'X' or its corresponding type declarations

**Message:** `error TS2307: Cannot find module './components/Header' or its corresponding type declarations.`

**Cause:**
- The file does not exist at the specified path.
- The file extension is missing or wrong.
- Path alias (e.g., `@/components/Header`) is not configured in tsconfig.
- The module has no type declarations (for third-party packages).

**Fix:**
```typescript
// Check the file exists:
// Does ./components/Header.tsx exist? (Note: .tsx, not .ts)

// For path aliases, verify tsconfig.json:
{
  "compilerOptions": {
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}

// For third-party packages without types:
npm install -D @types/package-name
// Or create a declaration file: src/types/package-name.d.ts
declare module 'package-name';
```

---

### TS2769: No overload matches this call

**Message:** `error TS2769: No overload matches this call.`

**Cause:** The function has multiple type signatures (overloads) and the arguments you passed do not match any of them. Common with React event handlers, DOM APIs, and library functions.

**Fix:**
- Read the overload signatures carefully (hover in IDE or check docs).
- Identify which overload you intend to use.
- Adjust your arguments to match that specific overload.
- If using a generic function, provide the type parameter explicitly:
```typescript
useState<Product[]>([]) // Instead of useState([])
```

---

## 2. Vite Build Errors

### Dependency Optimization Failures

**Error:** `[vite] Failed to resolve dependency: X`

**Cause:** Vite's dependency pre-bundling failed. The package may use CommonJS features incompatible with Vite's ESM transformation.

**Fix:**
```typescript
// vite.config.ts:
export default defineConfig({
  optimizeDeps: {
    include: ['problematic-package'], // Force pre-bundle
    exclude: ['package-that-breaks'], // Skip pre-bundling
  },
});
```

### CSS Import Issues

**Error:** `[vite] Failed to load CSS: X`

**Cause:** CSS file not found, or CSS module naming mismatch.

**Fix:**
- Verify the CSS file exists at the import path.
- For CSS modules: the file must be named `*.module.css` or `*.module.scss`.
- For Tailwind: verify `tailwind.config.js` `content` array includes your source files.

### Dynamic Import Problems

**Error:** `[vite] Failed to analyze dynamic import: X`

**Cause:** Vite cannot statically analyze a dynamic import with a fully variable path.

**Fix:**
```typescript
// WRONG (Vite can't analyze):
const module = await import(path);

// RIGHT (use template literal with known prefix):
const module = await import(`./pages/${pageName}.tsx`);

// Or use explicit mapping:
const modules = {
  dashboard: () => import('./pages/Dashboard'),
  products: () => import('./pages/Products'),
};
```

### Environment Variable Issues

**Error:** `import.meta.env.VITE_X is undefined`

**Cause:** Environment variables must be prefixed with `VITE_` to be exposed to the client. Variables without the prefix are only available on the server/main process.

**Fix:**
- Rename the variable to start with `VITE_`: `VITE_API_URL=http://...`
- Ensure the `.env` file is in the project root (next to `vite.config.ts`).
- Restart the dev server after changing `.env` files.

---

## 3. Electron Builder Errors

### ASAR Errors

**Error:** `Error: ENOENT: no such file or directory, open '...app.asar/...'`

**Cause:** A file that needs to be writable or executable is packed inside ASAR.

**Fix:** Move it to `extraResources` in `electron-builder` config:
```yaml
extraResources:
  - from: "resources/"
    to: "resources/"
    filter:
      - "**/*"
```

### Code Signing Failures

**Error:** `Error: Exit code: 1. Command failed: signtool sign ...`

**Cause:** Certificate not found, expired, or password wrong.

**Fix:**
- Verify certificate file path and password in environment variables.
- Check certificate expiry date.
- For development, set `forceCodeSigning: false` in config.

### Missing Native Modules

**Error:** `Module did not self-register` or `cannot find module`

**Cause:** Native module compiled for wrong Electron/Node ABI version.

**Fix:**
```bash
npx electron-rebuild
# Or for a specific module:
npx electron-rebuild -m node_modules/better-sqlite3
```

### Icon Format Issues

**Error:** `Error: icon is not set` or `icon format not supported`

**Fix:**
- Windows: requires `.ico` file (at least 256x256).
- macOS: requires `.icns` file.
- Linux: requires `.png` file.
- Use electron-icon-builder to generate all formats from a single source.

### NSIS Installer Problems

**Error:** `NSIS error: ...` during build

**Fix:**
- Ensure NSIS is installed (electron-builder downloads it automatically).
- Check that `nsis` config in `electron-builder` is correct.
- For custom NSIS scripts, verify the `.nsh` file syntax.
- Clear the builder cache: delete `~/.cache/electron-builder`.

---

## 4. WASM Bundling

### sql.js WASM File Not Found

**Error:** `RuntimeError: abort(both async and sync fetching of the wasm failed)`

**Cause:** The `sql-wasm.wasm` file is not accessible at the path sql.js expects.

**Fix:**
```typescript
// 1. Copy the WASM file to the output directory:
// vite.config.ts:
import { viteStaticCopy } from 'vite-plugin-static-copy';

export default defineConfig({
  plugins: [
    viteStaticCopy({
      targets: [{
        src: 'node_modules/sql.js/dist/sql-wasm.wasm',
        dest: '.',
      }],
    }),
  ],
});

// 2. Initialize sql.js with the correct path:
import initSqlJs from 'sql.js';

const SQL = await initSqlJs({
  locateFile: (file) => {
    if (app.isPackaged) {
      return path.join(process.resourcesPath, file);
    }
    return path.join(__dirname, file);
  },
});
```

### Public Assets Path

**Cause:** In development, Vite serves from `public/`. In production, assets are in the build output directory. The WASM file path must work in both environments.

**Fix:** Use the `locateFile` callback (shown above) to dynamically resolve the path based on whether the app is packaged.

---

## 5. Import Resolution

### ESM vs CJS Conflicts

**Error:** `SyntaxError: Cannot use import statement in a CommonJS module` or `require() of ES Module ... not supported`

**Cause:** Mixing ES modules (import/export) and CommonJS (require/module.exports) in incompatible ways.

**Fix:**
- For Electron main process: use `"type": "module"` in package.json and ESM syntax, or use CJS consistently.
- For renderer: Vite handles ESM natively; ensure all imports use `import` syntax.
- For problematic packages: use `optimizeDeps.include` in Vite config to pre-bundle them.

### Path Aliases Not Resolving

**Error:** `Module not found: Can't resolve '@/components/Header'`

**Cause:** Path aliases must be configured in BOTH `tsconfig.json` AND `vite.config.ts`.

**Fix:**
```json
// tsconfig.json:
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  }
}
```

```typescript
// vite.config.ts:
import path from 'path';

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
```

### Barrel File Circular Dependencies

**Error:** Variable is `undefined` at import time, or `RangeError: Maximum call stack size exceeded`.

**Cause:** `index.ts` barrel files that re-export from multiple modules can create circular dependency chains: A imports from index, index imports B, B imports A.

**Fix:**
- Import directly from the source file instead of the barrel: `import { X } from './X'` instead of `import { X } from '.'`.
- Break the circular chain by moving shared types/constants to a separate file.
- Use `madge --circular` to detect circular dependencies.

---

## 6. Node.js Version Issues

### Native Module ABI Mismatch

**Error:** `Error: The module was compiled against a different Node.js version`

**Cause:** The native module was compiled with a different Node.js ABI than what Electron uses. Electron bundles its own Node.js version.

**Fix:**
```bash
# Rebuild native modules for Electron:
npx electron-rebuild

# Or specify the Electron version explicitly:
npx electron-rebuild --version 28.0.0

# Check which Node version Electron uses:
npx electron -e "console.log(process.versions.node)"
```

### nvm Version Switching

When switching Node.js versions with nvm, native modules need to be reinstalled:
```bash
nvm use 20
npm install
npx electron-rebuild
```

### Engine Requirements

Some packages specify engine requirements in their `package.json`:
```json
{
  "engines": {
    "node": ">=18.0.0"
  }
}
```

If your Node version is too old, `npm install` may warn or fail. Check with `node --version` and switch if needed.

### Common Version-Specific Issues
- Node 18+: `fetch` is globally available (no need for `node-fetch`).
- Node 20+: some `fs` API behavior changes (symlink handling).
- Electron 28+: uses Node 18.18.x internally.
- Always check `process.versions` in Electron to know the actual Node version being used, not the system Node version.
