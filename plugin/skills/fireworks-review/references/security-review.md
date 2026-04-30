# Security Review Reference — Fireworks Review

Detailed checklist for the **Security** lens. Use this reference when reviewing code for vulnerabilities, credential hygiene, Electron-specific risks, and dependency safety.

---

## OWASP Top 10 — Electron App Focus

### 1. Injection (SQL, Command, IPC)

#### SQL Injection
- **Pattern**: String concatenation in SQL queries
  ```typescript
  // VULNERABLE
  const query = `SELECT * FROM products WHERE name = '${userInput}'`;

  // SAFE: Parameterized query
  const query = `SELECT * FROM products WHERE name = ?`;
  db.prepare(query).get(userInput);
  ```
- Check every `db.run()`, `db.prepare()` for string interpolation
- Even with sql.js, parameterized queries are the only safe approach
- ORDER BY and LIMIT clauses cannot be parameterized — validate against allowlist

#### Command Injection
- **Pattern**: User input in shell commands
  ```typescript
  // VULNERABLE — uses shell, susceptible to injection
  execShell(`convert ${filePath} output.png`);

  // SAFE: Use execFile with argument array (no shell involved)
  execFile('convert', [filePath, 'output.png']);
  ```
- Never use shell-based execution with user-provided strings
- Use `execFile()` or `spawn()` with argument arrays instead
- In this codebase, prefer `execFileNoThrow` from `src/utils/execFileNoThrow.ts`
- Validate file paths against path traversal before passing to any system call

#### IPC Injection
- Renderer can send arbitrary data through IPC channels
- Every `ipcMain.handle()` must validate its arguments
- Never trust data from the renderer — treat it like untrusted user input
- Channel names should follow a strict naming convention (`domain:action`)

### 2. Broken Authentication
- Session tokens stored in plaintext in localStorage or electron-store
- Missing session expiry / token refresh logic
- Hardcoded default passwords or API keys
- Missing rate limiting on authentication endpoints
- Token passed in URL query parameters (logged in server logs, browser history)

### 3. Sensitive Data Exposure
- Credentials logged to console or written to log files
- Sensitive data in error messages returned to renderer
- Database files without encryption at rest
- Temporary files with sensitive data not cleaned up
- Clipboard containing passwords without auto-clear
- Electron DevTools accessible in production builds

### 4. XML External Entities (XXE)
- Parsing XML with external entity resolution enabled
- Using older XML parsers without disabling DTD processing
- In Electron context: less common but relevant for import/export features

### 5. Broken Access Control
- Missing permission checks before performing actions
- Renderer directly accessing main process resources without validation
- IPC handlers that do not verify the sender's identity
- File system access without scope limitations
- Accessing other users' data without authorization checks

### 6. Security Misconfiguration
- DevTools enabled in production (`webPreferences.devTools: true`)
- Debug logging left in production code
- Default credentials not changed
- Unnecessary features enabled (remote module, node integration in renderer)
- CORS headers too permissive
- Missing Content-Security-Policy

### 7. Cross-Site Scripting (XSS)
- Rendering user input as HTML without sanitization
- Using React's raw HTML injection prop without sanitizing content first (always use DOMPurify)
- Electron's `webview` tag loading untrusted content
- Template literals in innerHTML assignments
- URL schemes (`javascript:`, `data:`) in user-controlled links

### 8. Insecure Deserialization
- `JSON.parse()` on untrusted data without validation
- Dynamic code evaluation on data from external sources (CWE-95)
- Deserializing complex objects from untrusted IPC messages
- Pickle/protobuf deserialization without schema validation

### 9. Using Components with Known Vulnerabilities
- Outdated Electron version with known CVEs
- npm packages with published security advisories
- Bundled native modules compiled against old OpenSSL
- Unmaintained dependencies (no updates in 2+ years)

### 10. Insufficient Logging and Monitoring
- Failed authentication attempts not logged
- Database modification operations not audited
- Error details swallowed without logging
- No way to trace a security incident back to its source
- Log injection (user input written directly to logs without sanitization)

---

## CWE Quick Map

