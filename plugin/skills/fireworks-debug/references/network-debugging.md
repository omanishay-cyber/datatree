# Network Debugging — API and HTTP Protocol

> 5-step protocol for debugging network and API issues in Electron apps.
> Covers request classification, retry logic, and Electron-specific patterns.

---

## 5-Step Network Debugging Protocol

### Step 1: Reproduce
Trigger the failing network request. Watch both:
- DevTools Network tab (renderer process)
- Main process terminal (if request is made from main)

### Step 2: Identify
Find the failing request in the Network tab. Note:
- URL: Is it correct?
- Method: GET, POST, PUT, DELETE — is it right?
- Status code: What did the server return?
- Request headers: Are auth tokens present?
- Request body: Is the payload correct?
- Response body: What did the server say?

### Step 3: Classify the Failure
Use the classification table below to determine the type of failure.

### Step 4: Check Electron-Specific Issues
- Is the request made from renderer or main? (CORS applies differently)
- Is the CSP blocking the request?
- Is the proxy/certificate configuration correct?

### Step 5: Fix Based on Classification
Apply the appropriate fix from the classification table.

---

## Request Failure Classification

### Class 1: No Request Sent
**Symptom**: Nothing appears in the Network tab. No request leaves the app.

**Causes**:
- The fetch/API call is never executed (check if the code path runs)
- URL is malformed (undefined variables in the URL string)
- The request is blocked by CSP
- CORS preflight fails before the actual request

**Diagnostic**:
```typescript
console.log('[NET] About to fetch:', url, options);
try {
  const response = await fetch(url, options);
  console.log('[NET] Response received:', response.status);
} catch (error) {
  console.error('[NET] Fetch failed:', error);
}
```

---

### Class 2: Request Sent, No Response
**Symptom**: Request appears in Network tab as "pending" indefinitely, then times out.

**Causes**:
- Server is down or unreachable
- DNS resolution failed
- Firewall blocking the connection
- Network is offline
- Request timeout too short

**Diagnostic**:
```typescript
// Add timeout to detect hung requests:
const controller = new AbortController();
const timeout = setTimeout(() => controller.abort(), 10000);

try {
  const response = await fetch(url, { signal: controller.signal });
  clearTimeout(timeout);
} catch (error) {
  if (error.name === 'AbortError') {
    console.error('[NET] Request timed out after 10s');
  }
}
```

---

### Class 3: Wrong Response Data
**Symptom**: Request succeeds (200 OK) but the data is wrong, empty, or unexpected format.

**Causes**:
- Wrong endpoint (hitting a different API route)
- Wrong query parameters or request body
- API version mismatch
- Server returning cached/stale data
- Response format changed (JSON structure different than expected)

**Diagnostic**:
```typescript
const response = await fetch(url);
const text = await response.text(); // Get raw text first
console.log('[NET] Raw response:', text);
try {
  const data = JSON.parse(text);
  console.log('[NET] Parsed data:', data);
} catch {
  console.error('[NET] Response is not valid JSON:', text);
}
```

---

### Class 4: Parse Failure
**Symptom**: Request succeeds but `response.json()` throws, or data processing fails.

**Causes**:
- Server returned HTML error page instead of JSON (404 page, 500 page)
- Server returned empty body
- Encoding issue (UTF-8 BOM, wrong content-type)
- Response is truncated

**Diagnostic**:
```typescript
const response = await fetch(url);
console.log('[NET] Content-Type:', response.headers.get('content-type'));
console.log('[NET] Content-Length:', response.headers.get('content-length'));
const text = await response.text();
console.log('[NET] Body length:', text.length);
console.log('[NET] First 200 chars:', text.substring(0, 200));
```

---

## Status Code Reference — Retry vs Do Not Retry

### Retry These (Transient Errors)
| Code | Meaning | Retry Strategy |
|------|---------|---------------|
| 429 | Too Many Requests | Retry after `Retry-After` header value |
| 500 | Internal Server Error | Retry with exponential backoff |
| 502 | Bad Gateway | Retry with exponential backoff |
| 503 | Service Unavailable | Retry after `Retry-After` header value |
| 504 | Gateway Timeout | Retry with exponential backoff |

### Do NOT Retry These (Client Errors)
| Code | Meaning | Action |
|------|---------|--------|
| 400 | Bad Request | Fix the request payload |
| 401 | Unauthorized | Refresh auth token, re-authenticate |
| 403 | Forbidden | Check permissions, check API key |
| 404 | Not Found | Fix the URL or check if resource exists |
| 405 | Method Not Allowed | Fix the HTTP method (GET vs POST) |
| 409 | Conflict | Resolve the conflict (e.g., version mismatch) |
| 422 | Unprocessable Entity | Fix validation errors in the payload |

---

## Electron-Specific Network Issues

### CORS in Electron
Renderer process is subject to CORS. Main process is NOT.

**If a fetch fails with CORS error in renderer**:
1. Move the request to the main process
2. Call it via IPC from the renderer
3. Return the response data through IPC

```typescript
// Main process:
ipcMain.handle('api-fetch', async (_, url, options) => {
  const response = await fetch(url, options);
  return {
    status: response.status,
    data: await response.json(),
  };
});

// Renderer (via preload):
const result = await window.api.apiFetch('https://api.example.com/data', {
  method: 'GET',
  headers: { 'Authorization': `Bearer ${token}` },
});
```

### Certificate Issues
Electron may reject self-signed certificates in production:
```typescript
// For development only — never in production:
app.commandLine.appendSwitch('ignore-certificate-errors');

// Better: configure certificate verification
app.on('certificate-error', (event, webContents, url, error, certificate, callback) => {
  if (url.startsWith('https://localhost')) {
    event.preventDefault();
    callback(true); // Trust localhost certs in dev
  } else {
    callback(false); // Reject all others
  }
});
```

### Proxy Configuration
```typescript
// Set proxy for all requests:
app.on('ready', () => {
  const ses = session.defaultSession;
  ses.setProxy({ proxyRules: 'http://proxy:8080' });
});
```

---

## Retry Implementation

```typescript
async function fetchWithRetry(
  url: string,
  options: RequestInit = {},
  maxRetries = 3
): Promise<Response> {
  let lastError: Error | null = null;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const response = await fetch(url, options);

      // Do not retry client errors (4xx except 429)
      if (response.status >= 400 && response.status < 500 && response.status !== 429) {
        return response; // Return as-is, caller handles the error
      }

      // Retry server errors and rate limits
      if (response.status === 429 || response.status >= 500) {
        if (attempt === maxRetries) return response;

        const retryAfter = response.headers.get('Retry-After');
        const delay = retryAfter
          ? parseInt(retryAfter, 10) * 1000
          : Math.min(1000 * Math.pow(2, attempt), 10000);

        console.warn(`[NET] Retry ${attempt + 1}/${maxRetries} after ${delay}ms (${response.status})`);
        await new Promise(resolve => setTimeout(resolve, delay));
        continue;
      }

      return response; // Success
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      if (attempt === maxRetries) throw lastError;

      const delay = Math.min(1000 * Math.pow(2, attempt), 10000);
      console.warn(`[NET] Retry ${attempt + 1}/${maxRetries} after ${delay}ms (network error)`);
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  throw lastError ?? new Error('Fetch failed after retries');
}
```
