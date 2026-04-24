# Error Lookup Table

> 40+ common error messages mapped to causes and fixes.
> Organized by category for fast pattern matching during T0 debugging.

---

## 1. JavaScript Runtime Errors

### TypeError: Cannot read properties of undefined (reading 'X')

**Cause**: Accessing a property on a value that is `undefined`. The variable exists but was never assigned, or a function returned `undefined` instead of an object.

**Common Triggers**:
- Accessing nested properties without null checks: `user.address.street` when `user.address` is undefined
- Array index out of bounds: `items[5]` when array has 3 items
- Async data not yet loaded: component renders before IPC response arrives
- Destructuring from undefined: `const { name } = getProduct()` when getProduct returns undefined

**Fix**:
```typescript
// Option A: Optional chaining
const street = user?.address?.street;

// Option B: Nullish coalescing with default
const street = user?.address?.street ?? 'Unknown';

// Option C: Guard clause
if (!user?.address) {
  console.error('Missing address data for user:', user);
  return null;
}
```

---

### TypeError: X is not a function

**Cause**: Calling something that is not a function. Common when:
- Import is wrong (imported the module instead of the function)
- Method does not exist on the object
- Variable was reassigned from a function to something else
- Default vs named export mismatch

**Fix**:
```typescript
// Check: is it a default vs named export issue?
import myFunc from './module';    // default export
import { myFunc } from './module'; // named export

// Check: does the method exist on the prototype?
console.log(typeof obj.methodName); // should be 'function'
```

---

### RangeError: Maximum call stack size exceeded

**Cause**: Infinite recursion. A function calls itself (directly or indirectly) without a base case, or a circular dependency causes infinite module loading.

**Common Triggers**:
- Recursive function missing base case
- useEffect triggering state change that triggers the same useEffect
- Circular imports via barrel files (index.ts)
- JSON.stringify on object with circular references

**Fix**:
```typescript
// Add base case to recursion
function traverse(node: TreeNode): void {
  if (!node) return; // BASE CASE
  traverse(node.left);
  traverse(node.right);
}

// For circular references in JSON:
JSON.stringify(obj, (key, value) => {
  if (key === 'parent') return undefined; // skip circular ref
  return value;
});

// For circular imports: use `madge --circular src/` to detect
```

---

### TypeError: Assignment to constant variable

**Cause**: Attempting to reassign a `const` variable. Use `let` if the variable needs to change.

**Fix**: Change `const` to `let`, or restructure to avoid reassignment.

---

### SyntaxError: Unexpected token

**Cause**: JavaScript parser encountered invalid syntax. Often caused by:
- Missing comma, bracket, or parenthesis
- Using TypeScript syntax in a .js file
- JSON.parse on invalid JSON string
- Template literal with unescaped backtick

**Fix**: Check the exact line and character position in the error. Look for missing punctuation.

---

### TypeError: Cannot convert undefined or null to object

**Cause**: Passing null/undefined to Object.keys(), Object.entries(), Object.assign(), or spread operator.

**Fix**:
```typescript
const keys = Object.keys(obj ?? {});
const merged = { ...defaults, ...(overrides ?? {}) };
```

---

### ReferenceError: X is not defined

**Cause**: Using a variable that was never declared in the current scope. Different from "cannot read properties of undefined" — the variable itself does not exist.

**Fix**: Check spelling, check imports, check scope (block scope vs function scope).

---

## 2. Electron Errors

### Error: ERR_IPC_CHANNEL_CLOSED

**Cause**: Attempting to send a message over an IPC channel that has been closed. The BrowserWindow was destroyed, or the renderer process crashed.

**Fix**:
```typescript
// Check if window still exists before sending
if (mainWindow && !mainWindow.isDestroyed()) {
  mainWindow.webContents.send('channel', data);
}
```

---

### Error: contextBridge API can only be used when contextIsolation is enabled

**Cause**: Using `contextBridge.exposeInMainWorld` but `contextIsolation` is set to `false` in BrowserWindow options.

**Fix**: Set `contextIsolation: true` in webPreferences (this is the default and recommended setting).

---

### window.api is undefined

**Cause**: The preload script failed to load or threw an error during execution. The `contextBridge.exposeInMainWorld` call never ran.