| CWE ID | Name | What to Look For |
|--------|------|-----------------|
| CWE-89 | SQL Injection | String concatenation in SQL queries, unparameterized values |
| CWE-79 | XSS | User input rendered as HTML, raw HTML injection, innerHTML |
| CWE-78 | Command Injection | Shell-based execution with user input, shell: true in spawn options |
| CWE-798 | Hardcoded Credentials | API keys, passwords, tokens in source code or config files |
| CWE-22 | Path Traversal | File operations with user-supplied paths without normalization |
| CWE-502 | Unsafe Deserialization | Dynamic code evaluation, JSON.parse on untrusted input without validation |
| CWE-200 | Information Exposure | Stack traces in error responses, sensitive data in logs |
| CWE-352 | CSRF | Missing CSRF tokens on state-changing operations |
| CWE-287 | Improper Authentication | Missing auth checks, authentication bypass paths |
| CWE-862 | Missing Authorization | Actions performed without permission verification |

### Path Traversal Deep Dive (CWE-22)
```typescript
// VULNERABLE: User can access any file
const filePath = path.join(baseDir, userInput);
// userInput = "../../etc/passwd" => escapes baseDir

// SAFE: Resolve and verify containment
const resolved = path.resolve(baseDir, userInput);
if (!resolved.startsWith(path.resolve(baseDir))) {
  throw new Error('Path traversal detected');
}
```
- Always resolve paths to absolute before comparison
- On Windows, check for both `/` and `\` separators
- Watch for URL-encoded path separators (`%2F`, `%5C`)
- Null bytes in paths can truncate path checks in some systems

---

## Electron Security Checklist

### WebPreferences — Non-Negotiable Settings
```typescript
const mainWindow = new BrowserWindow({
  webPreferences: {
    nodeIntegration: false,        // NEVER true in production
    contextIsolation: true,        // ALWAYS true — separates preload from renderer
    sandbox: true,                 // ALWAYS true — restricts renderer capabilities
    webSecurity: true,             // ALWAYS true — enforces same-origin policy
    allowRunningInsecureContent: false,  // NEVER true
    experimentalFeatures: false,   // NEVER true unless specifically needed
    enableBlinkFeatures: '',       // NEVER enable arbitrary Blink features
    webviewTag: false,             // Disable unless explicitly needed
  }
});
```

### Dangerous APIs to Audit
- **Dynamic code evaluation** — Never evaluate strings as code with any external input. This includes all forms: direct evaluation functions, the Function constructor, setTimeout/setInterval with string arguments. All are CWE-95 violations.
- `shell.openExternal()` — MUST validate URL before opening. Never pass user input directly.
  ```typescript
  // VULNERABLE
  shell.openExternal(userProvidedUrl);

  // SAFE: Validate protocol
  const url = new URL(userProvidedUrl);
  if (['https:', 'http:'].includes(url.protocol)) {
    shell.openExternal(userProvidedUrl);
  }
  ```
- `protocol.registerFileProtocol()` — can expose local file system if not restricted
- `ses.setPermissionRequestHandler()` — must deny unnecessary permissions (camera, mic, geolocation)

### Content Security Policy (CSP)
```typescript
// Set CSP in session
session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
  callback({
    responseHeaders: {
      ...details.responseHeaders,
      'Content-Security-Policy': [
        "default-src 'self'",
        "script-src 'self'",
        "style-src 'self' 'unsafe-inline'",  // Tailwind needs unsafe-inline
        "img-src 'self' data: blob:",
        "font-src 'self'",
        "connect-src 'self'",
      ].join('; ')
    }
  });
});
```

### Preload Script Best Practices
- Expose the MINIMUM API surface needed
- Every exposed function should validate its arguments
- Never expose raw Node.js modules (fs, child_process, etc.)
- Use `contextBridge.exposeInMainWorld()` — never assign modules to window directly
- Type the exposed API in both preload and renderer

### Production Build Security
- Remove DevTools in production: `win.webContents.openDevTools()` should be guarded
- Disable navigation to external URLs
- Disable creation of additional windows unless explicitly needed
- Sign the application binary
- Enable auto-updates with signature verification

---

## IPC Validation

### Every Handler Must Validate Input
```typescript
// BAD: No validation
ipcMain.handle('db:query', async (event, query) => {
  return db.run(query); // Arbitrary SQL!
});

