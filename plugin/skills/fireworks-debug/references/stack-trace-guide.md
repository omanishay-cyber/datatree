# Stack Trace Guide — How to Read Stack Traces

> A stack trace is the single most important piece of debugging information.
> Learning to read it correctly solves 50% of bugs immediately.

---

## 1. Anatomy of a Stack Trace

```
TypeError: Cannot read properties of undefined (reading 'name')
    at ProductCard (src/renderer/components/ProductCard.tsx:23:18)
    at renderWithHooks (node_modules/react-dom/cjs/react-dom.development.js:16305:18)
    at mountIndeterminateComponent (node_modules/react-dom/cjs/react-dom.development.js:20074:13)
    at beginWork (node_modules/react-dom/cjs/react-dom.development.js:21587:16)
    at HTMLUnknownElement.callCallback (node_modules/react-dom/cjs/react-dom.development.js:4164:14)
```

### Parts:
- **Line 1**: Error type and message — `TypeError: Cannot read properties of undefined (reading 'name')`
- **Line 2**: The TOP frame — where the error occurred — `ProductCard.tsx:23:18`
- **Lines 3+**: The call chain — who called whom, deepest first
- **File:Line:Column**: Exact location — `src/renderer/components/ProductCard.tsx:23:18` means file, line 23, column 18

---

## 2. Reading Rules

### Rule 1: Find the First Frame in YOUR Code
Skip `node_modules` and framework internals. The first frame in YOUR source code is where to start investigating.

In the example above: `ProductCard.tsx:23:18` — this is your code. Everything below it is React internals.

### Rule 2: Read Top-to-Bottom for Call Chain
The top frame is where the error happened. Each frame below is the caller:
- ProductCard called something on line 23 that failed
- renderWithHooks called ProductCard (React rendering the component)
- The chain continues down to the event that triggered the render

### Rule 3: The Error Message Tells You WHAT, the Stack Tells You WHERE
- `Cannot read properties of undefined (reading 'name')` = some variable is undefined and you tried to access `.name`
- `ProductCard.tsx:23` = this happened on line 23 of ProductCard
- Go to that line and find what could be undefined

### Rule 4: Multiple YOUR-Code Frames Show the Call Path
```
at saveProduct (src/main/handlers/products.ts:45:12)
at handleIPC (src/main/ipc-bridge.ts:23:8)
at processRequest (src/main/server.ts:12:5)
```
Read bottom-to-top for the execution flow: processRequest called handleIPC called saveProduct, which failed at line 45.

---

## 3. Electron Multi-Process Traces

Electron has separate processes, each with their own stack traces:

### Main Process Stack Trace
Appears in the terminal where `npm run dev` runs. Prefixed with nothing or `[main]`.
```
Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: products.sku
    at Database.run (src/main/database.ts:78:12)
    at saveProduct (src/main/handlers/products.ts:45:20)
```

### Renderer Process Stack Trace
Appears in the browser DevTools console. Often starts with React components.
```
TypeError: Cannot read properties of undefined (reading 'price')
    at ProductCard (src/renderer/components/ProductCard.tsx:23:18)
    at renderWithHooks (...)
```

### Preload Script Stack Trace
May appear in EITHER terminal (main process logs) or DevTools, depending on the error type.

### Cross-Process Debugging
When an error crosses process boundaries (renderer calls IPC, main handler fails):
1. The renderer gets a generic "Error invoking remote method" message
2. The REAL error and stack trace are in the main process terminal
3. Always check BOTH locations

---

## 4. Source Map Issues

### Problem: Stack Trace Shows Compiled Code
```
at Object.render (bundle.js:45234:12)
```
This means source maps are not working. You need to see the original TypeScript file and line.

### Fixes:
1. Enable source maps in tsconfig.json:
```json
{ "compilerOptions": { "sourceMap": true } }
```

2. Enable source maps in Vite:
```typescript
// vite.config.ts
export default defineConfig({
  build: { sourcemap: true }
});
```

3. For production debugging, use `sourcemap: 'hidden'` to generate maps without exposing them in the app.

### Problem: Line Numbers Are Wrong
Source maps can be slightly off, especially with decorators or complex transforms. If the line number seems wrong:
- Check 2-3 lines above and below the reported line
- The column number is often more accurate than the line number

---

## 5. Async Stack Traces

### Problem: Stack Trace Stops at await
```
TypeError: Cannot read properties of null (reading 'id')
    at getProduct (src/main/handlers.ts:23:15)
```
No caller information — who called getProduct?

### Fix: Enable Async Stack Traces
Modern V8 (Node 12+, Electron 6+) captures async stack traces by default. If they are missing:
```bash
# Enable with flag:
node --async-stack-traces app.js
```

With async traces, you see the full chain:
```
TypeError: Cannot read properties of null (reading 'id')
    at getProduct (src/main/handlers.ts:23:15)
    at async handleRequest (src/main/ipc-bridge.ts:45:20)
    at async ipcMain.handle (src/main/setup.ts:12:5)
```

### Understanding Async Boundaries
`async` keyword in the trace means the function was called across an await boundary. The actual caller may be in a completely different timing context.

---

## 6. Common Signatures Mapped to Causes

| Stack Signature | Likely Cause |
|----------------|-------------|
| Error in `renderWithHooks` | React component threw during render |
| Error in `commitHookEffectListMount` | useEffect callback threw |
| Error in `flushSync` | State update during render |
| Error in `ipcMain.handle` | IPC handler threw |
| Error in `contextBridge` | Preload script error |
| Error in `Database.run` / `Database.prepare` | SQL error |
| Error in `JSON.parse` | Invalid JSON string |
| Error in `fs.readFileSync` / `fs.readFile` | File system error |
| `at Object.<anonymous>` at top | Top-level script error (module load time) |
| `at new Promise` then nothing | Unhandled promise rejection |
| `at Timeout._onTimeout` | Error inside setTimeout/setInterval |
| `at EventEmitter.emit` | Error in event handler |
| Stack trace is empty | Error was swallowed and re-thrown without stack |

---

## 7. Preserving Stack Traces

When catching and re-throwing errors, preserve the original stack:

```typescript
// BAD: loses original stack
try {
  await riskyOperation();
} catch (error) {
  throw new Error('Operation failed'); // New stack starts here
}

// GOOD: preserves original stack
try {
  await riskyOperation();
} catch (error) {
  throw new Error('Operation failed', { cause: error });
}

// Access the original:
catch (error) {
  console.error('Wrapper:', error.message);
  console.error('Original:', error.cause?.stack);
}
```

---

## 8. Quick Diagnostic from Stack Trace

When you see a stack trace, answer these questions in order:

1. **What is the error type and message?** (Line 1)
2. **Where in MY code did it happen?** (First non-node_modules frame)
3. **What was the call chain?** (Read remaining YOUR-code frames bottom-to-top)
4. **Which process?** (Terminal = main, DevTools = renderer)
5. **What is the variable that is wrong?** (Error message tells you which property access failed)
6. **Go to that exact line** — read the code, understand what should be there vs what is there