**Diagnostic Steps**:
1. Check the preload path in BrowserWindow config — is the path correct?
2. Check main process terminal output — preload errors appear there, not in renderer
3. Verify the preload file exists at the specified path (dev vs prod paths differ)
4. Check for syntax errors or import errors in preload.ts

**Fix**: Correct the preload path. In Vite setups, the preload is often built to a different location than the source.

---

### Error: Cannot use import statement in preload script

**Cause**: Preload scripts in Electron must be CommonJS (require/module.exports) unless you configure a bundler to compile them.

**Fix**: Use a bundler (Vite/esbuild) to compile the preload script, or use `require()` syntax.

---

### Error: An object could not be cloned (DataCloneError)

**Cause**: Passing non-cloneable data through IPC. Functions, Symbols, class instances with methods, and circular references cannot be sent over IPC.

**Fix**: Convert to plain objects before sending:
```typescript
// Strip non-cloneable properties
const safeData = JSON.parse(JSON.stringify(data));
// Or explicitly pick only needed fields
const safeData = { id: item.id, name: item.name, price: item.price };
```

---

### Error: Module not found: electron

**Cause**: Trying to import `electron` in renderer code. Electron modules are only available in main process and preload.

**Fix**: Access Electron APIs through the preload bridge, never import electron directly in renderer.

---

### Error: electron.dialog is not a function / Cannot read properties of undefined (reading 'showOpenDialog')

**Cause**: Trying to use main-process-only APIs (dialog, app, BrowserWindow) from the renderer process.

**Fix**: Create an IPC handler in main process, call it from renderer via preload bridge.

---

## 3. TypeScript Compile Errors

### TS2304: Cannot find name 'X'

**Cause**: Identifier used but never imported or declared.
**Fix**: Add import. Check tsconfig includes. Check spelling.

### TS2345: Argument of type 'X' is not assignable to parameter of type 'Y'

**Cause**: Wrong argument type passed to function.
**Fix**: Convert type, update function signature, or add type guard. See `references/build-errors.md`.

### TS2322: Type 'X' is not assignable to type 'Y'

**Cause**: Value might be undefined/null but target requires definite type.
**Fix**: Add default value (`??`), narrow with check, or mark property optional. See `references/build-errors.md`.

### TS7006: Parameter 'X' implicitly has an 'any' type

**Cause**: Missing type annotation with noImplicitAny enabled.
**Fix**: Add explicit type annotation to parameter.

### TS2307: Cannot find module 'X'

**Cause**: File not found, wrong path, missing type declarations, or unconfigured path alias.
**Fix**: Check file exists, check tsconfig paths, install @types package. See `references/build-errors.md`.

### TS2769: No overload matches this call

**Cause**: Arguments do not match any function overload signature.
**Fix**: Check overload signatures, provide explicit generic type parameter.

### TS18048: 'X' is possibly undefined

**Cause**: Accessing a value that TypeScript knows could be undefined (strict null checks).
**Fix**:
```typescript
// Narrow with a check
if (value !== undefined) { useValue(value); }
// Or provide default
const safe = value ?? defaultValue;
```

---

## 4. Database Errors (SQLite / sql.js)

### SQLITE_CONSTRAINT: UNIQUE constraint failed

**Cause**: Inserting a row with a duplicate value in a UNIQUE column.
**Fix**:
```sql
-- Use INSERT OR REPLACE to upsert:
INSERT OR REPLACE INTO products (sku, name, price) VALUES (?, ?, ?);
-- Or check existence first:
INSERT INTO products (sku, name, price)
  SELECT ?, ?, ? WHERE NOT EXISTS (SELECT 1 FROM products WHERE sku = ?);
```

---

### SQLITE_CONSTRAINT: FOREIGN KEY constraint failed

**Cause**: Inserting a row that references a non-existent parent row, or deleting a parent that has children.
**Fix**: Insert parent first, or use CASCADE on delete. Check that the referenced ID exists.

---

### SQLITE_BUSY: database is locked

**Cause**: Another process or thread is writing to the database. With sql.js (in-memory), this usually means concurrent async operations are colliding.
**Fix**: Use an async mutex to serialize database access. See `references/bug-patterns.md` Section 5.

---

### SQLITE_CORRUPT: database disk image is malformed

