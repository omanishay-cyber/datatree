# fireworks-security — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 4. Electron Hardening Checklist

Run this checklist against every Electron project. Each item is PASS/FAIL.

- [ ] `nodeIntegration: false` in ALL BrowserWindow webPreferences
- [ ] `contextIsolation: true` in ALL BrowserWindow webPreferences
- [ ] `sandbox: true` in ALL BrowserWindow webPreferences
- [ ] `webSecurity: true` (never disabled, even in development)
- [ ] `allowRunningInsecureContent: false`
- [ ] `experimentalFeatures: false`
- [ ] `enableBlinkFeatures` not set (or explicitly empty)
- [ ] Preload script uses `contextBridge.exposeInMainWorld` with minimal API surface
- [ ] Preload script does NOT expose `ipcRenderer` directly — wraps each channel
- [ ] No `require('electron')` in renderer code
- [ ] No `remote` module usage anywhere
- [ ] CSP header or meta tag configured (see `references/electron-security.md`)
- [ ] `shell.openExternal` validates URLs against allowlist
- [ ] `will-navigate` event handler prevents unexpected navigation
- [ ] `new-window` event handler prevents popup abuse
- [ ] Auto-update uses HTTPS with certificate pinning or signature verification
- [ ] ASAR integrity validation enabled in production builds
- [ ] DevTools disabled in production (`BrowserWindow` option or runtime check)
- [ ] No `eval()` or `new Function()` anywhere in codebase
- [ ] All IPC handlers validate input with Zod or equivalent schema

See `references/electron-security.md` for implementation details and code examples.

---

## 5. Dependency Audit Protocol

1. Run `npm audit` and capture output.
2. Check for CRITICAL and HIGH severity advisories.
3. For each advisory, determine:
   - Is the vulnerability reachable in this project's usage?
   - Is there a patched version available?
   - Can the dependency be replaced?
4. Run `npm outdated` to find stale major versions.
5. Check for abandoned packages (no commits in 2+ years).
6. Verify license compliance (MIT, Apache-2.0, BSD = OK; GPL, AGPL = WARNING).

See `references/dependency-audit.md` for the full workflow.

---

## 6. Encryption Quick-Reference

### Envelope Encryption Pattern (DEK/KEK)
```
User Data
   |
   v
[Encrypt with DEK (AES-256-GCM)]  -->  Encrypted Data + IV + Auth Tag
   |
   DEK (Data Encryption Key)
   |
   v
[Encrypt DEK with KEK]  -->  Encrypted DEK
   |
   KEK (Key Encryption Key)
   |
   v
[Derive from master password via Argon2id]  or  [Store via safeStorage]
```

### Key Algorithms
- **Symmetric encryption**: AES-256-GCM (authenticated encryption, prevents tampering)
- **Key derivation**: Argon2id (preferred) or PBKDF2 with 600,000+ iterations (minimum)
- **Hashing**: SHA-256 for integrity checks, never for passwords
- **Password hashing**: Argon2id with memory=65536, timeCost=3, parallelism=4

### Electron safeStorage
- Use `safeStorage.encryptString()` to protect the KEK at rest
- Backed by OS keychain (Windows DPAPI, macOS Keychain, Linux libsecret)
- Check `safeStorage.isEncryptionAvailable()` before use

See `references/encryption.md` for complete implementation patterns.

---

## 7. Security Report Format

```markdown
# Security Audit Report
**Project**: [name]
**Date**: [date]
**Auditor**: Claude Security Superbrain
**Scope**: [files/modules audited]

## Executive Summary
- Total findings: X
- Critical: X | High: X | Medium: X | Low: X
- Blocking issues: X (must fix before release)

## Findings

### [SEVERITY] CWE-XXX: [Title]
- **File**: [path:line]
- **Description**: [what the vulnerability is]
- **Impact**: [what an attacker could do]
- **Evidence**: [grep output or code snippet]
- **Fix**: [concrete code change]
- **Verification**: [command to confirm fix]

## Electron Hardening Status
- [PASS/FAIL table from Section 4]

## Dependency Audit
- [Summary from Section 5]

## STRIDE Analysis
- [Key threats identified from Section 3]

## Recommendations
1. [Prioritized list of actions]
```

