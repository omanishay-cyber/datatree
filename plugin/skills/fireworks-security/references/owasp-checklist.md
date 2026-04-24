# OWASP Top 10 -- Adapted for Electron Desktop Apps

The OWASP Top 10 web application security risks, reinterpreted for the Electron desktop
application context. Each risk is mapped to its Electron-specific attack vectors and mitigations.

---

## 1. Injection (A03:2021)

**Web context**: SQL injection, LDAP injection, OS command injection.
**Electron context**: SQL injection via sql.js/better-sqlite3, IPC channel injection, command injection through process spawning.

**Attack vectors**:
- String concatenation in SQL queries executed by the main process
- IPC messages with unsanitized payloads that are passed to database queries
- User input flowing into shell commands via child process APIs

**Mitigations**:
- Parameterized queries for ALL database operations
- Input validation on ALL IPC handler arguments
- Use execFile with argument arrays instead of shell-based execution
- Sanitize and validate all data crossing trust boundaries

---

## 2. Broken Authentication (A07:2021)

**Web context**: Session management flaws, credential stuffing.
**Electron context**: Local credential storage, session token management, multi-user access.

**Attack vectors**:
- Credentials stored in plaintext config files
- Session tokens that never expire
- No account lockout after failed login attempts
- Weak or missing PIN/password verification for local access

**Mitigations**:
- Use Electron safeStorage for credential encryption
- Implement session timeouts (lock screen after inactivity)
- bcrypt or Argon2id for password hashing
- Rate limiting on authentication attempts
- Biometric authentication where available (via system APIs)

---

## 3. Sensitive Data Exposure (A02:2021)

**Web context**: Data in transit, TLS configuration.
**Electron context**: Local database encryption, config file protection, memory exposure.

**Attack vectors**:
- Unencrypted SQLite databases readable by any process
- Sensitive data in app logs or crash reports
- Credentials in memory accessible via process dump
- Config files with plaintext secrets

**Mitigations**:
- Envelope encryption for database records
- Encrypt entire database or sensitive columns
- Redact sensitive fields in all log output
- Zero out sensitive buffers after use
- Set file permissions to owner-only (0o600)

---

## 4. XML External Entities -- XXE (A05:2021)

**Web context**: XML parser attacks.
**Electron context**: Import/export features that parse XML, third-party XML data processing.

**Attack vectors**:
- XML import features that process files with external entity references
- SVG files containing external entity declarations
- XSLT processing with external references

**Mitigations**:
- Disable DTD processing in all XML parsers
- Use JSON instead of XML where possible
- If XML is required, use a parser configured to reject external entities
- Validate and sanitize imported XML files

---

## 5. Broken Access Control (A01:2021)

**Web context**: Privilege escalation, IDOR.
**Electron context**: IPC privilege escalation, renderer accessing main-process resources.

**Attack vectors**:
- Renderer process sending IPC messages to access admin-only functions
- Missing permission checks on IPC handlers
- File system access through IPC without path validation
- User A accessing User B's data in multi-user setups

**Mitigations**:
- Permission checks on EVERY IPC handler
- Role-based access control for multi-user features
- Validate that requested resources belong to the authenticated user
- Minimize the IPC API surface (preload exposes only necessary functions)
- Verify IPC sender identity via event.senderFrame

---

## 6. Security Misconfiguration (A05:2021)

**Web context**: Default credentials, open cloud storage.
**Electron context**: BrowserWindow settings, CSP, development features in production.

**Attack vectors**:
- nodeIntegration set to true in production
- contextIsolation set to false
- Missing or permissive CSP
- DevTools accessible in production builds
- Debug logging enabled in production
- Auto-updater using HTTP instead of HTTPS

**Mitigations**:
- Security checklist for all BrowserWindow configurations
- Strict CSP policy
- Disable DevTools in production builds
- Remove or disable debug logging in production builds
- HTTPS for all network communication including updates
- Electron fuses for compile-time hardening

---

## 7. Cross-Site Scripting -- XSS (A03:2021)

**Web context**: Reflected, stored, DOM-based XSS.
**Electron context**: Unsafe HTML rendering, user content rendering, IPC data display.

**Attack vectors**:
- Rendering user-supplied HTML without sanitization
- Displaying IPC response data as raw HTML
- Loading external content in webview/iframe without CSP
- Custom protocol handlers serving user content

**Mitigations**:
- Use React JSX which auto-escapes by default
- Sanitize with DOMPurify when raw HTML rendering is required
- Strict CSP blocking inline scripts
- Never render untrusted data as raw HTML
- Validate and sanitize all data displayed in the renderer

---

## 8. Insecure Deserialization (A08:2021)

**Web context**: Object deserialization attacks.
**Electron context**: IPC message deserialization, file format parsing, import/export.

**Attack vectors**:
- Malformed IPC messages causing unexpected behavior
- Import files (CSV, JSON, XML) with crafted payloads
- Plugin/extension loading without validation
- Database backup/restore with tampered data

**Mitigations**:
- Schema validation (zod/joi) on ALL IPC message payloads
- Validate imported file structure before processing
- Type-check all deserialized data before use
- Never run code derived from deserialized data
- Integrity checks (HMAC) on exported/imported data

---

## 9. Using Components with Known Vulnerabilities (A06:2021)

**Web context**: Vulnerable libraries and frameworks.
**Electron context**: npm dependencies, Electron version, native modules.

**Attack vectors**:
- Outdated Electron version with known CVEs
- npm packages with published vulnerabilities
- Native modules compiled against vulnerable libraries
- Transitive dependencies introducing vulnerabilities

**Mitigations**:
- Run `npm audit` regularly (weekly minimum)
- Keep Electron updated to latest stable release
- Monitor GitHub Dependabot alerts
- Review dependency tree for unnecessary packages
- Pin dependency versions in package-lock.json
- Audit new dependencies before adding them

---

## 10. Insufficient Logging and Monitoring (A09:2021)

**Web context**: Missing audit trails, no alerting.
**Electron context**: Action tracking, error reporting, security event logging.

**Attack vectors**:
- No record of who performed sensitive operations
- Failed login attempts not tracked
- Data modifications without audit trail
- Security-relevant errors silently swallowed

**Mitigations**:
- Log all authentication events (success and failure)
- Log all data modification operations with user identity and timestamp
- Log all IPC calls to sensitive handlers
- Implement structured logging with log levels
- Store logs securely (not accessible to renderer process)
- Include session/request IDs for correlation
- Regular log review for anomalous patterns

**Audit log schema**:
```typescript
interface AuditEntry {
  timestamp: string;    // ISO 8601
  userId: string;       // Who performed the action
  action: string;       // What was done (e.g., 'product:update')
  resource: string;     // What was affected (e.g., 'product:123')
  details: object;      // What changed (before/after)
  result: 'success' | 'failure';
  ip?: string;          // If network-originated
  sessionId: string;    // For correlation
}
```