**Cause**: Database file was corrupted, usually by:
- Incomplete write (crash during save)
- Writing to the file from multiple processes simultaneously
- File system corruption

**Fix**:
```typescript
// Restore from backup
// Or try to recover data:
// .dump command in sqlite3 CLI can sometimes extract data from corrupt DBs
// Prevention: always save to temp file first, then rename
```

---

### SQLITE_ERROR: no such column: X

**Cause**: Column name in query does not match the schema.
**Fix**: Check exact column names in the CREATE TABLE statement. SQL column names are case-insensitive but JavaScript property access is not.

---

### SQLITE_ERROR: no such table: X

**Cause**: Table not created yet, or database was not initialized.
**Fix**: Run migration/initialization before queries. Check database path points to correct file.

---

## 5. Vite / Build Errors

### Failed to resolve import "X" from "Y"

**Cause**: Vite cannot find the imported module. The file does not exist, the path is wrong, or the package is not installed.
**Fix**: Check file path, check node_modules, run `npm install`.

---

### import.meta.env.VITE_X is undefined

**Cause**: Environment variable not prefixed with `VITE_`, or `.env` file not in project root, or dev server not restarted after .env change.
**Fix**: Prefix with `VITE_`, place `.env` next to `vite.config.ts`, restart dev server.

---

### [vite] Internal server error: Failed to parse source

**Cause**: Syntax error in source file that Vite cannot parse.
**Fix**: Check the file mentioned in the error. Run `tsc --noEmit` to get precise location.

---

### Dynamic import() not supported for target

**Cause**: Vite cannot statically analyze a fully dynamic import path.
**Fix**: Use template literal with known prefix: `import('./pages/${name}.tsx')` instead of `import(path)`.

---

### [vite] Pre-transform error: Failed to load

**Cause**: A dependency failed to pre-bundle. Common with CJS packages.
**Fix**: Add to `optimizeDeps.include` in vite.config.ts, or exclude if it should not be pre-bundled.

---

### WASM: both async and sync fetching of the wasm failed

**Cause**: sql.js WASM file not found at expected path.
**Fix**: Copy WASM file to output directory and configure `locateFile`. See `references/build-errors.md` Section 4.

---

## 6. React Errors

### Warning: Each child in a list should have a unique "key" prop

**Cause**: Rendering a list without unique keys.
**Fix**: Add `key={item.id}` to list items. Never use array index as key if list can reorder.

---

### Warning: Cannot update a component while rendering a different component

**Cause**: Calling setState of component A inside the render of component B.
**Fix**: Move the state update to a useEffect.

---

### Error: Too many re-renders

**Cause**: Component triggers a state update on every render, creating an infinite loop.
**Fix**: Check for `setState()` calls that are not inside useEffect, event handlers, or conditionals.

---

### Error: Rendered fewer/more hooks than during the previous render

**Cause**: Hooks called conditionally or inside loops.
**Fix**: Hooks must always be called in the same order, at the top level of the component.

---

### Warning: A component is changing an uncontrolled input to be controlled

**Cause**: Input value changed from `undefined` to a defined value.
**Fix**: Initialize the state with an empty string instead of undefined:
```typescript
const [value, setValue] = useState(''); // Not useState() or useState(undefined)
```

---

## 7. Node.js / System Errors

### ENOENT: no such file or directory

**Cause**: File or directory does not exist at the specified path.
**Fix**: Check path spelling, use `path.join()`, check `app.getPath()` for correct directories.

---

### EACCES: permission denied

**Cause**: No permission to read/write the file. On Windows, the file may be locked by another process.
**Fix**: Check file permissions. Close other programs using the file. Use a different directory.

---

### EPERM: operation not permitted

**Cause**: On Windows, often caused by trying to modify a read-only file or a file inside Program Files.
**Fix**: Use `app.getPath('userData')` for writable storage, not the app installation directory.

---

### ERR_MODULE_NOT_FOUND

**Cause**: ES module import failed. The module does not exist or the file extension is missing.
**Fix**: Add explicit file extension in ESM: `import './utils.js'` (not `import './utils'`).

---

### ENOMEM: not enough memory

**Cause**: Process ran out of memory. Possible memory leak or processing too much data at once.
**Fix**: Check for memory leaks. Process data in chunks. Increase Node memory limit: `--max-old-space-size=4096`.