// GOOD: Validate and constrain
ipcMain.handle('products:search', async (event, searchTerm: unknown) => {
  if (typeof searchTerm !== 'string' || searchTerm.length > 200) {
    throw new Error('Invalid search term');
  }
  return db.prepare('SELECT * FROM products WHERE name LIKE ?').all(`%${searchTerm}%`);
});
```

### Channel Naming Convention
- Format: `domain:action` (e.g., `products:list`, `auth:login`, `sync:push`)
- Never expose generic channels like `db:query` or `fs:read`
- Each channel does ONE specific thing
- Document all channels in a central type definition file

### Typed Channels End-to-End
```typescript
// shared/ipc-types.ts
interface IpcChannels {
  'products:list': { args: [filters: ProductFilters]; result: Product[] };
  'products:create': { args: [data: CreateProductInput]; result: Product };
  'auth:login': { args: [credentials: LoginInput]; result: AuthResult };
}

// preload.ts — type-safe expose
contextBridge.exposeInMainWorld('api', {
  products: {
    list: (filters: ProductFilters) => ipcRenderer.invoke('products:list', filters),
    create: (data: CreateProductInput) => ipcRenderer.invoke('products:create', data),
  },
  auth: {
    login: (creds: LoginInput) => ipcRenderer.invoke('auth:login', creds),
  }
});
```

### Error Wrapping for IPC
- Never send raw Error objects across IPC (they don't serialize well)
- Create a serializable error envelope: `{ success: false, error: { code: string, message: string } }`
- Never expose stack traces to the renderer in production
- Map internal errors to user-friendly error codes

---

## Credential Hygiene

### No Secrets in Code
- **API keys**: Not in source code, not in config files committed to git
- **Passwords**: Not hardcoded, not in default values, not in comments
- **Tokens**: Not in URLs, not in localStorage (use secure storage)
- **Database passwords**: Use environment variables or secure storage mechanisms
- **Private keys**: Never committed, use OS-level secure credential storage or hardware security modules

### File-Based Secrets
- `.env` files must be in `.gitignore`
- `.env.example` should have placeholder values only (no real secrets)
- Electron apps: use `safeStorage.encryptString()` for locally stored credentials
- Database files: encrypt sensitive columns, not just the whole file

### Environment Variables
- Use `process.env.VAR_NAME` in main process only (not renderer)
- Validate required env vars at startup, fail fast if missing
- Never log environment variables (they often contain secrets)
- Different env var sets for development, staging, production

### Commit History
- If a secret was ever committed, it is compromised — rotate it
- Use git hooks (pre-commit) to scan for secrets before committing
- Tools: `git-secrets`, `detect-secrets`, `gitleaks`

---

## Dependency Risk Assessment

### Known CVEs
- Run `npm audit` regularly
- Check `npm audit --production` for production-only vulnerabilities
- Severity levels: critical > high > moderate > low
- Critical and high CVEs in production dependencies are blockers

### Outdated Packages
- `npm outdated` to check for available updates
- Security patches in minor/patch versions — update promptly
- Major version updates — evaluate breaking changes first
- Pin exact versions in `package.json` for reproducible builds

### Unmaintained Packages
- No commits in 2+ years — consider alternatives
- No response to security issues — high risk
- Single maintainer with no succession plan — bus factor risk
- Check download trends — declining usage may signal abandonment

### Supply Chain Attacks
- Lock file (`package-lock.json`) must be committed
- Verify package integrity with `npm ci` (not `npm install` in CI)
- Review dependencies before adding (check source, maintainers, downloads)
- Use `npx` cautiously — it downloads and runs code without review
- Typosquatting: verify package names carefully (`lodash` vs `l0dash`)

### Transitive Dependencies
- A vulnerability in a transitive dependency (dep of a dep) is still your vulnerability
- Use `npm ls <package>` to find which direct dependency pulls in the vulnerable one
- Override transitive versions with `overrides` in `package.json` when upstream is slow to patch