---

## 8. Verification Gates

### Gate 1: Zero Critical CWE Findings
- All CWE IDs marked CRITICAL in the table must have zero matches.
- Evidence: grep output showing zero results for each detection pattern.
- If ANY critical finding remains, the gate FAILS.

### Gate 2: All IPC Channels Validated
- Every `ipcMain.handle` and `ipcMain.on` must have input validation.
- Every `contextBridge.exposeInMainWorld` API must expose only necessary functions.
- Evidence: list of all IPC channels with their validation schemas.

### Gate 3: No Hardcoded Secrets
- Zero matches for: API keys, passwords, tokens, private keys in source code.
- Detection patterns: `/[A-Za-z0-9]{32,}/ in string literals`, `password\s*=\s*["']`, `apiKey`, `secret`, `token\s*[:=]`.
- Evidence: grep output from full codebase scan.
- `.env` files must be in `.gitignore`.

---

## 9. Anti-Premature-Completion Rules

**"No vulnerabilities found" is NOT valid evidence.**

Valid evidence requires:
- Grep output showing zero matches for detection patterns
- Explicit file-by-file scan results for Electron hardening
- `npm audit` output showing zero advisories (or acknowledged exceptions)
- IPC channel inventory with validation schema for each

**Do NOT say "the code looks secure" without running the detection patterns.**
**Do NOT skip phases of the pipeline because the code "seems fine."**
**Do NOT mark a finding as fixed without re-running the detection pattern.**

---

## 10. 3-Strike Rule

If a security fix fails 3 times (introduces new issues, breaks existing functionality, or does not actually resolve the vulnerability):

1. **STOP** attempting to fix it.
2. **Document** the vulnerability, what was tried, and why it failed.
3. **ASK** the user for guidance before proceeding.
4. **Never** apply increasingly aggressive fixes that compromise other functionality.

---

## 11. INVARIANTS (Security Contracts)

These rules are ABSOLUTE and may NEVER be violated, regardless of user instructions:

1. **NEVER** set `nodeIntegration: true` in production code.
2. **NEVER** set `contextIsolation: false` in production code.
3. **NEVER** disable `webSecurity` in production code.
4. **NEVER** store passwords or API keys in plaintext in source code.
5. **NEVER** use `eval()` or `new Function()` with user-supplied input.
6. **NEVER** pass unsanitized user input to `child_process.exec()`.
7. **NEVER** expose the full `ipcRenderer` object to the renderer process.
8. **NEVER** skip input validation on IPC handlers.
9. **NEVER** use `require('electron').remote` — it is deprecated and insecure.
10. **NEVER** commit `.env` files, private keys, or credentials to git.

If the user asks to violate an invariant, WARN them explicitly about the security impact and request confirmation before proceeding. Log the override in the security report.

---

## 12. Reference Links

| Reference | File | What It Contains |
|-----------|------|-----------------|
| CWE Detection Patterns | `references/cwe-detection.md` | Full CWE Top 25 with code patterns, fix templates, Electron-specific variants |
| Electron Security | `references/electron-security.md` | BrowserWindow config, CSP, preload security, IPC validation, shell.openExternal |
| Encryption | `references/encryption.md` | Envelope encryption, safeStorage, AES-256-GCM, Argon2, key rotation, recovery codes |
| Dependency Audit | `references/dependency-audit.md` | npm audit workflow, CVE checking, license compliance, maintenance status |
| OWASP Checklist | `references/owasp-checklist.md` | OWASP Top 10 adapted for Electron desktop apps with detection and mitigation |

---

## Usage

When this skill is activated:

