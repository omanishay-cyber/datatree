# Advanced Debugging — Complete Reference

> Compound launch configs, preLaunchTask, postDebugTask, environment variables,
> source maps, remote debugging, and multi-process debugging.

---

## Compound Launch Configurations

Compound configs launch multiple debug sessions simultaneously. Essential for Electron (main + renderer) and microservice architectures.

### Electron: Main + Renderer

```json
{
  "compounds": [
    {
      "name": "Electron: Main + Renderer",
      "configurations": [
        "Electron: Main Process",
        "Electron: Renderer (Chrome)"
      ],
      "stopAll": true,
      "preLaunchTask": "Build All"
    }
  ]
}
```

**Key properties:**
- `stopAll: true` — stopping one session stops all
- `preLaunchTask` — runs BEFORE any config in the compound starts
- Configs launch in order but do NOT wait for each other

### Flutter + Backend API

```json
{
  "compounds": [
    {
      "name": "Full Stack: Flutter + API",
      "configurations": [
        "Flutter Debug (Chrome)",
        "Node API Server"
      ],
      "stopAll": true
    }
  ]
}
```

---

## preLaunchTask and postDebugTask

### preLaunchTask

Runs a task from `tasks.json` BEFORE the debug session starts. The debug session waits for the task to complete (unless the task is `isBackground: true`).

```json
{
  "name": "Electron: Main Process",
  "type": "node",
  "request": "launch",
  "preLaunchTask": "Vite Build (Main)",
  "runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron",
  "runtimeArgs": [".", "--inspect=5858"]
}
```

**Common preLaunchTasks:**
- `tsc --noEmit` — type-check before debugging
- `vite build` — build before launching Electron
- `flutter pub get` — ensure dependencies before Flutter debug
- `docker compose up -d` — start dependent services

### postDebugTask

Runs AFTER the debug session ends. Useful for cleanup.

```json
{
  "name": "Integration Test",
  "type": "node",
  "request": "launch",
  "program": "${workspaceFolder}/test/integration.ts",
  "postDebugTask": "Docker Cleanup"
}
```

**Common postDebugTasks:**
- Stop Docker containers
- Clean temporary files
- Reset test database
- Kill background processes (NOT node.exe on Windows)

### Background Tasks as preLaunchTask

For tasks that run indefinitely (like dev servers), mark them as background with a problem matcher that detects when they are "ready":

```json
{
  "label": "Vite Dev Server",
  "type": "shell",
  "command": "npx vite",
  "isBackground": true,
  "problemMatcher": {
    "pattern": { "regexp": "^$" },
    "background": {
      "activeOnStart": true,
      "beginsPattern": "VITE",
      "endsPattern": "ready in"
    }
  }
}
```

The debug session starts when the `endsPattern` matches in the task output.

---

## Environment Variables

### Inline env

```json
{
  "name": "Debug with env",
  "type": "node",
  "request": "launch",
  "env": {
    "NODE_ENV": "development",
    "DB_PATH": "${workspaceFolder}/data/dev.db",
    "LOG_LEVEL": "debug",
    "PORT": "3000"
  }
}
```

### envFile (recommended for secrets)

```json
{
  "name": "Debug with .env",
  "type": "node",
  "request": "launch",
  "envFile": "${workspaceFolder}/.env.development"
}
```

**.env.development** (gitignored):
```
DATABASE_URL=sqlite:./data/dev.db
API_KEY=dev-key-not-real
ENCRYPTION_KEY=local-dev-key
```

### Flutter --dart-define

```json
{
  "name": "Flutter Staging",
  "type": "dart",
  "request": "launch",
  "args": [
    "--dart-define=ENV=staging",
    "--dart-define=API_URL=https://staging.api.example.com",
    "--dart-define-from-file=.env.staging"
  ]
}
```

**Security rule:** NEVER put real secrets in `launch.json`. Use `envFile` or `--dart-define-from-file` pointing to gitignored files.

---

## Source Maps

### TypeScript/Vite Source Maps

Ensure `tsconfig.json` has:
```json
{
  "compilerOptions": {
    "sourceMap": true,
    "declarationMap": true
  }
}
```

Ensure `vite.config.ts` has:
```typescript
export default defineConfig({
  build: {
    sourcemap: true,  // or 'inline' for dev
  },
});
```

### Launch config source map settings

```json
{
  "sourceMaps": true,
  "outFiles": [
    "${workspaceFolder}/dist/**/*.js"
  ],
  "resolveSourceMapLocations": [
    "${workspaceFolder}/**",
    "!**/node_modules/**"
  ],
  "sourceMapPathOverrides": {
    "webpack:///./src/*": "${workspaceFolder}/src/*",
    "webpack:///src/*": "${workspaceFolder}/src/*",
    "meteor://app/*": "${workspaceFolder}/*"
  }
}
```

### Common Source Map Issues

| Problem | Cause | Fix |
|---|---|---|
| Breakpoints unbound (gray) | Source maps not found | Check `outFiles` paths match actual output |
| Breakpoints in wrong location | Stale source maps | Clean build (`rm -rf dist`) and rebuild |
| Can't step into node_modules | Excluded by default | Remove from `resolveSourceMapLocations` exclusion |
| TypeScript shows compiled JS | Missing `sourceMap: true` | Add to `tsconfig.json` |

