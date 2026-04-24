# CWE Top 25 Detection Patterns

Complete detection patterns, fix templates, severity levels, and Electron-specific variants for the CWE Top 25 Most Dangerous Software Weaknesses.

---

## Critical Severity (Blocks Commits)

### CWE-79: Cross-site Scripting (XSS)

**Detection patterns**:
```
dangerouslySetInnerHTML
innerHTML\s*=
document\.write\(
document\.writeln\(
\.insertAdjacentHTML\(
v-html=
{@html
outerHTML\s*=
```

**Fix pattern**: Use React's built-in escaping (JSX text interpolation). For cases where raw HTML is genuinely needed, sanitize with DOMPurify:
```typescript
import DOMPurify from 'dompurify';
const clean = DOMPurify.sanitize(dirtyHTML);
// Then use: <div dangerouslySetInnerHTML={{ __html: clean }} />
```

**Electron-specific**: In Electron, XSS in the renderer can escalate to RCE if `nodeIntegration` is enabled or `contextIsolation` is disabled. Always verify these settings alongside XSS findings.

---

### CWE-89: SQL Injection

**Detection patterns**:
```
`.*\$\{.*\}.*`\s*\)           # Template literals in SQL calls
'.*'\s*\+\s*                   # String concatenation in queries
\.run\(\s*`                    # sql.js .run() with template literal
\.exec\(\s*`                   # sql.js .exec() with template literal
\.all\(\s*`                    # better-sqlite3 .all() with template literal
\.get\(\s*`                    # .get() with template literal
\.prepare\(\s*`.*\$\{          # .prepare() with interpolated values
```

**Fix pattern**: Always use parameterized queries:
```typescript
// VULNERABLE
db.run(`INSERT INTO products (name) VALUES ('${userInput}')`);

// SAFE
db.run('INSERT INTO products (name) VALUES (?)', [userInput]);
```

**Electron-specific**: sql.js and better-sqlite3 both support parameterized queries. There is never a reason to concatenate user input into SQL strings.

---

### CWE-78: OS Command Injection

**Detection patterns**:
```
child_process\.exec\(
child_process\.execSync\(
execSync\(.*\$\{
exec\(.*\$\{
shell:\s*true
require\(['"]child_process['"]\)
```

**Fix pattern**: Use `execFile` or `spawn` with argument arrays instead of shell strings:
```typescript
// VULNERABLE
exec(`convert ${userFilePath} output.png`);

// SAFE
execFile('convert', [userFilePath, 'output.png']);
```

---

### CWE-77: Command Injection

**Detection patterns**:
```
exec\(`.*\$\{
execSync\(`.*\$\{
spawn\(.*shell:\s*true
```

**Fix pattern**: Same as CWE-78 — never pass user input through a shell. Use argument arrays with `spawn` or `execFile`.

---

### CWE-94: Code Injection

**Detection patterns**:
```
\beval\(
new\s+Function\(
vm\.runInNewContext\(
vm\.runInThisContext\(
vm\.compileFunction\(
setTimeout\(\s*['"`]      # setTimeout with string argument
setInterval\(\s*['"`]     # setInterval with string argument
```

**Fix pattern**: Remove all uses of `eval()` and `new Function()`. Use JSON.parse for data, proper parsers for expressions, and function references for callbacks:
```typescript
// VULNERABLE
eval(userExpression);

// SAFE — if parsing math expressions
import { evaluate } from 'mathjs';
evaluate(userExpression); // sandboxed math parser
```

---

### CWE-287: Improper Authentication

**Detection patterns**:
```
password\s*[:=]\s*['"](?!.*\bprocess\.env\b)
apiKey\s*[:=]\s*['"]
api_key\s*[:=]\s*['"]
secret\s*[:=]\s*['"]
token\s*[:=]\s*['"][A-Za-z0-9]
authorization.*Bearer\s+[A-Za-z0-9]
```

**Fix pattern**: Never hardcode credentials. Use environment variables or Electron's `safeStorage`:
```typescript
// VULNERABLE
// const API_KEY ...hardcoded literal would go here (REDACTED for docs)...

// SAFE
const API_KEY = process.env.API_KEY;
// Or for desktop apps:
const key = safeStorage.decryptString(fs.readFileSync(keyPath));
```

---

### CWE-798: Hardcoded Credentials

**Detection patterns**:
```
(?i)password\s*=\s*['"][^'"]+['"]
(?i)passwd\s*=\s*['"][^'"]+['"]
(?i)api_?key\s*=\s*['"][A-Za-z0-9]{16,}['"]
(?i)secret\s*=\s*['"][^'"]+['"]
(?i)private_?key\s*=\s*['"]
-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----
-----BEGIN\s+CERTIFICATE-----
```

**Fix pattern**: Extract all secrets to environment variables or OS keychain. Add patterns to `.gitignore` and use git hooks to prevent accidental commits.

---

### CWE-502: Deserialization of Untrusted Data

**Detection patterns**:
```
JSON\.parse\(.*\)(?!.*schema|.*validate|.*zod|.*parse)
deserialize\(
unserialize\(
pickle\.loads\(
yaml\.load\((?!.*Loader)
```

**Fix pattern**: Always validate parsed data against a schema:
```typescript
import { z } from 'zod';

const ProductSchema = z.object({
  name: z.string().max(200),
  price: z.number().positive(),
});

// SAFE — validates structure and types
const product = ProductSchema.parse(JSON.parse(rawInput));
```

---

### CWE-269: Improper Privilege Management

**Detection patterns** (Electron-specific):
```
nodeIntegration:\s*true
contextIsolation:\s*false
webSecurity:\s*false
allowRunningInsecureContent:\s*true
experimentalFeatures:\s*true
sandbox:\s*false
enableRemoteModule:\s*true
```

**Fix pattern**: See the Electron Hardening Checklist in `electron-security.md`. Every flag above must be its secure value.

---

### CWE-787 / CWE-119: Out-of-bounds Write / Buffer Overflow

**Detection patterns**:
```
Buffer\.allocUnsafe\(
Buffer\.allocUnsafeSlow\(
new\s+Buffer\(
buffer\[.*\]\s*=         # Direct buffer index write without bounds check
```

**Fix pattern**: Use `Buffer.alloc()` (zero-filled) instead of `Buffer.allocUnsafe()`. Always check buffer bounds before write operations.

---

## High Severity (Must Fix Before Merge)

### CWE-20: Improper Input Validation

**Detection patterns**:
```
ipcMain\.handle\(\s*['"].*['"]\s*,\s*(?:async\s+)?\(\s*(?:event|_)\s*,\s*\w+\s*\)\s*=>  # IPC handler — check if body has validation
ipcMain\.on\(\s*['"]
req\.body\.                # Direct access to request body without validation
req\.params\.              # Direct access to URL params without validation
req\.query\.               # Direct access to query params without validation
```

**Fix pattern**: Add Zod validation at every entry point:
```typescript
import { z } from 'zod';

const AddProductSchema = z.object({
  name: z.string().min(1).max(200),
  price: z.number().positive().max(999999.99),
  quantity: z.number().int().nonnegative(),
});

ipcMain.handle('product:add', async (_event, data: unknown) => {
  const validated = AddProductSchema.parse(data);
  // Use validated.name, validated.price, etc.
});
```

---

### CWE-22: Path Traversal

**Detection patterns**:
```
path\.join\(.*,\s*\w+\)(?!.*(?:resolve|normalize|startsWith))
\.\.\/
\.\.\\
req\.params.*path
readFile\(.*\+
readFileSync\(.*\+
```

**Fix pattern**: Validate that the resolved path stays within the allowed base directory:
```typescript
import path from 'path';

function safePath(basePath: string, userInput: string): string {
  const resolved = path.resolve(basePath, userInput);
  if (!resolved.startsWith(path.resolve(basePath))) {
    throw new Error('Path traversal detected');
  }
  return resolved;
}
```

---

### CWE-862: Missing Authorization

**Detection patterns**:
```
ipcMain\.handle\(      # Check each handler for authorization logic
ipcMain\.on\(          # Check each listener for authorization logic
```

**Fix pattern**: Wrap IPC handlers with authorization middleware:
```typescript
function requireAuth(handler: IpcHandler): IpcHandler {
  return async (event, ...args) => {
    const isAuthed = await checkAuth(event.sender);
    if (!isAuthed) throw new Error('Unauthorized');
    return handler(event, ...args);
  };
}
```

---

### CWE-434: Unrestricted Upload

**Detection patterns**:
```
dialog\.showOpenDialog\((?!.*filters)
<input\s+type=['"]file['"](?!.*accept)
multer\(\s*\)             # multer without file filter
```

**Fix pattern**: Always specify allowed file types:
```typescript
const result = await dialog.showOpenDialog({
  filters: [
    { name: 'Images', extensions: ['jpg', 'png', 'gif'] },
    { name: 'Documents', extensions: ['pdf', 'xlsx'] },
  ],
});
```

---

### CWE-918: Server-Side Request Forgery (SSRF)

**Detection patterns**:
```
fetch\(\s*\w+          # fetch() with a variable URL (check if user-controlled)
axios\.\w+\(\s*\w+     # axios with variable URL
http\.request\(\s*\w+  # Node http with variable URL
got\(\s*\w+            # got with variable URL
```

**Fix pattern**: Validate URLs against an allowlist:
```typescript
const ALLOWED_HOSTS = ['api.example.com', 'cdn.example.com'];

function validateUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'https:' && ALLOWED_HOSTS.includes(parsed.hostname);
  } catch {
    return false;
  }
}
```

---

### CWE-306: Missing Authentication for Critical Function

**Detection patterns**: Same as CWE-862. Check that all IPC handlers performing destructive operations (delete, update, export) require authentication.

---

### CWE-190: Integer Overflow

**Detection patterns**:
```
parseInt\(.*\)(?!.*isNaN|.*isFinite|.*Number\.isSafe)
Number\(.*\)(?!.*isFinite|.*isSafe)
\*\s*\d{4,}                # Multiplication with large constants
```

**Fix pattern**: Use `Number.isSafeInteger()` checks and BigInt for large financial calculations:
```typescript
function safeMultiply(a: number, b: number): number {
  const result = a * b;
  if (!Number.isSafeInteger(result)) {
    throw new RangeError('Integer overflow detected');
  }
  return result;
}
```

---

### CWE-863: Incorrect Authorization

**Detection patterns**: Manual code review required. Look for:
- Role checks using `==` instead of `===`
- Authorization logic that only checks one of multiple required conditions
- Permission checks that can be bypassed by manipulating the request

---

### CWE-476: NULL Pointer Dereference

**Detection patterns**:
```
\.get\(\s*['"].*['"]\s*\)\.\w+    # Chaining after .get() without null check
db\.\w+\(.*\)\.\w+                 # Chaining after DB query without null check
```

**Fix pattern**: Use optional chaining and null checks:
```typescript
const result = db.get('SELECT * FROM users WHERE id = ?', [id]);
if (!result) {
  throw new Error('User not found');
}
// Now safe to access result.name
```

---

## Medium Severity (Fix in Next Sprint)

### CWE-362: Race Condition

**Detection patterns**:
```
fs\.existsSync\(.*\).*fs\.(read|write|unlink)    # TOCTOU
fs\.accessSync\(.*\).*fs\.(read|write|unlink)     # TOCTOU
```

**Fix pattern**: Use atomic operations or file locking:
```typescript
// VULNERABLE (TOCTOU)
if (fs.existsSync(filePath)) {
  fs.readFileSync(filePath);
}

// SAFE — just try the operation and handle the error
try {
  const data = fs.readFileSync(filePath);
} catch (err) {
  if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
    // File does not exist
  }
}
```

---

### CWE-276: Incorrect Default Permissions

**Detection patterns**:
```
fs\.writeFileSync\((?!.*mode)
fs\.mkdirSync\((?!.*mode)
chmod\s+777
chmod\s+666
```

**Fix pattern**: Always set restrictive file permissions:
```typescript
fs.writeFileSync(configPath, data, { mode: 0o600 }); // Owner read/write only
fs.mkdirSync(dirPath, { mode: 0o700 });               // Owner full access only
```

---

### CWE-352: Cross-Site Request Forgery

**Detection patterns** (relevant if your Electron app runs a local HTTP server):
```
app\.post\(.*(?!.*csrf|.*token)
app\.put\(.*(?!.*csrf|.*token)
app\.delete\(.*(?!.*csrf|.*token)
```

**Fix pattern**: For Electron apps with local HTTP servers, use CSRF tokens or restrict to localhost connections only with origin validation.

---

### CWE-416: Use After Free

**Detection patterns**: Primarily relevant for native addons (C/C++). In JavaScript/TypeScript, look for:
```
\.destroy\(\).*\.\w+    # Accessing an object after calling .destroy()
\.close\(\).*\.\w+      # Accessing after .close()
\.end\(\).*\.write       # Writing after .end()
```

**Fix pattern**: Set references to null after destruction and check before use:
```typescript
db.close();
db = null;
// Later...
if (db) { db.run(...); }
```

---

### CWE-125: Out-of-bounds Read

**Detection patterns**:
```
\[\w+\](?!.*\.length|.*\.size)    # Array access without length guard
buffer\.read\w+\(                  # Buffer read methods without bounds check
```

**Fix pattern**: Always check array bounds before access:
```typescript
if (index >= 0 && index < array.length) {
  const value = array[index];
}
```

---

## Electron-Specific CWEs (Not in Top 25 but Critical for Desktop)

### Unsafe shell.openExternal

**Detection pattern**:
```
shell\.openExternal\((?!.*https:\/\/|.*validateUrl|.*allowlist)
```

**Fix pattern**: Validate and restrict to HTTPS URLs only. See `electron-security.md`.

### Exposed ipcRenderer

**Detection pattern**:
```
contextBridge\.exposeInMainWorld\(.*ipcRenderer
exposeInMainWorld.*ipcRenderer\.send
exposeInMainWorld.*ipcRenderer\.on
```

**Fix pattern**: Never expose raw ipcRenderer. Wrap each channel individually.

### Remote Module Usage

**Detection pattern**:
```
require\(['"]@electron/remote['"]\)
enableRemoteModule
remote\.require
remote\.getCurrentWindow
remote\.getGlobal
```

**Fix pattern**: Remove the remote module entirely. Use IPC for all main-renderer communication.