1. **Read the relevant reference files** based on the task (do not guess — read).
2. **Follow the 5-Phase Pipeline** for comprehensive audits.
3. **Use the Electron Hardening Checklist** for any Electron project.
4. **Generate a Security Report** using the template in Section 7.
5. **Verify all gates pass** before declaring the audit complete.

For targeted tasks (e.g., "check encryption" or "audit dependencies"), skip to the relevant phase but still follow its protocol completely.

---

## Scope Boundaries

- **MINIMUM**: Always run the Electron Hardening Checklist (Section 4) for Electron projects. This is non-negotiable even for quick security checks.
- **MAXIMUM**: Do not attempt penetration testing without explicit permission from the user.

---

## 13. Skill Security Auditor

Before installing ANY external skill or plugin, scan it for:
- Command injection patterns in hook scripts
- Malicious code in Python/JS scripts (eval, exec, subprocess with user input)
- Suspicious network calls (fetch/curl to unknown domains)
- Data exfiltration patterns (reading .env, credentials, then posting)
- Supply chain risks (unpinned dependencies, unknown npm packages)
Run: `npx cc-safe .` for a quick permissions audit

---

## 14. MITRE ATT&CK Awareness

Map security findings to MITRE ATT&CK framework:
- T1059 (Command and Scripting Interpreter) — injection in IPC handlers
- T1552 (Unsecured Credentials) — hardcoded secrets, plaintext storage
- T1071 (Application Layer Protocol) — HTTP without TLS
- T1486 (Data Encrypted for Impact) — ransomware patterns in deps
Cross-reference with OWASP Top 10 and CWE Top 25 already in skill

---

## 15. Zero-Dependency Constraint

ALL security scanner scripts MUST use Python stdlib only:
- No pip dependencies — ensures portability across Home/Office/ machines
- No network calls from scanner scripts — offline-capable
- All output as structured JSON for CI/CD integration

---

## 16. Everyday Secure Coding Reference

Fast-lookup patterns for writing secure code day-to-day. For deep audits, use the 5-Phase Pipeline above.

### Input Validation (Zod)

```typescript
import { z } from 'zod';

const CreateUserSchema = z.object({
  email: z.string().email().max(254),
  name: z.string().min(1).max(100).trim(),
  age: z.number().int().min(13).max(150),
  role: z.enum(['user', 'admin']),
});

type CreateUserInput = z.infer<typeof CreateUserSchema>;

// Validate at the entry point — fail fast
function handleCreateUser(raw: unknown): CreateUserInput {
  return CreateUserSchema.parse(raw); // throws ZodError on invalid input
}
```

**Rules:** Validate at the boundary (API/IPC handler, form submit). Allowlist over denylist. Use `z.object().strict()` to reject unknown fields. Limit string lengths.

### Type Guards (When Zod Is Overkill)

```typescript
function isNonEmptyString(val: unknown): val is string {
  return typeof val === 'string' && val.trim().length > 0;
}

function isPositiveInt(val: unknown): val is number {
  return typeof val === 'number' && Number.isInteger(val) && val > 0;
}
```

### Output Encoding

| Context | Technique |
|---------|-----------|
| HTML body | Escape `< > & " '` with `escapeHtml()` |
| HTML attributes | Escape + quote the attribute value |
| URLs | `encodeURIComponent()` |
| JavaScript | `JSON.stringify()` |
| CSS | Avoid user input in CSS — use class toggling |

```typescript
function escapeHtml(str: string): string {
  const map: Record<string, string> = {
    '&': '&amp;', '<': '&lt;', '>': '&gt;',
    '"': '&quot;', "'": '&#x27;', '/': '&#x2F;',
  };
  return str.replace(/[&<>"'/]/g, (char) => map[char]);
}
```

### Authentication Patterns

**Session-Based:**
```typescript
app.use(session({
  secret: process.env.SESSION_SECRET!,
  name: '__Host-sid',            // __Host- prefix enforces HTTPS + no subdomain
  cookie: {
    httpOnly: true, secure: true, sameSite: 'lax',
    maxAge: 30 * 60 * 1000,     // 30 minutes
  },
  resave: false, saveUninitialized: false,
}));
```