---

## Remote Debugging

### Attach to Remote Node Process

```json
{
  "name": "Attach to Remote",
  "type": "node",
  "request": "attach",
  "address": "192.168.1.100",
  "port": 9229,
  "localRoot": "${workspaceFolder}",
  "remoteRoot": "/app",
  "sourceMaps": true
}
```

Start the remote process with:
```bash
node --inspect=0.0.0.0:9229 dist/main.js
```

### Attach to Remote Chrome (Electron Renderer)

```json
{
  "name": "Attach to Remote Electron Renderer",
  "type": "chrome",
  "request": "attach",
  "address": "192.168.1.100",
  "port": 9222,
  "webRoot": "${workspaceFolder}/src/renderer"
}
```

Start Electron with:
```bash
electron . --remote-debugging-port=9222 --remote-debugging-address=0.0.0.0
```

### SSH Tunnel Debugging

When the remote machine is behind a firewall:

```bash
ssh -L 9229:localhost:9229 user@remote-machine
```

Then use a local attach config pointing to `localhost:9229`.

---

## Conditional Breakpoints and Logpoints

### Conditional Breakpoints

Right-click the gutter > "Add Conditional Breakpoint":

**Expression examples:**
- `items.length > 100` — pause when list is large
- `user.role === 'admin'` — pause only for admin users
- `error instanceof TypeError` — pause on specific error type
- `i % 1000 === 0` — pause every 1000th iteration

### Hit Count Breakpoints

Right-click gutter > "Add Conditional Breakpoint" > "Hit Count":
- `5` — pause on 5th hit
- `>10` — pause after 10th hit
- `%3` — pause every 3rd hit

### Logpoints

Right-click gutter > "Add Logpoint":

**Format:** Use `{expression}` for interpolation:
- `User logged in: {user.name} at {new Date().toISOString()}`
- `Query returned {results.length} rows in {elapsed}ms`
- `State transition: {prevState} -> {nextState}`

Logpoints do NOT pause execution. They print to the Debug Console.

---

## Exception Breakpoints

### VS Code Exception Settings

In the Debug panel > BREAKPOINTS section:

| Setting | Effect |
|---|---|
| All Exceptions | Pause on every throw (noisy but thorough) |
| Uncaught Exceptions | Pause only on unhandled exceptions (recommended default) |
| User Uncaught Exceptions | Pause only on your code's unhandled exceptions |

### Flutter Exception Breakpoints

```json
{
  "name": "Flutter Debug",
  "type": "dart",
  "request": "launch",
  "program": "lib/main.dart",
  "dart.debugExternalPackageLibraries": false,
  "dart.debugSdkLibraries": false
}
```

Setting `debugExternalPackageLibraries: false` prevents breaking in package code.

### Dart-specific: Breaking on Specific Exception Types

In VS Code Debug Console during a Flutter session:
```
// Break on specific exception
debugger;  // in code

// Or use conditional breakpoint:
// Condition: e is FormatException
```

---

## Debug Console Evaluation

During a paused debug session, the Debug Console allows expression evaluation:

### Node.js / TypeScript
```javascript
// Inspect variables
user.permissions
items.filter(i => i.active).length
JSON.stringify(config, null, 2)

// Modify state (careful!)
user.name = "Test User"

// Call functions
await database.query("SELECT count(*) FROM users")
```

### Dart / Flutter
```dart
// Inspect state
ref.read(userProvider)
context.mounted
MediaQuery.of(context).size

// Evaluate expressions
items.where((e) => e.isActive).length
jsonEncode(user.toJson())
```

---

## Multi-Process Debugging Checklist

When debugging Electron (main + renderer) or full-stack (frontend + backend):

1. **Set up compound config** with all processes
2. **Use `stopAll: true`** so stopping one stops all
3. **Assign different inspect ports** to each process (5858, 9229, etc.)
4. **Use the dropdown** in the Debug toolbar to switch between sessions
5. **Set breakpoints in both** main and renderer code before launching
6. **Check CALL STACK panel** — it shows all active sessions with their frames
7. **Use separate Debug Console** instances for each process (dropdown at top of console)

---

## Troubleshooting

| Issue | Diagnostic | Fix |
|---|---|---|
| Breakpoints are gray/unbound | Source maps not loaded | Check `outFiles`, rebuild, verify `.map` files exist |
| "Cannot connect to runtime" | Wrong port or process not started | Verify `--inspect` port matches config |
| Breakpoints hit wrong line | Stale build output | Clean and rebuild (`rm -rf dist && npm run build`) |
| Variables show "undefined" | Optimized away in release | Debug in development mode only |
| Electron renderer won't attach | DevTools port blocked | Use `--remote-debugging-port=9222` flag |
| Flutter won't attach | Observatory not running | Use `flutter run` with `--observatory-port` |
| preLaunchTask never completes | Background task missing matcher | Add `isBackground: true` with proper `problemMatcher` |
| Can't evaluate in Debug Console | Not paused at breakpoint | Set a breakpoint first, then evaluate |