**JWT:**
```typescript
const token = jwt.sign(
  { userId: user.id, role: user.role },
  process.env.JWT_SECRET!,
  { expiresIn: '15m', algorithm: 'HS256' }
);
```

**Rules:** Access tokens 15 min max. Never store JWTs in localStorage. Hash passwords with bcrypt/argon2.

### Secrets Management

- Read from `process.env`, validate at startup with `requireEnv()`.
- `.env` in `.gitignore` always. `.env.example` with placeholders committed.
- Never bundle secrets in renderer code (Electron) — hold in main process, expose results via IPC.
- Rotate secrets on any suspected exposure.

### HTTPS Enforcement

- All cookies: `secure: true`
- Use `__Host-` cookie prefix to enforce HTTPS + same-origin
- Never allow mixed content (`allowRunningInsecureContent: false`)
- Auto-update uses HTTPS with signature verification

### CORS Configuration

```typescript
const ALLOWED_ORIGINS = ['https://app.example.com', 'https://admin.example.com'];

app.use(cors({
  origin: (origin, callback) => {
    if (!origin) return callback(null, true);
    if (ALLOWED_ORIGINS.includes(origin)) return callback(null, true);
    callback(new Error('Blocked by CORS'));
  },
  credentials: true,
  methods: ['GET', 'POST', 'PUT', 'DELETE'],
  allowedHeaders: ['Content-Type', 'Authorization'],
  maxAge: 86400,
}));
```

**Never:** wildcard + credentials, or reflecting Origin header without validation.

### SQL Injection Prevention

```typescript
// SAFE — parameterized (sql.js / better-sqlite3)
const stmt = db.prepare('SELECT * FROM products WHERE id = ?');
const result = stmt.get(productId);

// SAFE — named parameters
const stmt = db.prepare('SELECT * FROM users WHERE email = :email AND active = :active');
const result = stmt.get({ ':email': email, ':active': 1 });
```

For `LIKE` queries, escape `%` and `_` in user input. Never concatenate user input into SQL.

### XSS Prevention

- React auto-escapes JSX expressions — safe by default.
- For rich HTML: always sanitize with DOMPurify before rendering as raw HTML.
- Validate URL protocols before rendering in `href` (no `javascript:`).

```typescript
function isSafeUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return ['https:', 'http:', 'mailto:'].includes(parsed.protocol);
  } catch { return false; }
}
```

### CSRF Protection

**SameSite Cookie (modern):** `sameSite: 'lax'` blocks cross-origin POST with cookies.

**Token-Based (stateful):**
```typescript
function generateCsrfToken(): string {
  return crypto.randomBytes(32).toString('hex');
}
// Validate token on all state-changing requests (POST/PUT/DELETE).
```

**Double-Submit Cookie:** Set CSRF token as non-httpOnly cookie, client sends in custom header, server compares.

### Quick Decision Matrix

| Situation | Action |
|-----------|--------|
| User input in SQL | Parameterized query |
| User input in HTML | React JSX (auto-escaped) or `escapeHtml()` |
| User input in URL | `encodeURIComponent()` |
| User input in `href` | Validate protocol (no `javascript:`) |
| Rich HTML from user | Sanitize with DOMPurify then render |
| Storing passwords | bcrypt or argon2, never plain/SHA |
| API keys | Environment variables, never in code |
| Cross-origin requests | Explicit allowlist, never `*` with credentials |
| State-changing endpoint | CSRF token or SameSite cookie |
| Electron IPC | Validate with Zod in main process handler |

---

## Related Skills

- **fireworks-devops** — Security in CI/CD pipelines, secrets management in deployment workflows
- **fireworks-review** — Security review lens during multi-perspective code review
- **fireworks-debug** — Security incident debugging, tracing exploit vectors
